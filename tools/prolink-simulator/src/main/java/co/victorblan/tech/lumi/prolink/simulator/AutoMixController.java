package co.victorblan.tech.lumi.prolink.simulator;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Objects;
import java.util.Random;
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
    private final UsbLibrary library;
    private final Random random;
    private final ScheduledExecutorService scheduler = Executors.newSingleThreadScheduledExecutor(
            Thread.ofPlatform().name("lumi-prolink-auto-mix").daemon(true).factory()
    );
    private boolean enabled;
    private int intervalSeconds = 30;
    private long nextTransitionNanos;
    private long transitionCount;
    private Long playlistId;
    private String playlistPath;
    private boolean shuffle;
    private List<UsbLibrary.Track> playlistOrder = List.of();
    private int playlistCursor;
    private int lastAssignedTrackId = -1;

    AutoMixController(List<PlayerState> players, UsbLibrary library) {
        this(players, library, new Random());
    }

    AutoMixController(List<PlayerState> players, UsbLibrary library, Random random) {
        if (players.size() != 2) {
            throw new IllegalArgumentException("Auto Mix requires exactly two simulated players");
        }
        this.players = List.copyOf(players);
        this.library = Objects.requireNonNull(library, "library");
        this.random = Objects.requireNonNull(random, "random");
        scheduler.scheduleAtFixedRate(this::tickSafely, 100, 100, TimeUnit.MILLISECONDS);
    }

    synchronized void setEnabled(boolean requested, int requestedIntervalSeconds) {
        setEnabled(requested, requestedIntervalSeconds, null, false);
    }

    synchronized void setEnabled(
            boolean requested,
            int requestedIntervalSeconds,
            Long requestedPlaylistId,
            boolean requestedShuffle
    ) {
        validateInterval(requestedIntervalSeconds);
        intervalSeconds = requestedIntervalSeconds;
        if (!requested) {
            enabled = false;
            nextTransitionNanos = 0;
            return;
        }
        PlayerState leader;
        if (requestedPlaylistId == null) {
            clearPlaylistMode();
            requireLoadedTracks();
            leader = currentLeader();
        } else {
            preparePlaylist(requestedPlaylistId, requestedShuffle);
            leader = players.getFirst();
            leader.load(nextPlaylistTrack());
            other(leader).load(nextPlaylistTrack());
        }
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
        PlayerState prepared = leader == 0 ? null : other(player(leader));
        PlayerState.Snapshot preparedSnapshot = prepared == null ? null : prepared.snapshot();
        return new Status(
                enabled,
                intervalSeconds,
                transitionCount,
                leader,
                remainingMillis,
                playlistId == null ? "manual" : "playlist",
                playlistId,
                playlistPath,
                playlistOrder.size(),
                shuffle,
                preparedSnapshot == null ? 0 : preparedSnapshot.playerNumber(),
                preparedSnapshot == null || preparedSnapshot.track() == null
                        ? null
                        : UsbLibrary.TrackSummary.from(preparedSnapshot.track())
        );
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
        if (playlistId != null) {
            outgoing.load(nextPlaylistTrack());
        }
        transitionCount++;
    }

    private void preparePlaylist(long requestedPlaylistId, boolean requestedShuffle) {
        UsbLibrary.Playlist playlist = library.requirePlaylist(requestedPlaylistId);
        LinkedHashMap<Integer, UsbLibrary.Track> unique = new LinkedHashMap<>();
        for (UsbLibrary.Track track : playlist.tracks()) {
            unique.putIfAbsent(track.id(), track);
        }
        if (unique.size() < 2) {
            throw new IllegalStateException("Auto Mix requires at least two different tracks in the playlist");
        }
        playlistId = playlist.id();
        playlistPath = playlist.path();
        shuffle = requestedShuffle;
        playlistOrder = List.copyOf(unique.values());
        playlistCursor = 0;
        lastAssignedTrackId = -1;
        if (shuffle) {
            reshufflePlaylist();
        }
    }

    private UsbLibrary.Track nextPlaylistTrack() {
        if (playlistCursor >= playlistOrder.size()) {
            playlistCursor = 0;
            if (shuffle) {
                reshufflePlaylist();
            }
        }
        UsbLibrary.Track track = playlistOrder.get(playlistCursor++);
        lastAssignedTrackId = track.id();
        return track;
    }

    private void reshufflePlaylist() {
        ArrayList<UsbLibrary.Track> shuffled = new ArrayList<>(playlistOrder);
        Collections.shuffle(shuffled, random);
        if (shuffled.size() > 1 && shuffled.getFirst().id() == lastAssignedTrackId) {
            Collections.swap(shuffled, 0, 1);
        }
        playlistOrder = List.copyOf(shuffled);
    }

    private void clearPlaylistMode() {
        playlistId = null;
        playlistPath = null;
        playlistOrder = List.of();
        playlistCursor = 0;
        lastAssignedTrackId = -1;
        shuffle = false;
    }

    private PlayerState currentLeader() {
        return players.stream()
                .filter(player -> player.snapshot().master())
                .findFirst()
                .orElse(players.getFirst());
    }

    private PlayerState player(int playerNumber) {
        return players.stream()
                .filter(player -> player.snapshot().playerNumber() == playerNumber)
                .findFirst()
                .orElseThrow(() -> new IllegalStateException("Unknown Auto Mix leader " + playerNumber));
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
            long nextTransitionInMillis,
            String mode,
            Long playlistId,
            String playlistPath,
            int playlistTrackCount,
            boolean shuffle,
            int preparedPlayerNumber,
            UsbLibrary.TrackSummary preparedTrack
    ) {
    }
}
