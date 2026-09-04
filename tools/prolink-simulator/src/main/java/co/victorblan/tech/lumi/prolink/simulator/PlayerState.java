package co.victorblan.tech.lumi.prolink.simulator;

import java.util.Objects;
import java.util.function.LongSupplier;

final class PlayerState {
    private final int playerNumber;
    private final LongSupplier nanoTime;
    private UsbLibrary.Track track;
    private boolean playing;
    private boolean master;
    private boolean onAir;
    private double pitchPercent;
    private double anchoredPositionMillis;
    private long anchorNanos;
    private boolean loopEnabled;
    private long loopStartMillis;
    private long loopEndMillis;
    private long loopWrapCount;
    private long revision;

    PlayerState(int playerNumber) {
        this(playerNumber, System::nanoTime);
    }

    PlayerState(int playerNumber, LongSupplier nanoTime) {
        this.playerNumber = playerNumber;
        this.nanoTime = Objects.requireNonNull(nanoTime, "nanoTime");
        anchorNanos = nanoTime.getAsLong();
    }

    synchronized void load(UsbLibrary.Track nextTrack) {
        track = Objects.requireNonNull(nextTrack, "nextTrack");
        playing = false;
        pitchPercent = 0.0;
        anchoredPositionMillis = 0.0;
        anchorNanos = nanoTime.getAsLong();
        loopEnabled = false;
        loopStartMillis = 0;
        loopEndMillis = 0;
        loopWrapCount = 0;
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
            anchorNanos = nanoTime.getAsLong();
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
        anchorNanos = nanoTime.getAsLong();
        revision++;
    }

    synchronized void setPitchPercent(double value) {
        if (!Double.isFinite(value) || value < -100.0 || value > 100.0) {
            throw new IllegalArgumentException("pitchPercent must be between -100 and 100");
        }
        capturePosition();
        pitchPercent = value;
        anchorNanos = nanoTime.getAsLong();
        revision++;
    }

    synchronized void setLoop(long startMillis, long endMillis) {
        requireTrack();
        if (startMillis < 0 || endMillis > track.durationMillis() || endMillis <= startMillis) {
            throw new IllegalArgumentException(
                    "Loop start must be before loop end and both must fit inside the loaded track"
            );
        }
        capturePosition();
        loopStartMillis = startMillis;
        loopEndMillis = endMillis;
        loopEnabled = true;
        loopWrapCount = 0;
        revision++;
    }

    synchronized void disableLoop() {
        if (loopEnabled) {
            capturePosition();
            loopEnabled = false;
            revision++;
        }
    }

    synchronized void restartForAutoMix() {
        requireTrack();
        anchoredPositionMillis = loopEnabled ? loopStartMillis : 0.0;
        anchorNanos = nanoTime.getAsLong();
        playing = true;
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
                beatIndex, beat, tempo, loopEnabled, loopStartMillis, loopEndMillis,
                loopWrapCount, revision
        );
    }

    private void capturePosition() {
        long now = nanoTime.getAsLong();
        if (playing && track != null) {
            double elapsedMillis = (now - anchorNanos) / 1_000_000.0;
            anchoredPositionMillis += elapsedMillis * (1.0 + pitchPercent / 100.0);
            if (loopEnabled && anchoredPositionMillis >= loopEndMillis) {
                double loopLength = loopEndMillis - loopStartMillis;
                double elapsedInsideLoop = anchoredPositionMillis - loopStartMillis;
                long completedLoops = Math.max(1L, (long) Math.floor(elapsedInsideLoop / loopLength));
                anchoredPositionMillis = loopStartMillis + elapsedInsideLoop % loopLength;
                loopWrapCount += completedLoops;
                revision++;
            }
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
            boolean loopEnabled,
            long loopStartMillis,
            long loopEndMillis,
            long loopWrapCount,
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
