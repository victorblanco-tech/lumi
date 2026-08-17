package co.victorblan.tech.lumi.prolink;

import com.fasterxml.jackson.databind.ObjectMapper;

import java.io.IOException;
import java.io.OutputStream;
import java.util.EnumMap;
import java.util.HashMap;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;

/** Keeps realtime control facts ahead of replaceable UI samples. */
final class BridgePublisher implements AutoCloseable {
    private static final int CRITICAL_QUEUE_CAPACITY = 256;

    private final ArrayBlockingQueue<PendingEvent> criticalQueue = new ArrayBlockingQueue<>(CRITICAL_QUEUE_CAPACITY);
    private final Object mailboxLock = new Object();
    private final EnumMap<BridgeTrafficClass, Map<Integer, PendingEvent>> mailboxes = new EnumMap<>(BridgeTrafficClass.class);
    private final Object availability = new Object();
    private final AtomicBoolean running = new AtomicBoolean(true);
    private final AtomicLong sequence = new AtomicLong();
    private final AtomicLong criticalSaturationCount = new AtomicLong();
    private final AtomicLong coalescedContinuousCount = new AtomicLong();
    private final ObjectMapper mapper;
    private final OutputStream output;
    private final Thread writerThread;

    BridgePublisher(OutputStream output) {
        this(output, new ObjectMapper());
    }

    BridgePublisher(OutputStream output, ObjectMapper mapper) {
        this.output = Objects.requireNonNull(output, "output");
        this.mapper = Objects.requireNonNull(mapper, "mapper");
        mailboxes.put(BridgeTrafficClass.TEMPO, new HashMap<>());
        mailboxes.put(BridgeTrafficClass.TRANSPORT, new HashMap<>());
        mailboxes.put(BridgeTrafficClass.DISPLAY, new HashMap<>());
        writerThread = Thread.ofPlatform().name("lumi-prolink-protocol-writer").daemon(true).start(this::writeEvents);
    }

    boolean publish(String type, Object payload) {
        return publishCritical(type, payload);
    }

    boolean publishCritical(String type, Object payload) {
        Objects.requireNonNull(type, "type");
        Objects.requireNonNull(payload, "payload");
        if (!running.get()) {
            return false;
        }
        boolean accepted = criticalQueue.offer(PendingEvent.now(BridgeTrafficClass.CRITICAL, type, payload));
        if (!accepted) {
            criticalSaturationCount.incrementAndGet();
            return false;
        }
        signalAvailability();
        return true;
    }

    boolean publishLatest(BridgeTrafficClass trafficClass, int deviceNumber, String type, Object payload) {
        Objects.requireNonNull(trafficClass, "trafficClass");
        Objects.requireNonNull(type, "type");
        Objects.requireNonNull(payload, "payload");
        if (trafficClass == BridgeTrafficClass.CRITICAL) {
            throw new IllegalArgumentException("Critical traffic must use publishCritical");
        }
        if (!running.get()) {
            return false;
        }
        PendingEvent previous;
        synchronized (mailboxLock) {
            previous = mailboxes.get(trafficClass).put(deviceNumber, PendingEvent.now(trafficClass, type, payload));
        }
        if (previous != null) {
            coalescedContinuousCount.incrementAndGet();
        }
        signalAvailability();
        return true;
    }

    long droppedEvents() {
        return criticalSaturationCount.get();
    }

    long criticalSaturationCount() {
        return criticalSaturationCount.get();
    }

    long coalescedContinuousCount() {
        return coalescedContinuousCount.get();
    }

    private void writeEvents() {
        while (running.get() || hasPendingEvents()) {
            try {
                PendingEvent event = nextEvent();
                if (event == null) {
                    waitForEvent();
                } else {
                    write(event);
                }
            } catch (InterruptedException interrupted) {
                Thread.currentThread().interrupt();
                break;
            } catch (IOException failure) {
                System.err.println("Pro Link bridge protocol output failed: " + failure.getMessage());
                running.set(false);
            }
        }
        flushOutput();
    }

    private PendingEvent nextEvent() {
        PendingEvent critical = criticalQueue.poll();
        if (critical != null) {
            return critical;
        }
        synchronized (mailboxLock) {
            for (BridgeTrafficClass lane : new BridgeTrafficClass[]{BridgeTrafficClass.TEMPO, BridgeTrafficClass.TRANSPORT, BridgeTrafficClass.DISPLAY}) {
                Map<Integer, PendingEvent> mailbox = mailboxes.get(lane);
                if (!mailbox.isEmpty()) {
                    Integer oldestDevice = mailbox.entrySet().stream()
                            .min(Map.Entry.comparingByValue((left, right) -> Long.compare(left.observedAtNanos(), right.observedAtNanos())))
                            .orElseThrow().getKey();
                    return mailbox.remove(oldestDevice);
                }
            }
        }
        return null;
    }

    private boolean hasPendingEvents() {
        if (!criticalQueue.isEmpty()) {
            return true;
        }
        synchronized (mailboxLock) {
            return mailboxes.values().stream().anyMatch(mailbox -> !mailbox.isEmpty());
        }
    }

    private void waitForEvent() throws InterruptedException {
        synchronized (availability) {
            if (!hasPendingEvents() && running.get()) {
                availability.wait(TimeUnit.MILLISECONDS.toMillis(100));
            }
        }
    }

    private void signalAvailability() {
        synchronized (availability) {
            availability.notifyAll();
        }
    }

    private void write(PendingEvent event) throws IOException {
        long queueAgeMicros = TimeUnit.NANOSECONDS.toMicros(Math.max(0L, System.nanoTime() - event.observedAtNanos()));
        BridgeEnvelope envelope = new BridgeEnvelope(
                BridgeProtocol.NAME,
                BridgeProtocol.VERSION,
                sequence.incrementAndGet(),
                event.observedAtNanos(),
                event.trafficClass().externalName(),
                queueAgeMicros,
                event.type(),
                event.payload()
        );
        byte[] json = mapper.writeValueAsBytes(envelope);
        output.write(json);
        output.write('\n');
        output.flush();
    }

    private void flushOutput() {
        try {
            output.flush();
        } catch (IOException failure) {
            System.err.println("Pro Link bridge protocol flush failed: " + failure.getMessage());
        }
    }

    @Override
    public void close() {
        if (!running.getAndSet(false)) {
            return;
        }
        signalAvailability();
        try {
            writerThread.join(2_000);
        } catch (InterruptedException interrupted) {
            Thread.currentThread().interrupt();
        }
        if (writerThread.isAlive()) {
            writerThread.interrupt();
        }
    }

    private record PendingEvent(long observedAtNanos, BridgeTrafficClass trafficClass, String type, Object payload) {
        static PendingEvent now(BridgeTrafficClass trafficClass, String type, Object payload) {
            return new PendingEvent(System.nanoTime(), trafficClass, type, payload);
        }
    }
}
