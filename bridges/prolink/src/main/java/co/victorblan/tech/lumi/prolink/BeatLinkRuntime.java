package co.victorblan.tech.lumi.prolink;

import org.deepsymmetry.beatlink.BeatFinder;
import org.deepsymmetry.beatlink.CdjStatus;
import org.deepsymmetry.beatlink.DeviceAnnouncement;
import org.deepsymmetry.beatlink.DeviceAnnouncementListener;
import org.deepsymmetry.beatlink.DeviceFinder;
import org.deepsymmetry.beatlink.DeviceUpdate;
import org.deepsymmetry.beatlink.VirtualCdj;
import java.util.Objects;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicBoolean;

final class BeatLinkRuntime implements AutoCloseable {
    private final BridgePublisher publisher;
    private final AtomicBoolean started = new AtomicBoolean();
    private final AtomicBoolean sessionStarting = new AtomicBoolean();
    private final Map<Integer, TransportFingerprint> transportFingerprints = new ConcurrentHashMap<>();
    private final ExecutorService lifecycleExecutor = Executors.newSingleThreadExecutor(
            Thread.ofPlatform().name("lumi-prolink-lifecycle-", 0).factory()
    );

    private final DeviceAnnouncementListener deviceListener = new DeviceAnnouncementListener() {
        @Override
        public void deviceFound(DeviceAnnouncement announcement) {
            publishDevice("deviceFound", announcement);
            ensureFullSession();
        }

        @Override
        public void deviceLost(DeviceAnnouncement announcement) {
            transportFingerprints.remove(announcement.getDeviceNumber());
            publishDevice("deviceLost", announcement);
        }
    };

    BeatLinkRuntime(BridgePublisher publisher) {
        this.publisher = Objects.requireNonNull(publisher, "publisher");
    }

    void start() throws Exception {
        if (!started.compareAndSet(false, true)) {
            return;
        }
        publisher.publish("sourceStatus", new BridgePayloads.SourceStatus(
                "starting", "Starting read-only Pro DJ Link discovery"
        ));

        DeviceFinder.getInstance().addDeviceAnnouncementListener(deviceListener);
        VirtualCdj.getInstance().addUpdateListener(this::receivedDeviceUpdate);
        BeatFinder.getInstance().addBeatListener(beat -> publisher.publishCritical(
                "beat",
                new BridgePayloads.Beat(
                        beat.getDeviceNumber(),
                        beat.getDeviceName(),
                        beat.getEffectiveTempo(),
                        beat.getBeatWithinBar(),
                        beat.isTempoMaster()
                )
        ));
        // Modern players such as the CDJ-1500X publish their exact transport
        // position independently from the beat packet. Lumi uses this fact as
        // the sole authority for phrase changes and AutoLoop decisions. A beat
        // packet contains only the beat within the bar and cannot distinguish
        // normal playback from a hotcue jump before the next CdjStatus frame.
        BeatFinder.getInstance().addPrecisePositionListener(position -> publisher.publishLatest(
                BridgeTrafficClass.TRANSPORT,
                position.getDeviceNumber(),
                "precisePosition",
                new BridgePayloads.PrecisePosition(
                        position.getDeviceNumber(),
                        position.getDeviceName(),
                        position.getPlaybackPosition(),
                        position.getEffectiveTempo(),
                        position.getBeatWithinBar(),
                        position.isTempoMaster()
                )
        ));
        DeviceFinder.getInstance().start();
        publisher.publish("sourceStatus", new BridgePayloads.SourceStatus(
                "discovering", "Waiting for a supported Pro DJ Link device"
        ));
        DeviceFinder.getInstance().getCurrentDevices().forEach(this::publishExistingDevice);
        if (!DeviceFinder.getInstance().getCurrentDevices().isEmpty()) {
            ensureFullSession();
        }
    }

    private void publishExistingDevice(DeviceAnnouncement announcement) {
        publishDevice("deviceFound", announcement);
    }

