package co.victorblan.tech.lumi.prolink.simulator;

import java.util.Objects;

final class PlayerState {
    private final int playerNumber;
    private UsbLibrary.Track track;
    private boolean playing;
    private boolean master;
    private boolean onAir;
    private double pitchPercent;
    private double anchoredPositionMillis;
    private long anchorNanos = System.nanoTime();
    private long revision;

    PlayerState(int playerNumber) {
        this.playerNumber = playerNumber;
    }

    synchronized void load(UsbLibrary.Track nextTrack) {
        track = Objects.requireNonNull(nextTrack, "nextTrack");
        playing = false;
        pitchPercent = 0.0;
        anchoredPositionMillis = 0.0;
        anchorNanos = System.nanoTime();
        revision++;
    }

    synchronized void play() {
        requireTrack();
        capturePosition();
        if (anchoredPositionMillis >= track.durationMillis()) {
            anchoredPositionMillis = 0.0;
        }
        if (!playing) {
            playing = true;
            anchorNanos = System.nanoTime();
            revision++;
        }
    }

    synchronized void pause() {
        if (playing) {
            capturePosition();
            playing = false;
            revision++;
        }
    }

    synchronized void seek(long positionMillis) {
        requireTrack();
        anchoredPositionMillis = Math.max(0L, Math.min(positionMillis, track.durationMillis()));
        anchorNanos = System.nanoTime();
        revision++;
    }

    synchronized void setPitchPercent(double value) {
        if (!Double.isFinite(value) || value < -100.0 || value > 100.0) {
            throw new IllegalArgumentException("pitchPercent must be between -100 and 100");
        }
        capturePosition();
        pitchPercent = value;
        anchorNanos = System.nanoTime();
        revision++;
    }

    synchronized void setMaster(boolean value) {
        if (master != value) {
            master = value;
            revision++;
        }
    }

    synchronized void setOnAir(boolean value) {
        if (onAir != value) {
            onAir = value;
            revision++;
        }
    }

    synchronized Snapshot snapshot() {
        capturePosition();
        if (track != null && anchoredPositionMillis >= track.durationMillis() && playing) {
            anchoredPositionMillis = track.durationMillis();
            playing = false;
            revision++;
        }
        long position = Math.round(anchoredPositionMillis);
        int beatIndex = track == null ? -1 : track.beatIndexAt(position);
        UsbLibrary.BeatPoint beat = beatIndex < 0 ? null : track.beatGrid().get(beatIndex);
        int tempo = beat == null
                ? track == null ? 0 : track.originalTempoCentiBpm()
                : beat.tempoCentiBpm();
        return new Snapshot(
                playerNumber, track, playing, master, onAir, pitchPercent, position,
                beatIndex, beat, tempo, revision
        );
    }

    private void capturePosition() {
        long now = System.nanoTime();
        if (playing && track != null) {
            double elapsedMillis = (now - anchorNanos) / 1_000_000.0;
            anchoredPositionMillis += elapsedMillis * (1.0 + pitchPercent / 100.0);
        }
        anchorNanos = now;
    }

    private void requireTrack() {
        if (track == null) {
            throw new IllegalStateException("Load a USB track first");
        }
    }

    record Snapshot(
            int playerNumber,
            UsbLibrary.Track track,
            boolean playing,
            boolean master,
            boolean onAir,
            double pitchPercent,
            long positionMillis,
            int beatIndex,
            UsbLibrary.BeatPoint beat,
            int originalTempoCentiBpm,
            long revision
    ) {
        double effectiveBpm() {
            return originalTempoCentiBpm / 100.0 * (1.0 + pitchPercent / 100.0);
        }

        int beatNumber() {
            return beat == null ? 0 : beat.absoluteBeat();
        }

        int beatWithinBar() {
            return beat == null ? 0 : beat.beatWithinBar();
        }
    }
}
