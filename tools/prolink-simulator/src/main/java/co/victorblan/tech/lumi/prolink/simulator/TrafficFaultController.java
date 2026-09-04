package co.victorblan.tech.lumi.prolink.simulator;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Objects;
import java.util.Set;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.function.LongSupplier;

/**
 * Applies opt-in, deterministic faults at the simulator transport boundary.
 *
 * <p>The authoritative Player state keeps advancing while traffic is withheld.
 * This models an actual network interruption and guarantees that recovery does
 * not fabricate a seek, reload or second clock authority.</p>
 */
final class TrafficFaultController implements AutoCloseable {
    static final int MINIMUM_DURATION_MILLIS = 250;
    static final int MAXIMUM_DURATION_MILLIS = 60_000;
    static final int MINIMUM_SOAK_INTERVAL_SECONDS = 8;
    static final int MAXIMUM_SOAK_INTERVAL_SECONDS = 3_600;

    enum Lane {
        ANNOUNCEMENT,
        STATUS,
        BEAT,
        PRECISE_POSITION,
        TIMING,
        ALL;

        static Lane parse(String value) {
            if (value == null || value.isBlank()) {
                throw new IllegalArgumentException("lane is required");
            }
            return switch (value.trim().toLowerCase(Locale.ROOT)) {
                case "announcement" -> ANNOUNCEMENT;
                case "status" -> STATUS;
                case "beat" -> BEAT;
                case "precise-position", "precise_position", "precise" -> PRECISE_POSITION;
                case "timing" -> TIMING;
                case "all" -> ALL;
                default -> throw new IllegalArgumentException("Unknown traffic lane " + value);
            };
        }

        String externalName() {
            return switch (this) {
                case ANNOUNCEMENT -> "announcement";
                case STATUS -> "status";
                case BEAT -> "beat";
                case PRECISE_POSITION -> "precise-position";
                case TIMING -> "timing";
                case ALL -> "all";
            };
        }

        boolean matches(Lane actual) {
            return this == ALL || this == actual
                    || (this == TIMING && actual != ANNOUNCEMENT);
        }
    }

    private final List<PlayerState> players;
    private final LongSupplier nanoTime;
    private final Runnable masterHandover;
    private final ScheduledExecutorService scheduler;
    private final Set<Integer> manuallyOffline = new HashSet<>();
    private final List<ActiveFault> activeFaults = new ArrayList<>();
    private long nextFaultID = 1;
    private long totalSuppressedPackets;
    private boolean recoverySoakEnabled;
    private int recoverySoakIntervalSeconds = 20;
    private long nextRecoveryEventNanos;
    private long recoveryEventCount;
    private String lastRecoveryEvent;

    TrafficFaultController(List<PlayerState> players, AutoMixController autoMix) {
        this(players, System::nanoTime, true, autoMix::transitionNowForTesting);
    }

    TrafficFaultController(List<PlayerState> players, LongSupplier nanoTime, boolean runScheduler) {
        this(players, nanoTime, runScheduler, null);
    }

    private TrafficFaultController(
            List<PlayerState> players,
            LongSupplier nanoTime,
            boolean runScheduler,
            Runnable masterHandover
    ) {
        this.players = List.copyOf(players);
        this.nanoTime = Objects.requireNonNull(nanoTime, "nanoTime");
        this.masterHandover = masterHandover;
        scheduler = Executors.newSingleThreadScheduledExecutor(
                Thread.ofPlatform().name("lumi-prolink-recovery-soak").daemon(true).factory()
        );
        if (runScheduler) {
            scheduler.scheduleAtFixedRate(this::tickRecoverySoakSafely, 100, 100, TimeUnit.MILLISECONDS);
        }
    }

    synchronized boolean permit(int playerNumber, Lane lane) {
        requirePlayer(playerNumber);
        long now = nanoTime.getAsLong();
        removeExpired(now);
        if (manuallyOffline.contains(playerNumber)) {
            totalSuppressedPackets++;
            return false;
        }
        for (ActiveFault fault : activeFaults) {
            if (fault.playerNumber == playerNumber && fault.lane.matches(lane)) {
                fault.attempts++;
                if (fault.attempts % fault.everyN == 0) {
                    fault.suppressedPackets++;
                    totalSuppressedPackets++;
                    return false;
                }
            }
        }
        return true;
    }

    synchronized void setPlayerOnline(int playerNumber, boolean online) {
        requirePlayer(playerNumber);
        if (online) {
            manuallyOffline.remove(playerNumber);
        } else {
            manuallyOffline.add(playerNumber);
        }
    }

