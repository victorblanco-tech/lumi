package co.victorblan.tech.lumi.prolink;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.Test;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.io.IOException;
import java.io.OutputStream;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

final class BridgePublisherTest {
    private final ObjectMapper mapper = new ObjectMapper();

    @Test
    void nativeUsbMountStatesAreIndependentOfLoadedTrackAndTempo() {
        assertEquals("loaded", BeatLinkRuntime.usbMountState(true, false, false));
        assertEquals("unloading", BeatLinkRuntime.usbMountState(false, true, false));
        assertEquals("empty", BeatLinkRuntime.usbMountState(false, false, true));
        assertEquals("unknown", BeatLinkRuntime.usbMountState(false, false, false));
        assertFalse(BeatLinkRuntime.mountContinuityLost(null, 10_000_000_000L));
        assertFalse(BeatLinkRuntime.mountContinuityLost(1_000L, 500_001_000L));
        assertTrue(BeatLinkRuntime.mountContinuityLost(1_000L, 3_000_001_001L));
    }

    @Test
    void ejectAndInsertionCannotBeCoalescedIntoOnlyTheLatestMount() throws Exception {
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        BridgePublisher publisher = new BridgePublisher(output, mapper);
        for (String state : new String[]{"loaded", "unloading", "empty", "loaded"}) {
            assertTrue(publisher.publishCritical("usbMount", new BridgePayloads.USBMount(1, state)));
        }
        publisher.close();
        String[] lines = output.toString(StandardCharsets.UTF_8).strip().split("\\R");
        assertEquals(4, lines.length);
        assertEquals("unloading", mapper.readTree(lines[1]).get("payload").get("state").asText());
        assertEquals("empty", mapper.readTree(lines[2]).get("payload").get("state").asText());
        assertEquals("critical", mapper.readTree(lines[3]).get("trafficClass").asText());
    }

    @Test
    void rejectsTransientPlayerWarmupTempoWithoutRejectingValidDeckTempo() {
        assertFalse(BeatLinkRuntime.hasRealtimeTempo(-0.01, 0));
        assertFalse(BeatLinkRuntime.hasRealtimeTempo(0.0, 0));
        assertFalse(BeatLinkRuntime.hasRealtimeTempo(Double.NaN, 1));
        assertTrue(BeatLinkRuntime.hasRealtimeTempo(155.0, 0));
        assertTrue(BeatLinkRuntime.hasRealtimeTempo(155.0, 4));
        assertFalse(BeatLinkRuntime.hasExactBeat(155.0, 0));
        assertTrue(BeatLinkRuntime.hasExactBeat(155.0, 1));
    }

    @Test
    void decodesLoadedPausedTrackIdentityFromCdj1500xExtendedStatus() {
        byte[] playerOneStatus = new byte[512];
        playerOneStatus[0x194] = 0x00;
        playerOneStatus[0x195] = 0x00;
        playerOneStatus[0x196] = 0x04;
        playerOneStatus[0x197] = (byte)0xe8;

        BeatLinkRuntime.ResolvedTrackIdentity identity =
                BeatLinkRuntime.resolveCdj1500xExtendedTrackIdentity("CDJ-1500X", 1, playerOneStatus);

        assertEquals(1, identity.sourcePlayer());
        assertEquals("USB_SLOT", identity.sourceSlot());
        assertEquals("REKORDBOX", identity.trackType());
        assertEquals(1256, identity.rekordboxId());
    }

    @Test
    void cdj1500xExtendedStatusKeepsTrueNoTrackAndUnknownLayoutsEmpty() {
        byte[] noTrackStatus = new byte[512];
        assertEquals(
                BeatLinkRuntime.ResolvedTrackIdentity.noTrack(),
                BeatLinkRuntime.resolveCdj1500xExtendedTrackIdentity("CDJ-1500X", 2, noTrackStatus)
        );

        byte[] knownIdAtUnsupportedLength = new byte[513];
        knownIdAtUnsupportedLength[0x196] = 0x04;
        knownIdAtUnsupportedLength[0x197] = (byte)0xd5;
        assertEquals(
                BeatLinkRuntime.ResolvedTrackIdentity.noTrack(),
                BeatLinkRuntime.resolveCdj1500xExtendedTrackIdentity(
                        "CDJ-1500X",
                        2,
                        knownIdAtUnsupportedLength
                )
        );
        assertEquals(
                BeatLinkRuntime.ResolvedTrackIdentity.noTrack(),
                BeatLinkRuntime.resolveCdj1500xExtendedTrackIdentity("CDJ-3000", 2, noTrackStatus)
        );
    }

