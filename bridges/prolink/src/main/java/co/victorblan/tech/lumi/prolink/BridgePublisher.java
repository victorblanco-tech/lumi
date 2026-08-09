package co.victorblan.tech.lumi.prolink;

import com.fasterxml.jackson.databind.ObjectMapper;

import java.io.IOException;
import java.io.OutputStream;
import java.util.Objects;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;

final class BridgePublisher implements AutoCloseable {
    private static final int QUEUE_CAPACITY = 4096;

    private final ArrayBlockingQueue<PendingEvent> queue = new ArrayBlockingQueue<>(QUEUE_CAPACITY);
    private final AtomicBoolean running = new AtomicBoolean(true);
    private final AtomicLong sequence = new AtomicLong();
    private final AtomicLong droppedEvents = new AtomicLong();
    private final ObjectMapper mapper;
    private final OutputStream output;
    private final Thread writerThread;

    BridgePublisher(OutputStream output) {
        this(output, new ObjectMapper());
    }

    BridgePublisher(OutputStream output, ObjectMapper mapper) {
        this.output = Objects.requireNonNull(output, "output");
        this.mapper = Objects.requireNonNull(mapper, "mapper");
        writerThread = Thread.ofPlatform()
                .name("lumi-prolink-protocol-writer")
                .daemon(true)
                .start(this::writeEvents);
    }

    boolean publish(String type, Object payload) {
        Objects.requireNonNull(type, "type");
        Objects.requireNonNull(payload, "payload");
        if (!running.get()) {
            return false;
        }
        boolean accepted = queue.offer(new PendingEvent(System.nanoTime(), type, payload));
        if (!accepted) {
            droppedEvents.incrementAndGet();
        }
        return accepted;
    }

    long droppedEvents() {
        return droppedEvents.get();
    }

    private void writeEvents() {
        while (running.get() || !queue.isEmpty()) {
            try {
                PendingEvent event = queue.poll(100, TimeUnit.MILLISECONDS);
                if (event != null) {
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

    private void write(PendingEvent event) throws IOException {
        BridgeEnvelope envelope = new BridgeEnvelope(
                BridgeProtocol.NAME,
                BridgeProtocol.VERSION,
                sequence.incrementAndGet(),
                event.observedAtNanos(),
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
        try {
            writerThread.join(2_000);
        } catch (InterruptedException interrupted) {
            Thread.currentThread().interrupt();
        }
        if (writerThread.isAlive()) {
            writerThread.interrupt();
        }
    }

    private record PendingEvent(long observedAtNanos, String type, Object payload) {
    }
}
