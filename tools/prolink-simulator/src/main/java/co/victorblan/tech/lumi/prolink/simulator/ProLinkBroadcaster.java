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
    private final TrafficFaultController faults;
    private final Endpoint endpoint;
    private final DatagramSocket socket;
    private final java.util.Map<Integer, Endpoint> playerEndpoints = new java.util.HashMap<>();
    private final java.util.Map<Integer, DatagramSocket> playerSockets = new java.util.HashMap<>();
    private final java.util.Set<Integer> unavailable = java.util.concurrent.ConcurrentHashMap.newKeySet();
    private volatile MediaRpcService media;
    void attachMedia(MediaRpcService service) { media = service; }
    @Override public String mediaStatus() { return media == null ? "Not running" : media.status(); }
    @Override public void clearMediaFaults() { if (media != null) media.clearFaults(); }
    @Override public void mediaFault(int player, String kind, int durationMillis) {
        if (media == null) throw new IllegalStateException("Media service not running");
        media.fault(player, kind, durationMillis);
    }
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
            ProLinkTrafficProfile trafficProfile,
            TrafficFaultController faults
    ) throws IOException {
        this.states = List.copyOf(Objects.requireNonNull(states, "states"));
        if (this.states.isEmpty()) {
            throw new IllegalArgumentException("At least one simulated player is required");
        }
        this.trafficProfile = Objects.requireNonNull(trafficProfile, "trafficProfile");
        this.faults = Objects.requireNonNull(faults, "faults");
        java.util.Arrays.fill(lastBeatIndex, Integer.MIN_VALUE);
        java.util.Arrays.fill(lastRevision, Long.MIN_VALUE);
        this.endpoint = selectEndpoint(requestedInterface);
        this.socket = new DatagramSocket(new InetSocketAddress(endpoint.localAddress(), 0));
        this.announcementSocket = new DatagramSocket(null);
        try {
        socket.setBroadcast(true);
        Endpoint secondary = secondaryEndpoint(endpoint);
        for (int i = 0; i < states.size(); i++) {
            int player = states.get(i).snapshot().playerNumber();
            Endpoint selected = i == 1 && secondary != null ? secondary : endpoint;
            playerEndpoints.put(player, selected);
            if (selected.equals(endpoint)) playerSockets.put(player, socket);
            else {
                DatagramSocket additional = new DatagramSocket(new InetSocketAddress(selected.localAddress(), 0));
                additional.setBroadcast(true);
                playerSockets.put(player, additional);
            }
        }
        announcementSocket.setReuseAddress(true);
        announcementSocket.bind(new InetSocketAddress(ANNOUNCEMENT_PORT));
        announcementSocket.setBroadcast(true);
        } catch (IOException | RuntimeException failure) {
            socket.close(); announcementSocket.close(); playerSockets.values().forEach(DatagramSocket::close);
            scheduler.shutdownNow(); throw failure;
        }
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

    @Override public Endpoint endpointForPlayer(int player) { return playerEndpoints.getOrDefault(player, endpoint); }

    boolean independentSources() { return playerEndpoints.values().stream().map(Endpoint::localAddressText).distinct().count() > 1; }

    private void sendAnnouncementSafely() {
        try {
            for (PlayerState state : states) {
                PlayerState.Snapshot snapshot = state.snapshot();
                int number = snapshot.playerNumber();
                if (NetworkInterface.getByInetAddress(endpointForPlayer(number).localAddress()) == null) {
                    unavailable.add(number);
                    lastTrafficError.set("Player " + number + " address unavailable; restart simulator if the address changed");
                    continue;
                }
                unavailable.remove(number);
                if (!faults.permit(
                        snapshot.playerNumber(), TrafficFaultController.Lane.ANNOUNCEMENT
                )) {
                    continue;
                }
                DatagramPacket packet = ProLinkPackets.announcement(
                        deviceName(snapshot.playerNumber()), snapshot.playerNumber(),
                        hardwareAddress(snapshot.playerNumber()), endpointForPlayer(snapshot.playerNumber()).localAddress(),
                        states.size() + peers.size(System.nanoTime())
                );
                send(packet, ANNOUNCEMENT_PORT, snapshot.playerNumber());
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
                if (!faults.permit(snapshot.playerNumber(), TrafficFaultController.Lane.STATUS)) {
                    continue;
                }
                DatagramPacket packet = ProLinkPackets.status(
                        deviceName(snapshot.playerNumber()), snapshot, packetCounter.incrementAndGet()
                );
                send(packet, STATUS_PORT, snapshot.playerNumber());
                sendStatusToPeers(packet, snapshot.playerNumber());
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
                    if (!faults.permit(player, TrafficFaultController.Lane.BEAT)) {
                        continue;
                    }
                    send(ProLinkPackets.beat(deviceName(player), snapshot), BEAT_PORT, player);
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
                if (!faults.permit(
                        snapshot.playerNumber(), TrafficFaultController.Lane.PRECISE_POSITION
                )) {
                    continue;
                }
                send(
                        ProLinkPackets.precisePosition(
                                deviceName(snapshot.playerNumber()), snapshot, snapshot.positionMillis()
                        ),
                        BEAT_PORT, snapshot.playerNumber()
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
            if (faults.permit(
                    snapshot.playerNumber(), TrafficFaultController.Lane.PRECISE_POSITION
            )) {
                send(ProLinkPackets.precisePosition(deviceName, snapshot, position), BEAT_PORT, snapshot.playerNumber());
                precisePositionPacketCount.incrementAndGet();
            }
        }
        // End every burst with a current observation so consumers which
        // correctly keep only the latest value recover immediately.
        if (faults.permit(
                snapshot.playerNumber(), TrafficFaultController.Lane.PRECISE_POSITION
        )) {
            send(
                    ProLinkPackets.precisePosition(deviceName, snapshot, snapshot.positionMillis()),
                    BEAT_PORT, snapshot.playerNumber()
            );
            precisePositionPacketCount.incrementAndGet();
        }
        preciseBurstCount.incrementAndGet();
    }

    private void send(DatagramPacket packet, int port, int player) throws IOException {
        send(packet, endpointForPlayer(player).broadcastAddress(), port, player);
    }

    private void send(DatagramPacket packet, InetAddress address, int port, int player) throws IOException {
        // Never silently move a Player to another address while a USB identity is cached.
        if (unavailable.contains(player)) return;
        playerSockets.get(player).send(new DatagramPacket(
                packet.getData(), packet.getOffset(), packet.getLength(), address, port
        ));
    }

    private void sendStatusToPeers(DatagramPacket packet, int player) throws IOException {
        for (InetAddress peer : peers.active(System.nanoTime())) {
            send(packet, peer, STATUS_PORT, player);
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
                        && playerEndpoints.values().stream().noneMatch(e -> e.localAddress().equals(packet.getAddress()))) {
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

    static List<Endpoint> availableEndpoints() throws SocketException {
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
                        || !(interfaceAddress.getBroadcast() instanceof Inet4Address broadcast)
                        || !(local.isSiteLocalAddress() || local.isLinkLocalAddress())) {
                    continue;
                }
                candidates.add(new Endpoint(
                        networkInterface.getName(), local, broadcast, hardwareAddress.clone()
                ));
            }
        }
        return candidates;
    }

    static Endpoint selectEndpoint(String requestedName) throws SocketException {
        List<Endpoint> candidates = availableEndpoints();
        if (requestedName != null) {
            return candidates.stream()
                    .filter(candidate -> candidate.interfaceName().equals(requestedName))
                    .findFirst()
                    .orElseThrow(() -> new SocketException(
                            "No active IPv4 broadcast address found on interface " + requestedName
                    ));
        }
        String preferred = defaultInterface();
        return candidates.stream()
                .sorted(Comparator.comparing((Endpoint endpoint) -> !endpoint.interfaceName().equals(preferred))
                        .thenComparing(Endpoint::interfaceName))
                .findFirst()
                .orElseThrow(() -> new SocketException("No active IPv4 broadcast network interface found"));
    }

    static Endpoint secondaryEndpoint(Endpoint primary) throws SocketException {
        return availableEndpoints().stream().filter(e -> suitableSecondary(primary, e))
                .sorted(Comparator.comparing(Endpoint::interfaceName)).findFirst().orElse(null);
    }

    static boolean suitableSecondary(Endpoint primary, Endpoint candidate) {
        return !primary.interfaceName().equals(candidate.interfaceName())
                && !primary.localAddress().equals(candidate.localAddress())
                && primary.broadcastAddress().equals(candidate.broadcastAddress());
    }

    private static String defaultInterface() {
        Process process = null;
        try {
            process = new ProcessBuilder("/sbin/route", "-n", "get", "default").redirectErrorStream(true).start();
            if (!process.waitFor(2, TimeUnit.SECONDS)) return "";
            String text = new String(process.getInputStream().readNBytes(8192), java.nio.charset.StandardCharsets.UTF_8);
            for (String line : text.split("\\n")) if (line.trim().startsWith("interface:")) return line.split(":", 2)[1].trim();
        } catch (IOException ignored) {
        } catch (InterruptedException interrupted) { Thread.currentThread().interrupt();
        } finally { if (process != null && process.isAlive()) process.destroyForcibly(); }
        return "";
    }

    private void report(String packetType, Exception failure) {
        lastTrafficError.set(packetType + ": " + failure.getMessage());
        System.err.println("Pro DJ Link simulator could not send " + packetType + ": " + failure.getMessage());
    }

    private String deviceName(int playerNumber) {
        return DEVICE_NAME_PREFIX + playerNumber;
    }

    private byte[] hardwareAddress(int playerNumber) {
        byte[] address = endpointForPlayer(playerNumber).hardwareAddress();
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
        playerSockets.values().forEach(DatagramSocket::close);
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
