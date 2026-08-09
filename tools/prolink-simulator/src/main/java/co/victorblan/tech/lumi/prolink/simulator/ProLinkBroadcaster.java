package co.victorblan.tech.lumi.prolink.simulator;

import java.io.IOException;
import java.net.DatagramPacket;
import java.net.DatagramSocket;
import java.net.Inet4Address;
import java.net.InterfaceAddress;
import java.net.InetSocketAddress;
import java.net.NetworkInterface;
import java.net.SocketException;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.Enumeration;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

final class ProLinkBroadcaster implements AutoCloseable {
    private static final int ANNOUNCEMENT_PORT = 50_000;
    private static final int BEAT_PORT = 50_001;
    private static final int STATUS_PORT = 50_002;
    private static final String DEVICE_NAME = "LUMI-SIM";

    private final PlayerState state;
    private final Endpoint endpoint;
    private final DatagramSocket socket;
    private final ScheduledExecutorService scheduler = Executors.newScheduledThreadPool(
            3, Thread.ofPlatform().name("lumi-prolink-simulator-", 0).daemon(true).factory()
    );
    private final AtomicInteger packetCounter = new AtomicInteger();
    private final AtomicBoolean running = new AtomicBoolean();
    private volatile int lastBeatIndex = Integer.MIN_VALUE;
    private volatile long lastRevision = Long.MIN_VALUE;
    private volatile boolean lastPlaying;

    ProLinkBroadcaster(PlayerState state, String requestedInterface) throws IOException {
        this.state = Objects.requireNonNull(state, "state");
        this.endpoint = selectEndpoint(requestedInterface);
        this.socket = new DatagramSocket(new InetSocketAddress(endpoint.localAddress(), 0));
        socket.setBroadcast(true);
    }

    void start() {
        if (!running.compareAndSet(false, true)) {
            return;
        }
        scheduler.scheduleAtFixedRate(this::sendAnnouncementSafely, 0, 1_500, TimeUnit.MILLISECONDS);
        scheduler.scheduleAtFixedRate(this::sendStatusSafely, 0, 100, TimeUnit.MILLISECONDS);
        scheduler.scheduleAtFixedRate(this::sendBeatWhenDueSafely, 0, 5, TimeUnit.MILLISECONDS);
    }

    Endpoint endpoint() {
        return endpoint;
    }

    private void sendAnnouncementSafely() {
        try {
            DatagramPacket packet = ProLinkPackets.announcement(
                    DEVICE_NAME, state.snapshot().playerNumber(), endpoint.hardwareAddress(),
                    endpoint.localAddress(), 2
            );
            send(packet, ANNOUNCEMENT_PORT);
        } catch (Exception failure) {
            report("announcement", failure);
        }
    }

    private void sendStatusSafely() {
        try {
            DatagramPacket packet = ProLinkPackets.status(
                    DEVICE_NAME, state.snapshot(), packetCounter.incrementAndGet()
            );
            send(packet, STATUS_PORT);
        } catch (Exception failure) {
            report("status", failure);
        }
    }

    private void sendBeatWhenDueSafely() {
        try {
            PlayerState.Snapshot snapshot = state.snapshot();
            if (snapshot.revision() != lastRevision) {
                lastRevision = snapshot.revision();
                lastBeatIndex = snapshot.beatIndex();
            }
            if (!snapshot.playing() || snapshot.beatIndex() < 0) {
                lastPlaying = false;
                return;
            }
            if (!lastPlaying || snapshot.beatIndex() != lastBeatIndex) {
                lastPlaying = true;
                lastBeatIndex = snapshot.beatIndex();
                send(ProLinkPackets.beat(DEVICE_NAME, snapshot), BEAT_PORT);
            }
        } catch (Exception failure) {
            report("beat", failure);
        }
    }

    private void send(DatagramPacket packet, int port) throws IOException {
        packet.setAddress(endpoint.broadcastAddress());
        packet.setPort(port);
        socket.send(packet);
    }

    private static Endpoint selectEndpoint(String requestedName) throws SocketException {
        List<Endpoint> candidates = new ArrayList<>();
        Enumeration<NetworkInterface> interfaces = NetworkInterface.getNetworkInterfaces();
        while (interfaces.hasMoreElements()) {
            NetworkInterface networkInterface = interfaces.nextElement();
            if (!networkInterface.isUp() || networkInterface.isLoopback() || networkInterface.isVirtual()) {
                continue;
            }
            byte[] hardwareAddress = networkInterface.getHardwareAddress();
            if (hardwareAddress == null) {
                hardwareAddress = new byte[6];
            }
            for (InterfaceAddress interfaceAddress : networkInterface.getInterfaceAddresses()) {
                if (!(interfaceAddress.getAddress() instanceof Inet4Address local)
                        || !(interfaceAddress.getBroadcast() instanceof Inet4Address broadcast)) {
                    continue;
                }
                candidates.add(new Endpoint(
                        networkInterface.getName(), local, broadcast, hardwareAddress.clone()
                ));
            }
        }
        if (requestedName != null) {
            return candidates.stream()
                    .filter(candidate -> candidate.interfaceName().equals(requestedName))
                    .findFirst()
                    .orElseThrow(() -> new SocketException(
                            "No active IPv4 broadcast address found on interface " + requestedName
                    ));
        }
        return candidates.stream()
                .sorted(Comparator.comparing((Endpoint endpoint) -> !endpoint.interfaceName().startsWith("en"))
                        .thenComparing(Endpoint::interfaceName))
                .findFirst()
                .orElseThrow(() -> new SocketException("No active IPv4 broadcast network interface found"));
    }

    private static void report(String packetType, Exception failure) {
        System.err.println("Pro DJ Link simulator could not send " + packetType + ": " + failure.getMessage());
    }

    @Override
    public void close() {
        if (!running.getAndSet(false)) {
            return;
        }
        scheduler.shutdownNow();
        socket.close();
    }

    record Endpoint(
            String interfaceName,
            Inet4Address localAddress,
            Inet4Address broadcastAddress,
            byte[] hardwareAddress
    ) {
        Endpoint {
            hardwareAddress = hardwareAddress.clone();
        }

        @Override
        public byte[] hardwareAddress() {
            return hardwareAddress.clone();
        }

        String localAddressText() {
            return localAddress.getHostAddress();
        }

        String broadcastAddressText() {
            return broadcastAddress.getHostAddress();
        }
    }
}