    synchronized void startPositionGap(int playerNumber, int durationMillis) {
        startFault("position-gap", playerNumber, Lane.PRECISE_POSITION, 1, durationMillis);
    }

    synchronized void startDisconnect(int playerNumber, int durationMillis) {
        startFault("temporary-disconnect", playerNumber, Lane.ALL, 1, durationMillis);
    }

    synchronized void startPacketLoss(
            int playerNumber,
            Lane lane,
            int everyN,
            int durationMillis
    ) {
        if (everyN < 2 || everyN > 100) {
            throw new IllegalArgumentException("everyN must be between 2 and 100");
        }
        startFault("packet-loss", playerNumber, lane, everyN, durationMillis);
    }

    synchronized void clearFaults() {
        manuallyOffline.clear();
        activeFaults.clear();
        recoverySoakEnabled = false;
        nextRecoveryEventNanos = 0;
        lastRecoveryEvent = null;
    }

    synchronized void setRecoverySoak(boolean enabled, int intervalSeconds) {
        if (intervalSeconds < MINIMUM_SOAK_INTERVAL_SECONDS
                || intervalSeconds > MAXIMUM_SOAK_INTERVAL_SECONDS) {
            throw new IllegalArgumentException(
                    "Recovery Soak interval must be between " + MINIMUM_SOAK_INTERVAL_SECONDS
                            + " and " + MAXIMUM_SOAK_INTERVAL_SECONDS + " seconds"
            );
        }
        if (enabled) {
            requireRecoveryReady();
        }
        recoverySoakIntervalSeconds = intervalSeconds;
        recoverySoakEnabled = enabled;
        nextRecoveryEventNanos = enabled
                ? nanoTime.getAsLong() + TimeUnit.SECONDS.toNanos(intervalSeconds)
                : 0;
        if (!enabled) {
            lastRecoveryEvent = null;
        }
    }

    synchronized Status status() {
        long now = nanoTime.getAsLong();
        removeExpired(now);
        List<Integer> offline = manuallyOffline.stream().sorted().toList();
        List<FaultStatus> faults = activeFaults.stream()
                .sorted(Comparator.comparingLong(fault -> fault.id))
                .map(fault -> new FaultStatus(
                        fault.id,
                        fault.kind,
                        fault.playerNumber,
                        fault.lane.externalName(),
                        fault.everyN,
                        Math.max(0L, TimeUnit.NANOSECONDS.toMillis(fault.endsAtNanos - now)),
                        fault.suppressedPackets
                ))
                .toList();
        long nextEventMillis = recoverySoakEnabled
                ? Math.max(0L, TimeUnit.NANOSECONDS.toMillis(nextRecoveryEventNanos - now))
                : 0;
        return new Status(
                offline,
                faults,
                totalSuppressedPackets,
                new RecoverySoakStatus(
                        recoverySoakEnabled,
                        recoverySoakIntervalSeconds,
                        recoveryEventCount,
                        nextEventMillis,
                        lastRecoveryEvent
                )
        );
    }

    synchronized void runRecoveryEventForTesting() {
        runRecoveryEvent();
    }

    private void startFault(
            String kind,
            int playerNumber,
            Lane lane,
            int everyN,
            int durationMillis
    ) {
        requirePlayer(playerNumber);
        validateDuration(durationMillis);
        long now = nanoTime.getAsLong();
        removeExpired(now);
        activeFaults.add(new ActiveFault(
                nextFaultID++, kind, playerNumber, lane, everyN,
                now + TimeUnit.MILLISECONDS.toNanos(durationMillis)
        ));
    }

    private void tickRecoverySoakSafely() {
        try {
            synchronized (this) {
                if (!recoverySoakEnabled || nanoTime.getAsLong() < nextRecoveryEventNanos) {
                    return;
                }
                runRecoveryEvent();
                nextRecoveryEventNanos = nanoTime.getAsLong()
                        + TimeUnit.SECONDS.toNanos(recoverySoakIntervalSeconds);
            }
        } catch (RuntimeException failure) {
            synchronized (this) {
                recoverySoakEnabled = false;
                nextRecoveryEventNanos = 0;
                lastRecoveryEvent = "Stopped safely: " + failure.getMessage();
            }
            System.err.println("Recovery Soak stopped safely: " + failure.getMessage());
        }
    }

