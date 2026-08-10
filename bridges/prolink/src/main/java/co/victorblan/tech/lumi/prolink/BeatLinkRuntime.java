package co.victorblan.tech.lumi.prolink;

import org.deepsymmetry.beatlink.BeatFinder;
import org.deepsymmetry.beatlink.CdjStatus;
import org.deepsymmetry.beatlink.DeviceAnnouncement;
import org.deepsymmetry.beatlink.DeviceAnnouncementListener;
import org.deepsymmetry.beatlink.DeviceFinder;
import org.deepsymmetry.beatlink.DeviceUpdate;
import org.deepsymmetry.beatlink.VirtualCdj;
import org.deepsymmetry.beatlink.data.MetadataFinder;
import org.deepsymmetry.beatlink.data.SearchableItem;
import org.deepsymmetry.beatlink.data.SignatureFinder;
import org.deepsymmetry.beatlink.data.TrackMetadata;

import java.util.Objects;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicBoolean;

final class BeatLinkRuntime implements AutoCloseable {
    private final BridgePublisher publisher;
    private final AtomicBoolean started = new AtomicBoolean();
    private final AtomicBoolean sessionStarting = new AtomicBoolean();
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
        BeatFinder.getInstance().addBeatListener(beat -> publisher.publish(
                "beat",
                new BridgePayloads.Beat(
                        beat.getDeviceNumber(),
                        beat.getDeviceName(),
                        beat.getEffectiveTempo(),
                        beat.getBeatWithinBar(),
                        beat.isTempoMaster()
                )
        ));
        MetadataFinder.getInstance().addTrackMetadataListener(this::metadataChanged);
        SignatureFinder.getInstance().addSignatureListener(update -> publisher.publish(
                "trackSignature",
                new BridgePayloads.TrackSignature(update.player, update.signature)
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
                SignatureFinder.getInstance().start();
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
        publisher.publish("deckStatus", new BridgePayloads.DeckStatus(
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
        ));
    }

    private void metadataChanged(org.deepsymmetry.beatlink.data.TrackMetadataUpdate update) {
        TrackMetadata metadata = update.metadata;
        if (metadata == null) {
            publisher.publish("trackMetadata", new BridgePayloads.TrackMetadata(
                    update.player, false, null, null, null, null,
                    null, null, null, null, null, null
            ));
            return;
        }
        publisher.publish("trackMetadata", new BridgePayloads.TrackMetadata(
                update.player,
                true,
                metadata.trackReference.player,
                metadata.trackReference.slot.name(),
                metadata.trackType.name(),
                metadata.trackReference.rekordboxId,
                metadata.getTitle(),
                label(metadata.getArtist()),
                metadata.getDuration(),
                metadata.getTempo() / 100.0,
                label(metadata.getKey()),
                metadata.getColor() == null ? null : metadata.getColor().label
        ));
    }

    private static String label(SearchableItem item) {
        return item == null ? null : item.label;
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
        SignatureFinder.getInstance().stop();
        MetadataFinder.getInstance().stop();
        BeatFinder.getInstance().stop();
        VirtualCdj.getInstance().stop();
        DeviceFinder.getInstance().stop();
        publisher.publish("sourceStatus", new BridgePayloads.SourceStatus(
                "stopped", "Direct Pro DJ Link bridge stopped"
        ));
    }
}