    @Test
    void extendedCachedTrackKeepsConfirmedLinkOriginAfterSourceUsbEject() {
        var noTrack = BeatLinkRuntime.ResolvedTrackIdentity.noTrack();
        var linked = new BeatLinkRuntime.ResolvedTrackIdentity(1, "USB_SLOT", "REKORDBOX", 1031);
        var inferredLocal = new BeatLinkRuntime.ResolvedTrackIdentity(2, "USB_SLOT", "REKORDBOX", 1031);
        assertEquals(linked, BeatLinkRuntime.resolveLoadedTrackIdentity(noTrack, inferredLocal, linked));
        assertEquals(inferredLocal, BeatLinkRuntime.resolveLoadedTrackIdentity(noTrack, inferredLocal, null));
        // A native source change is explicit evidence even if numeric IDs collide.
        assertEquals(inferredLocal, BeatLinkRuntime.resolveLoadedTrackIdentity(inferredLocal, inferredLocal, linked));
        var newTrack = new BeatLinkRuntime.ResolvedTrackIdentity(2, "USB_SLOT", "REKORDBOX", 2048);
        assertEquals(newTrack, BeatLinkRuntime.resolveLoadedTrackIdentity(noTrack, newTrack, linked));
        assertEquals(noTrack, BeatLinkRuntime.resolveLoadedTrackIdentity(noTrack, noTrack, linked));
        // Once unloaded, a later track must not inherit the previous LINK source.
        assertEquals(inferredLocal, BeatLinkRuntime.resolveLoadedTrackIdentity(noTrack, inferredLocal, noTrack));
    }

    @Test
    void publishesVersionedMonotoneNdjsonWithoutDroppedEvents() throws Exception {
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        BridgePublisher publisher = new BridgePublisher(output, mapper);

        assertTrue(publisher.publish("hello", new BridgePayloads.Hello("0.4.0-dev-20", "8.0.0", true)));
        assertTrue(publisher.publish("sourceStatus", new BridgePayloads.SourceStatus("starting", "test")));
        publisher.close();

        String[] lines = output.toString(StandardCharsets.UTF_8).strip().split("\\R");
        assertEquals(2, lines.length);

        JsonNode hello = mapper.readTree(lines[0]);
        JsonNode status = mapper.readTree(lines[1]);
        assertEquals("lumi-prolink-bridge", hello.get("protocol").asText());
        assertEquals(1, hello.get("protocolVersion").asInt());
        assertEquals(1, hello.get("sequence").asLong());
        assertEquals("critical", hello.get("trafficClass").asText());
        assertTrue(hello.get("bridgeQueueAgeMicros").asLong() >= 0);
        assertEquals("hello", hello.get("type").asText());
        assertEquals("8.0.0", hello.get("payload").get("beatLinkVersion").asText());
        assertEquals(2, status.get("sequence").asLong());
        assertEquals("sourceStatus", status.get("type").asText());
        assertEquals(0, publisher.droppedEvents());
    }

    @Test
    void fiftyThousandDisplaySamplesCannotQueueBehindCriticalTraffic() throws Exception {
        GatedOutputStream output = new GatedOutputStream();
        BridgePublisher publisher = new BridgePublisher(output, mapper);
        assertTrue(publisher.publishCritical("hello", new BridgePayloads.Hello("dev", "8.0.0", true)));
        assertTrue(output.awaitBlocked());

        for (int index = 0; index < 50_000; index++) {
            assertTrue(publisher.publishLatest(
                    BridgeTrafficClass.DISPLAY,
                    1,
                    "precisePosition",
                    new BridgePayloads.PrecisePosition(1, "Player 1", index, 140.0, 1, true)
            ));
        }
        assertEquals(49_999, publisher.coalescedContinuousCount());
        output.release();
        publisher.close();

        String[] lines = output.content().strip().split("\\R");
        assertEquals(2, lines.length);
        JsonNode latest = mapper.readTree(lines[1]);
        assertEquals("display", latest.get("trafficClass").asText());
        assertEquals(49_999, latest.get("payload").get("playbackPositionMillis").asLong());
        assertEquals(0, publisher.criticalSaturationCount());
    }

    private static final class GatedOutputStream extends OutputStream {
        private final ByteArrayOutputStream delegate = new ByteArrayOutputStream();
        private final CountDownLatch blocked = new CountDownLatch(1);
        private final CountDownLatch released = new CountDownLatch(1);
        private boolean firstWrite = true;

        @Override
        public synchronized void write(int value) throws IOException {
            awaitReleaseOnce();
            delegate.write(value);
        }

        @Override
        public synchronized void write(byte[] bytes, int offset, int length) throws IOException {
            awaitReleaseOnce();
            delegate.write(bytes, offset, length);
        }

        private void awaitReleaseOnce() throws IOException {
            if (!firstWrite) {
                return;
            }
            firstWrite = false;
            blocked.countDown();
            try {
                if (!released.await(2, TimeUnit.SECONDS)) {
                    throw new IOException("test output was not released");
                }
            } catch (InterruptedException interrupted) {
                Thread.currentThread().interrupt();
                throw new IOException(interrupted);
            }
        }

        boolean awaitBlocked() throws InterruptedException {
            return blocked.await(2, TimeUnit.SECONDS);
        }

        void release() {
            released.countDown();
        }

        synchronized String content() {
            return delegate.toString(StandardCharsets.UTF_8);
        }
    }
}