    private void runRecoveryEvent() {
        requireRecoveryReady();
        PlayerState master = currentMaster();
        int playerNumber = master.snapshot().playerNumber();
        int phase = (int) (recoveryEventCount % 5);
        switch (phase) {
            case 0 -> {
                startPositionGap(playerNumber, 1_500);
                lastRecoveryEvent = "Player " + playerNumber + " exact-position gap (1.5 s)";
            }
            case 1 -> {
                startPacketLoss(playerNumber, Lane.TIMING, 4, 3_000);
                lastRecoveryEvent = "Player " + playerNumber + " timing packet loss (1 in 4)";
            }
            case 2 -> {
                startDisconnect(playerNumber, 2_500);
                lastRecoveryEvent = "Player " + playerNumber + " temporary disconnect (2.5 s)";
            }
            case 3 -> {
                master.jumpBeats(32);
                lastRecoveryEvent = "Player " + playerNumber + " beat jump (+32)";
            }
            default -> {
                int incomingPlayerNumber = other(master).snapshot().playerNumber();
                if (masterHandover == null) {
                    handoverMasterDirectly(master);
                } else {
                    masterHandover.run();
                }
                lastRecoveryEvent = "Master handover to Player " + incomingPlayerNumber;
            }
        }
        recoveryEventCount++;
    }

    private void requireRecoveryReady() {
        if (players.size() != 2 || players.stream().anyMatch(player -> player.snapshot().track() == null)) {
            throw new IllegalStateException(
                    "Recovery Soak requires one loaded track on both simulated Players"
            );
        }
        if (!manuallyOffline.isEmpty()) {
            throw new IllegalStateException(
                    "Bring both simulated Players online before starting Recovery Soak"
            );
        }
        long masterCount = players.stream().filter(player -> player.snapshot().master()).count();
        if (masterCount != 1) {
            throw new IllegalStateException(
                    "Select exactly one Master before starting Recovery Soak"
            );
        }
    }

    private PlayerState currentMaster() {
        return players.stream()
                .filter(player -> player.snapshot().master())
                .findFirst()
                .orElse(players.getFirst());
    }

    private PlayerState other(PlayerState selected) {
        return players.stream()
                .filter(player -> player != selected)
                .findFirst()
                .orElseThrow(() -> new IllegalStateException("Recovery Soak requires two Players"));
    }

    private void handoverMasterDirectly(PlayerState master) {
        PlayerState incoming = other(master);
        if (incoming.snapshot().track() == null) {
            throw new IllegalStateException("Load a track on both Players before a Master handover");
        }
        if (!incoming.snapshot().playing()) {
            incoming.play();
        }
        incoming.setOnAir(true);
        master.setMaster(false);
        incoming.setMaster(true);
        master.setOnAir(false);
    }

    private void requirePlayer(int playerNumber) {
        if (players.stream().noneMatch(player -> player.snapshot().playerNumber() == playerNumber)) {
            throw new IllegalArgumentException("Unknown player number " + playerNumber);
        }
    }

    private static void validateDuration(int durationMillis) {
        if (durationMillis < MINIMUM_DURATION_MILLIS || durationMillis > MAXIMUM_DURATION_MILLIS) {
            throw new IllegalArgumentException(
                    "durationMillis must be between " + MINIMUM_DURATION_MILLIS
                            + " and " + MAXIMUM_DURATION_MILLIS
            );
        }
    }

    private void removeExpired(long now) {
        activeFaults.removeIf(fault -> fault.endsAtNanos <= now);
    }

    @Override
    public synchronized void close() {
        recoverySoakEnabled = false;
        nextRecoveryEventNanos = 0;
        activeFaults.clear();
        manuallyOffline.clear();
        scheduler.shutdownNow();
    }

    private static final class ActiveFault {
        private final long id;
        private final String kind;
        private final int playerNumber;
        private final Lane lane;
        private final int everyN;
        private final long endsAtNanos;
        private long attempts;
        private long suppressedPackets;

        private ActiveFault(
                long id,
                String kind,
                int playerNumber,
                Lane lane,
                int everyN,
                long endsAtNanos
        ) {
            this.id = id;
            this.kind = kind;
            this.playerNumber = playerNumber;
            this.lane = lane;
            this.everyN = everyN;
            this.endsAtNanos = endsAtNanos;
        }
    }

    record FaultStatus(
            long id,
            String kind,
            int playerNumber,
            String lane,
            int everyN,
            long remainingMillis,
            long suppressedPackets
    ) {
    }

    record RecoverySoakStatus(
            boolean enabled,
            int intervalSeconds,
            long eventCount,
            long nextEventInMillis,
            String lastEvent
    ) {
    }

    record Status(
            List<Integer> manuallyOfflinePlayers,
            List<FaultStatus> activeFaults,
            long totalSuppressedPackets,
            RecoverySoakStatus recoverySoak
    ) {
    }
}
