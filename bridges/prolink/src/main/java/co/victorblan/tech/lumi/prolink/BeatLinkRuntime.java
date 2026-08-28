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
        BeatFinder.getInstance().addBeatListener(beat -> {
            if (!hasExactBeat(beat.getEffectiveTempo(), beat.getBeatWithinBar())) {
                return;
            }
            publisher.publishCritical(
                    "beat",
                    new BridgePayloads.Beat(
                            beat.getDeviceNumber(),
                            beat.getDeviceName(),
                            beat.getEffectiveTempo(),
                            beat.getBeatWithinBar(),
                            beat.isTempoMaster()
                    )
            );
        });
        // Modern players such as the CDJ-1500X publish their exact transport
        // position independently from the beat packet. Lumi uses this fact as
        // the sole authority for phrase changes and AutoLoop decisions. A beat
        // packet contains only the beat within the bar and cannot distinguish
        // normal playback from a hotcue jump before the next CdjStatus frame.
        BeatFinder.getInstance().addPrecisePositionListener(position -> {
            DeviceUpdate latestStatus = VirtualCdj.getInstance().getLatestStatusFor(position);
            if (latestStatus == null) {
                return;
            }
            int beatWithinBar = latestStatus.getBeatWithinBar();
            if (!hasRealtimeTempo(position.getEffectiveTempo(), beatWithinBar)) {
                return;
            }
            publisher.publishLatest(
                    BridgeTrafficClass.TRANSPORT,
                    position.getDeviceNumber(),
                    "precisePosition",
                    new BridgePayloads.PrecisePosition(
                            position.getDeviceNumber(),
                            position.getDeviceName(),
                            position.getPlaybackPosition(),
                            position.getEffectiveTempo(),
                            beatWithinBar,
                            position.isTempoMaster()
                    )
            );
        });
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
        ResolvedTrackIdentity trackIdentity = resolveTrackIdentity(status);
        // A player that has only just joined the network briefly reports the
        // Beat Link sentinel values (no BPM/beat yet). Those frames describe
        // normal device warm-up, not a bridge protocol failure. Wait for one
        // coherent loaded-track status before publishing it, while still
        // allowing a real unloaded status to clear an existing deck.
        if (trackIdentity.rekordboxId() != 0 && !hasCoherentLoadedTrack(status, trackIdentity)) {
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
                trackIdentity.sourcePlayer(),
                trackIdentity.sourceSlot(),
                trackIdentity.trackType(),
                trackIdentity.rekordboxId(),
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
        if (hasRealtimeTempo(status.getEffectiveTempo(), status.getBeatWithinBar())) {
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
    }

    private static boolean hasCoherentLoadedTrack(CdjStatus status, ResolvedTrackIdentity identity) {
        return identity.sourcePlayer() != 0
                && identity.rekordboxId() != 0
                && status.getBeatNumber() >= 0
                && hasRealtimeTempo(status.getBpm() / 100.0, status.getBeatWithinBar())
                && hasRealtimeTempo(status.getEffectiveTempo(), status.getBeatWithinBar());
    }

    /**
     * The CDJ-1500X status packet currently has a 512-byte extended layout.
     * Beat Link 8.0 understands its transport fields but reads the legacy
     * track-identity offsets, which remain zero while a loaded paused/cued
     * track is reported at the extended Rekordbox ID offset. Decode only this
     * exact, observed layout and otherwise retain Beat Link's interpretation.
     */
    static ResolvedTrackIdentity resolveTrackIdentity(CdjStatus status) {
        ResolvedTrackIdentity beatLinkIdentity = new ResolvedTrackIdentity(
                status.getTrackSourcePlayer(),
                status.getTrackSourceSlot().name(),
                status.getTrackType().name(),
                status.getRekordboxId()
        );
        if (beatLinkIdentity.rekordboxId() != 0) {
            return beatLinkIdentity;
        }
        return resolveCdj1500xExtendedTrackIdentity(
                status.getDeviceName(),
                status.getDeviceNumber(),
                status.getPacketBytes()
        );
    }

    static ResolvedTrackIdentity resolveCdj1500xExtendedTrackIdentity(
            String deviceName,
            int deviceNumber,
            byte[] packetBytes
    ) {
        if (!"CDJ-1500X".equals(deviceName) || packetBytes.length != 512) {
            return ResolvedTrackIdentity.noTrack();
        }
        int rekordboxId = ((packetBytes[0x194] & 0xff) << 24)
                | ((packetBytes[0x195] & 0xff) << 16)
                | ((packetBytes[0x196] & 0xff) << 8)
                | (packetBytes[0x197] & 0xff);
        if (rekordboxId == 0 || deviceNumber <= 0 || deviceNumber > 15) {
            return ResolvedTrackIdentity.noTrack();
        }
        return new ResolvedTrackIdentity(deviceNumber, "USB_SLOT", "REKORDBOX", rekordboxId);
    }

    record ResolvedTrackIdentity(int sourcePlayer, String sourceSlot, String trackType, int rekordboxId) {
        static ResolvedTrackIdentity noTrack() {
            return new ResolvedTrackIdentity(0, "NO_TRACK", "NO_TRACK", 0);
        }
    }

    static boolean hasRealtimeTempo(double effectiveTempo, int beatWithinBar) {
        return Double.isFinite(effectiveTempo)
                && effectiveTempo >= 20.0
                && effectiveTempo <= 300.0
                && beatWithinBar >= 0
                && beatWithinBar <= 4;
    }

    static boolean hasExactBeat(double effectiveTempo, int beatWithinBar) {
        return hasRealtimeTempo(effectiveTempo, beatWithinBar) && beatWithinBar >= 1;
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
