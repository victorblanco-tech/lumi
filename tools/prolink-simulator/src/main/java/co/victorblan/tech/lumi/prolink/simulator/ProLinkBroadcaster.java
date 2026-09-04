package co.victorblan.tech.lumi.prolink.simulator;

import java.io.IOException;
import java.net.DatagramPacket;
import java.net.DatagramSocket;
import java.net.InetAddress;
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
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.AtomicReference;

final class ProLinkBroadcaster implements AutoCloseable, SimulatorTransport {
    private static final int ANNOUNCEMENT_PORT = 50_000;
    private static final int BEAT_PORT = 50_001;
    private static final int STATUS_PORT = 50_002;
    private static final String DEVICE_NAME_PREFIX = "LUMI-SIM-";
    private static final long PEER_LEASE_NANOS = TimeUnit.SECONDS.toNanos(6);

    private final List<PlayerState> states;
    private final ProLinkTrafficProfile trafficProfile;
    private final Endpoint endpoint;
    private final DatagramSocket socket;
    private final DatagramSocket announcementSocket;
    private final ProLinkPeerRegistry peers = new ProLinkPeerRegistry(PEER_LEASE_NANOS);
    private final Thread peerDiscoveryThread;
    private final ScheduledExecutorService scheduler = Executors.newScheduledThreadPool(
            5, Thread.ofPlatform().name("lumi-prolink-simulator-", 0).daemon(true).factory()
    );
    private final AtomicInteger packetCounter = new AtomicInteger();
    private final AtomicBoolean running = new AtomicBoolean();
    private final AtomicBoolean closed = new AtomicBoolean();
    private final AtomicLong announcementPacketCount = new AtomicLong();
    private final AtomicLong statusPacketCount = new AtomicLong();
    private final AtomicLong beatPacketCount = new AtomicLong();
    private final AtomicLong precisePositionPacketCount = new AtomicLong();
    private final AtomicLong preciseBurstCount = new AtomicLong();
    private final AtomicReference<String> lastTrafficError = new AtomicReference<>();
    private final int[] lastBeatIndex = new int[5];
    private final long[] lastRevision = new long[5];
    private final boolean[] lastPlaying = new boolean[5];

    ProLinkBroadcaster(
            List<PlayerState> states,
            String requestedInterface,
            ProLinkTrafficProfile trafficProfile
    ) throws IOException {
        this.states = List.copyOf(Objects.requireNonNull(states, "states"));
        if (this.states.isEmpty()) {
            throw new IllegalArgumentException("At least one simulated player is required");
        }
        this.trafficProfile = Objects.requireNonNull(trafficProfile, "trafficProfile");
        java.util.Arrays.fill(lastBeatIndex, Integer.MIN_VALUE);
        java.util.Arrays.fill(lastRevision, Long.MIN_VALUE);
        this.endpoint = selectEndpoint(requestedInterface);
        this.socket = new DatagramSocket(new InetSocketAddress(endpoint.localAddress(), 0));
        socket.setBroadcast(true);
        this.announcementSocket = new DatagramSocket(null);
        announcementSocket.setReuseAddress(true);
        announcementSocket.bind(new InetSocketAddress(ANNOUNCEMENT_PORT));
        announcementSocket.setBroadcast(true);
        this.peerDiscoveryThread = Thread.ofPlatform()
                .name("lumi-prolink-simulator-peer-discovery")
                .daemon(true)
                .unstarted(this::receivePeerAnnouncements);
    }

    void start() {
        if (closed.get()) {
            throw new IllegalStateException("The Pro DJ Link broadcaster is already closed");
        }
        if (!running.compareAndSet(false, true)) {
            return;
        }
        peerDiscoveryThread.start();
        scheduler.scheduleAtFixedRate(this::sendAnnouncementSafely, 0, 1_500, TimeUnit.MILLISECONDS);
        scheduler.scheduleAtFixedRate(
                this::sendStatusSafely,
                0,
                trafficProfile.statusIntervalMillis(),
                TimeUnit.MILLISECONDS
        );
        scheduler.scheduleAtFixedRate(this::sendBeatWhenDueSafely, 0, 5, TimeUnit.MILLISECONDS);
        if (trafficProfile.publishesPrecisePosition()) {
            scheduler.scheduleAtFixedRate(
                    this::sendPrecisePositionSafely,
                    0,
                    trafficProfile.precisePositionIntervalMillis(),
                    TimeUnit.MILLISECONDS
            );
        }
        if (trafficProfile.publishesBursts()) {
            scheduler.scheduleAtFixedRate(
                    this::sendPreciseBurstSafely,
                    trafficProfile.burstIntervalMillis(),
                    trafficProfile.burstIntervalMillis(),
                    TimeUnit.MILLISECONDS
            );
        }
    }

