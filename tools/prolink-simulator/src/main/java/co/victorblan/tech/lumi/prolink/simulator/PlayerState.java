package co.victorblan.tech.lumi.prolink.simulator;

import java.util.Objects;
import java.util.function.LongSupplier;

final class PlayerState {
    private final int playerNumber;
    private final LongSupplier nanoTime;
    private final MediaSlot usb;
    private MediaSlot loadedSlot;
    private MediaSlot.Mount loadedMount;
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
        usb = new MediaSlot(playerNumber);
        this.nanoTime = Objects.requireNonNull(nanoTime, "nanoTime");
        anchorNanos = nanoTime.getAsLong();
    }

    synchronized void load(UsbLibrary.Track nextTrack) {
        loadedSlot = null;
        loadedMount = null;
        if (usb.current() != null) {
            MediaSlot.Mount mount = usb.requireMounted();
            if (mount.library().requireTrack(nextTrack.id()) != nextTrack) {
                throw new IllegalArgumentException("Track does not belong to this USB");
            }
            loadedSlot = usb;
            loadedMount = mount;
        }
        resetTrack(nextTrack);
    }

    MediaSlot usb() { return usb; }

    void configureUsb(UsbLibrary library) { usb.configure(library); }

    synchronized void loadFrom(MediaSlot source, int trackId) {
        MediaSlot.Mount mount = source.requireMounted();
        UsbLibrary.Track next = mount.library().requireTrack(trackId);
        loadedSlot = source;
        loadedMount = mount;
        resetTrack(next);
    }

    synchronized void unload() {
        track = null;
        loadedSlot = null;
        loadedMount = null;
        playing = false;
        loopEnabled = false;
        anchoredPositionMillis = 0;
        anchorNanos = nanoTime.getAsLong();
        revision++;
    }

    private void resetTrack(UsbLibrary.Track nextTrack) {
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

    synchronized void jumpBeats(int beats) {
        requireTrack();
        if (beats == 0) {
            throw new IllegalArgumentException("beats must not be zero");
        }
        capturePosition();
        if (track.beatGrid().isEmpty()) {
            throw new IllegalStateException("The loaded track has no beat grid");
        }
        int currentIndex = Math.max(0, track.beatIndexAt(Math.round(anchoredPositionMillis)));
        int targetIndex = Math.max(0, Math.min(currentIndex + beats, track.beatGrid().size() - 1));
        anchoredPositionMillis = track.beatGrid().get(targetIndex).timeMillis();
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
                loopWrapCount, revision, usb.current(), loadedMount,
                loadedMount != null && !loadedSlot.stillPresent(loadedMount)
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
            long revision,
            MediaSlot.Mount insertedUsb,
            MediaSlot.Mount loadedFrom,
            boolean cachedAfterEject
    ) {
        int sourcePlayerNumber() {
            return loadedFrom == null ? playerNumber : loadedFrom.playerNumber();
        }

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