    private void ensureFullSession() {
        if (!started.get() || !sessionStarting.compareAndSet(false, true)) {
            return;
        }
        lifecycleExecutor.submit(() -> {
            boolean sessionReady = false;
            try {
                VirtualCdj.getInstance().setDeviceName("Lumi");
                if (!VirtualCdj.getInstance().start()) {
                    throw new IllegalStateException("Beat Link could not claim a virtual player session");
                }
                BeatFinder.getInstance().start();
                // Lumi resolves track content from its trusted, read-only USB mirror.
                // Starting SignatureFinder also starts Beat Link's active metadata,
                // waveform and beat-grid queries. On a fully occupied four-player
                // network those queries cannot claim a player number and retry in a
                // tight loop. Deck status already supplies the mounted-player/slot
                // and Rekordbox ID needed by Lumi, so keep this realtime bridge
                // passive with respect to media content.
                sessionReady = true;
                publisher.publish("sourceStatus", new BridgePayloads.SourceStatus(
                        "ready", "Direct Pro DJ Link session is receiving deck status and beat data"
                ));
            } catch (Exception failure) {
                publishFailure("startSession", failure);
                publisher.publish("sourceStatus", new BridgePayloads.SourceStatus(
                        "degraded", "Device discovery works, but the rich player session failed"
                ));
            } finally {
                if (!sessionReady) {
                    sessionStarting.set(false);
                }
            }
        });
    }

    private void publishDevice(String type, DeviceAnnouncement announcement) {
        publisher.publish(type, new BridgePayloads.Device(
                announcement.getDeviceNumber(),
                announcement.getDeviceName(),
                announcement.getAddress().getHostAddress()
        ));
    }

    private void receivedDeviceUpdate(DeviceUpdate update) {
        if (!(update instanceof CdjStatus status)) {
            return;
        }
        BridgePayloads.DeckStatus payload = new BridgePayloads.DeckStatus(
                status.getDeviceNumber(),
                status.getDeviceName(),
                status.isPlaying(),
                status.isPaused(),
                status.isCued(),
                status.isTempoMaster(),
                status.isOnAir(),
                status.getTrackSourcePlayer(),
                status.getTrackSourceSlot().name(),
                status.getTrackType().name(),
                status.getRekordboxId(),
                status.getBpm() / 100.0,
                status.getEffectiveTempo(),
                status.getBeatNumber(),
                status.getBeatWithinBar(),
                status.getPitch()
        );
        TransportFingerprint fingerprint = TransportFingerprint.from(payload);
        TransportFingerprint previous = transportFingerprints.put(status.getDeviceNumber(), fingerprint);
        if (!fingerprint.equals(previous)) {
            publisher.publishCritical("transportStatus", payload);
        } else {
            publisher.publishLatest(
                    BridgeTrafficClass.TRANSPORT,
                    status.getDeviceNumber(),
                    "deckStatus",
                    payload
            );
        }
        publisher.publishLatest(
                BridgeTrafficClass.TEMPO,
                status.getDeviceNumber(),
                "tempoStatus",
                new BridgePayloads.TempoStatus(
                        status.getDeviceNumber(),
                        status.getDeviceName(),
                        status.getEffectiveTempo(),
                        status.getBeatWithinBar(),
                        status.isTempoMaster(),
                        status.isPlaying()
                )
        );
    }

    private void publishFailure(String operation, Exception failure) {
        String message = failure.getMessage() == null
                ? failure.getClass().getSimpleName()
                : failure.getMessage();
        System.err.println("Pro Link bridge " + operation + " failed: " + message);
        publisher.publish("error", new BridgePayloads.Error(operation, message));
    }

    @Override
    public void close() {
        if (!started.getAndSet(false)) {
            return;
        }
        lifecycleExecutor.shutdownNow();
        BeatFinder.getInstance().stop();
        VirtualCdj.getInstance().stop();
        DeviceFinder.getInstance().stop();
        publisher.publish("sourceStatus", new BridgePayloads.SourceStatus(
                "stopped", "Direct Pro DJ Link bridge stopped"
        ));
    }

    private record TransportFingerprint(
            boolean playing,
            boolean paused,
            boolean cued,
            boolean tempoMaster,
            boolean onAir,
            int sourcePlayer,
            String sourceSlot,
            String trackType,
            int rekordboxId
    ) {
        static TransportFingerprint from(BridgePayloads.DeckStatus status) {
            return new TransportFingerprint(
                    status.playing(),
                    status.paused(),
                    status.cued(),
                    status.tempoMaster(),
                    status.onAir(),
                    status.sourcePlayer(),
                    status.sourceSlot(),
                    status.trackType(),
                    status.rekordboxId()
            );
        }
    }
}