    @Override
    public Endpoint endpoint() {
        return endpoint;
    }

    private void sendAnnouncementSafely() {
        try {
            for (PlayerState state : states) {
                PlayerState.Snapshot snapshot = state.snapshot();
                DatagramPacket packet = ProLinkPackets.announcement(
                        deviceName(snapshot.playerNumber()), snapshot.playerNumber(),
                        hardwareAddress(snapshot.playerNumber()), endpoint.localAddress(),
                        states.size() + peers.size(System.nanoTime())
                );
                send(packet, ANNOUNCEMENT_PORT);
                announcementPacketCount.incrementAndGet();
            }
        } catch (Exception failure) {
            report("announcement", failure);
        }
    }

    private void sendStatusSafely() {
        try {
            for (PlayerState state : states) {
                PlayerState.Snapshot snapshot = state.snapshot();
                DatagramPacket packet = ProLinkPackets.status(
                        deviceName(snapshot.playerNumber()), snapshot, packetCounter.incrementAndGet()
                );
                send(packet, STATUS_PORT);
                sendStatusToPeers(packet);
                statusPacketCount.incrementAndGet();
            }
        } catch (Exception failure) {
            report("status", failure);
        }
    }

    private void sendBeatWhenDueSafely() {
        try {
            for (PlayerState state : states) {
                PlayerState.Snapshot snapshot = state.snapshot();
                int player = snapshot.playerNumber();
                if (snapshot.revision() != lastRevision[player]) {
                    lastRevision[player] = snapshot.revision();
                    lastBeatIndex[player] = snapshot.beatIndex();
                }
                if (!snapshot.playing() || snapshot.beatIndex() < 0) {
                    lastPlaying[player] = false;
                    continue;
                }
                if (!lastPlaying[player] || snapshot.beatIndex() != lastBeatIndex[player]) {
                    lastPlaying[player] = true;
                    lastBeatIndex[player] = snapshot.beatIndex();
                    send(ProLinkPackets.beat(deviceName(player), snapshot), BEAT_PORT);
                    beatPacketCount.incrementAndGet();
                }
            }
        } catch (Exception failure) {
            report("beat", failure);
        }
    }

    private void sendPrecisePositionSafely() {
        try {
            for (PlayerState state : states) {
                PlayerState.Snapshot snapshot = state.snapshot();
                if (snapshot.track() == null) {
                    continue;
                }
                send(
                        ProLinkPackets.precisePosition(
                                deviceName(snapshot.playerNumber()), snapshot, snapshot.positionMillis()
                        ),
                        BEAT_PORT
                );
                precisePositionPacketCount.incrementAndGet();
            }
        } catch (Exception failure) {
            report("precise position", failure);
        }
    }

    private void sendPreciseBurstSafely() {
        try {
            for (PlayerState state : states) {
                sendPreciseBurst(state);
            }
        } catch (Exception failure) {
            report("precise position burst", failure);
        }
    }

    @Override
    public void triggerPreciseBurst(int playerNumber) {
        if (!trafficProfile.publishesBursts()) {
            throw new IllegalStateException(
                    "The " + trafficProfile.externalName() + " profile does not publish position bursts"
            );
        }
        PlayerState state = states.stream()
                .filter(candidate -> candidate.snapshot().playerNumber() == playerNumber)
                .findFirst()
                .orElseThrow(() -> new IllegalArgumentException("Unknown player number " + playerNumber));
        scheduler.execute(() -> {
            try {
                sendPreciseBurst(state);
            } catch (Exception failure) {
                report("precise position burst", failure);
            }
        });
    }

