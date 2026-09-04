package co.victorblan.tech.lumi.prolink.simulator;

import java.util.List;
import java.util.Objects;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

/**
 * Deterministically alternates two simulated players for unattended soak tests.
 * It changes only simulator state; no wall-clock work is added to Lumi itself.
 */
final class AutoMixController implements AutoCloseable {
    static final int MINIMUM_INTERVAL_SECONDS = 5;
    static final int MAXIMUM_INTERVAL_SECONDS = 3_600;

    private final List<PlayerState> players;
    private final ScheduledExecutorService scheduler = Executors.newSingleThreadScheduledExecutor(
            Thread.ofPlatform().name("lumi-prolink-auto-mix").daemon(true).factory()
    );
    private boolean enabled;
    private int intervalSeconds = 30;
    private long nextTransitionNanos;
    private long transitionCount;

    AutoMixController(List<PlayerState> players) {
        if (players.size() != 2) {
            throw new IllegalArgumentException("Auto Mix requires exactly two simulated players");
        }
        this.players = List.copyOf(players);
        scheduler.scheduleAtFixedRate(this::tickSafely, 100, 100, TimeUnit.MILLISECONDS);
    }

    synchronized void setEnabled(boolean requested, int requestedIntervalSeconds) {
        validateInterval(requestedIntervalSeconds);
        intervalSeconds = requestedIntervalSeconds;
        if (!requested) {
            enabled = false;
            nextTransitionNanos = 0;
            return;
        }
        requireLoadedTracks();
        PlayerState leader = currentLeader();
        PlayerState follower = other(leader);
        follower.pause();
        follower.setMaster(false);
        follower.setOnAir(false);
        leader.setOnAir(true);
        leader.setMaster(true);
        if (!leader.snapshot().playing()) {
            leader.restartForAutoMix();
        }
        enabled = true;
        nextTransitionNanos = System.nanoTime() + TimeUnit.SECONDS.toNanos(intervalSeconds);
    }

    synchronized Status status() {
        int leader = players.stream()
                .map(PlayerState::snapshot)
                .filter(PlayerState.Snapshot::master)
                .mapToInt(PlayerState.Snapshot::playerNumber)
                .findFirst()
                .orElse(0);
        long remainingMillis = enabled
                ? Math.max(0L, TimeUnit.NANOSECONDS.toMillis(nextTransitionNanos - System.nanoTime()))
                : 0L;
        return new Status(enabled, intervalSeconds, transitionCount, leader, remainingMillis);
    }

    synchronized void transitionNowForTesting() {
        requireLoadedTracks();
        transition();
        if (enabled) {
            nextTransitionNanos = System.nanoTime() + TimeUnit.SECONDS.toNanos(intervalSeconds);
        }
    }

    private void tickSafely() {
        try {
            synchronized (this) {
                if (!enabled || System.nanoTime() < nextTransitionNanos) {
                    return;
                }
                transition();
                nextTransitionNanos = System.nanoTime() + TimeUnit.SECONDS.toNanos(intervalSeconds);
            }
        } catch (RuntimeException failure) {
            synchronized (this) {
                enabled = false;
                nextTransitionNanos = 0;
            }
            System.err.println("Auto Mix stopped safely: " + failure.getMessage());
        }
    }

    private void transition() {
        PlayerState outgoing = currentLeader();
        PlayerState incoming = other(outgoing);

        // A real mixer handoff is observed over multiple network packets too.
        // Prepare the incoming transport first, then publish one exclusive
        // master and on-air state without ever leaving both players master.
        incoming.restartForAutoMix();
        incoming.setOnAir(true);
        outgoing.setMaster(false);
        incoming.setMaster(true);
        outgoing.setOnAir(false);
        outgoing.pause();
        transitionCount++;
    }

    private PlayerState currentLeader() {
        return players.stream()
                .filter(player -> player.snapshot().master())
                .findFirst()
                .orElse(players.getFirst());
    }

    private PlayerState other(PlayerState player) {
        return Objects.equals(players.getFirst(), player) ? players.get(1) : players.getFirst();
    }

    private void requireLoadedTracks() {
        if (players.stream().anyMatch(player -> player.snapshot().track() == null)) {
            throw new IllegalStateException("Load one USB track on both players before starting Auto Mix");
        }
    }

    private static void validateInterval(int seconds) {
        if (seconds < MINIMUM_INTERVAL_SECONDS || seconds > MAXIMUM_INTERVAL_SECONDS) {
            throw new IllegalArgumentException(
                    "Auto Mix interval must be between " + MINIMUM_INTERVAL_SECONDS
                            + " and " + MAXIMUM_INTERVAL_SECONDS + " seconds"
            );
        }
    }

    @Override
    public synchronized void close() {
        enabled = false;
        nextTransitionNanos = 0;
        scheduler.shutdownNow();
    }

    record Status(
            boolean enabled,
            int intervalSeconds,
            long transitionCount,
            int leaderPlayerNumber,
            long nextTransitionInMillis
    ) {
    }
}