    private void sendPreciseBurst(PlayerState state) throws IOException {
        PlayerState.Snapshot snapshot = state.snapshot();
        if (snapshot.track() == null || !snapshot.playing()) {
            return;
        }
        long millisPerBeat = Math.max(
                1L,
                Math.round(60_000.0 / Math.max(1.0, snapshot.effectiveBpm()))
        );
        long rewindMillis = millisPerBeat * trafficProfile.burstRewindBeats();
        long stalePosition = Math.max(0L, snapshot.positionMillis() - rewindMillis);
        String deviceName = deviceName(snapshot.playerNumber());
        for (int index = 0; index < trafficProfile.burstPacketCount(); index++) {
            long position = Math.min(
                    snapshot.positionMillis(),
                    stalePosition + index * trafficProfile.precisePositionIntervalMillis()
            );
            send(ProLinkPackets.precisePosition(deviceName, snapshot, position), BEAT_PORT);
            precisePositionPacketCount.incrementAndGet();
        }
        // End every burst with a current observation so consumers which
        // correctly keep only the latest value recover immediately.
        send(
                ProLinkPackets.precisePosition(deviceName, snapshot, snapshot.positionMillis()),
                BEAT_PORT
        );
        precisePositionPacketCount.incrementAndGet();
        preciseBurstCount.incrementAndGet();
    }

    private void send(DatagramPacket packet, int port) throws IOException {
        send(packet, endpoint.broadcastAddress(), port);
    }

    private void send(DatagramPacket packet, InetAddress address, int port) throws IOException {
        socket.send(new DatagramPacket(
                packet.getData(), packet.getOffset(), packet.getLength(), address, port
        ));
    }

    private void sendStatusToPeers(DatagramPacket packet) throws IOException {
        for (InetAddress peer : peers.active(System.nanoTime())) {
            send(packet, peer, STATUS_PORT);
        }
    }

    private void receivePeerAnnouncements() {
        byte[] buffer = new byte[512];
        DatagramPacket packet = new DatagramPacket(buffer, buffer.length);
        while (running.get()) {
            try {
                packet.setLength(buffer.length);
                announcementSocket.receive(packet);
                if (ProLinkPackets.hasMagicHeader(packet)
                        && !packet.getAddress().equals(endpoint.localAddress())) {
                    peers.observe(packet.getAddress(), System.nanoTime());
                }
            } catch (IOException failure) {
                if (running.get()) {
                    report("peer discovery", failure);
                }
            }
        }
    }

    @Override
    public int peerCount() {
        return peers.size(System.nanoTime());
    }

    @Override
    public TrafficDiagnostics trafficDiagnostics() {
        return new TrafficDiagnostics(
                trafficProfile.externalName(),
                trafficProfile.statusIntervalMillis(),
                trafficProfile.precisePositionIntervalMillis(),
                announcementPacketCount.get(),
                statusPacketCount.get(),
                beatPacketCount.get(),
                precisePositionPacketCount.get(),
                preciseBurstCount.get(),
                lastTrafficError.get()
        );
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

    private void report(String packetType, Exception failure) {
        lastTrafficError.set(packetType + ": " + failure.getMessage());
        System.err.println("Pro DJ Link simulator could not send " + packetType + ": " + failure.getMessage());
    }

    private String deviceName(int playerNumber) {
        return DEVICE_NAME_PREFIX + playerNumber;
    }

    private byte[] hardwareAddress(int playerNumber) {
        byte[] address = endpoint.hardwareAddress();
        address[address.length - 1] = (byte) (address[address.length - 1] ^ playerNumber);
        return address;
    }

    @Override
    public void close() {
        if (!closed.compareAndSet(false, true)) {
            return;
        }
        running.set(false);
        scheduler.shutdownNow();
        announcementSocket.close();
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

    record TrafficDiagnostics(
            String profile,
            long statusIntervalMillis,
            long precisePositionIntervalMillis,
            long announcementPackets,
            long statusPackets,
            long beatPackets,
            long precisePositionPackets,
            long preciseBursts,
            String lastError
    ) {
    }
}
