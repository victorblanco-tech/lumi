package co.victorblan.tech.lumi.prolink;

import org.junit.jupiter.api.Test;
import org.deepsymmetry.beatlink.CdjStatus;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.io.ByteArrayOutputStream;
import java.net.DatagramPacket;
import java.net.DatagramSocket;
import java.net.InetAddress;
import java.nio.charset.StandardCharsets;
import java.util.HexFormat;
import static org.junit.jupiter.api.Assertions.assertEquals;

final class LoadedTrackOriginTrackerTest {
    private static final BeatLinkRuntime.ResolvedTrackIdentity LINK = identity(1, 1031);
    private static final BeatLinkRuntime.ResolvedTrackIdentity LOCAL = identity(2, 1031);

    // Only captured status facts: neither private IP/MAC nor an invented full
    // hardware packet. Unused bytes are zero in these constructed fixtures.
    private static byte[] packet(boolean cached) {
        byte[] packet = new byte[1152];
        packet[0x20] = 8;
        packet[0x22] = 4;
        packet[0x23] = 0x5c;
        packet[0x125] = (byte)(cached ? 4 : 0);
        byte[] block = HexFormat.of().parseHex(
                "010000ff00000000017aefc600000000000000000100010000000001010200020001"
                + "000000000407000000050000006500000000000000000000001500000033");
        System.arraycopy(block, 0, packet, 0x170, block.length);
        return packet;
    }

    private static BeatLinkRuntime.ResolvedTrackIdentity identity(int player, int id) {
        return new BeatLinkRuntime.ResolvedTrackIdentity(player, "USB_SLOT", "REKORDBOX", id);
    }

    private static BeatLinkRuntime.ResolvedTrackIdentity observe(
            LoadedTrackOriginTracker tracker, byte[] packet, int state,
            BeatLinkRuntime.ResolvedTrackIdentity reported
    ) {
        return tracker.observe("CDJ-1500X", "1.10", 2, state, packet, reported, true);
    }

    @Test void actualCapturedCachedOwnershipChangePreservesLinkOrigin() {
        var tracker = new LoadedTrackOriginTracker();
        assertEquals(LINK, observe(tracker, packet(false), 6, LINK));
        for (int i = 0; i < 10_000; i++) {
            assertEquals(LINK, observe(tracker, packet(true), 6, LOCAL));
        }
        // Play, hotcue and pause on the same cached load do not replace origin.
        assertEquals(LINK, observe(tracker, packet(true), 3, LOCAL));
        assertEquals(LINK, observe(tracker, packet(true), 5, LOCAL));
        assertEquals(LINK, observe(tracker, packet(false), 6, LINK));
    }

    @Test void noSourceRebaseWithoutObservedCacheEvidence() {
        var tracker = new LoadedTrackOriginTracker();
        observe(tracker, packet(false), 6, LINK);
        assertEquals(LOCAL, observe(tracker, packet(false), 6, LOCAL));
        assertEquals(LOCAL, observe(new LoadedTrackOriginTracker(), packet(true), 6, LOCAL));
    }

    @Test void loadingResetsOriginEvenWhenIdAndExtendedBlockCollide() {
        var tracker = new LoadedTrackOriginTracker();
        observe(tracker, packet(false), 6, LINK);
        observe(tracker, packet(true), 6, LOCAL);
        // Loading is seen before coherent tempo is available.
        tracker.observe("CDJ-1500X", "1.10", 2, 2, packet(true), LOCAL, false);
        assertEquals(LOCAL, observe(tracker, packet(true), 6, LOCAL));
    }

    @Test void unloadAndDeviceLossNeverTransferAnOldOriginToANewLoad() {
        var tracker = new LoadedTrackOriginTracker();
        observe(tracker, packet(false), 6, LINK);
        assertEquals(BeatLinkRuntime.ResolvedTrackIdentity.noTrack(),
                observe(tracker, new byte[1152], 0, BeatLinkRuntime.ResolvedTrackIdentity.noTrack()));
        assertEquals(LOCAL, observe(tracker, packet(true), 6, LOCAL));
        observe(tracker, packet(false), 6, LINK);
        tracker.forget(2);
        assertEquals(LOCAL, observe(tracker, packet(true), 6, LOCAL));
    }

    @Test void differentTrackContentOrIdIsNotPinnedToOldSource() {
        var tracker = new LoadedTrackOriginTracker();
        observe(tracker, packet(false), 6, LINK);
        byte[] changed = packet(true);
        changed[0x19f] ^= 1;
        assertEquals(LOCAL, observe(tracker, changed, 6, LOCAL));
        observe(tracker, packet(false), 6, LINK);
        assertEquals(identity(2, 2048), observe(tracker, packet(true), 6, identity(2, 2048)));
    }

    @Test void anotherRemoteSourceAndUnsupportedLayoutsKeepNativeIdentity() {
        for (String firmware : new String[]{"1.11", ""}) {
            var tracker = new LoadedTrackOriginTracker();
            tracker.observe("CDJ-1500X", firmware, 2, 6, packet(false), LINK, true);
            assertEquals(LOCAL, tracker.observe("CDJ-1500X", firmware, 2, 6, packet(true), LOCAL, true));
        }
        var tracker = new LoadedTrackOriginTracker();
        observe(tracker, packet(false), 6, LINK);
        assertEquals(identity(3, 1031), observe(tracker, packet(true), 6, identity(3, 1031)));
        tracker = new LoadedTrackOriginTracker();
        observe(tracker, packet(false), 6, LINK);
        assertEquals(LOCAL, observe(tracker, new byte[512], 6, LOCAL));
        tracker = new LoadedTrackOriginTracker();
        observe(tracker, packet(false), 6, LINK);
        byte[] otherSubtype = packet(true);
        otherSubtype[0x20] = 7;
        assertEquals(LOCAL, observe(tracker, otherSubtype, 6, LOCAL));
        assertEquals(LOCAL, tracker.observe("CDJ-3000", "1.10", 2, 6, packet(true), LOCAL, true));
        tracker = new LoadedTrackOriginTracker();
        observe(tracker, packet(false), 6, LINK);
        byte[] wrongDeclaredLength = packet(true);
        wrongDeclaredLength[0x22] = 1;
        wrongDeclaredLength[0x23] = (byte)0xdc;
        assertEquals(LOCAL, observe(tracker, wrongDeclaredLength, 6, LOCAL));
    }

    @Test void transientIncoherentIdentityDoesNotReplaceConfirmedOrigin() {
        var tracker = new LoadedTrackOriginTracker();
        observe(tracker, packet(false), 6, LINK);
        tracker.observe("CDJ-1500X", "1.10", 2, 6, packet(true), identity(2, 2048), false);
        assertEquals(LINK, observe(tracker, packet(true), 6, LOCAL));
    }

    @Test void known512ByteSameLoadDoesNotEraseExtendedWitness() {
        var tracker = new LoadedTrackOriginTracker();
        observe(tracker, packet(false), 6, LINK);
        byte[] legacy = java.util.Arrays.copyOf(packet(true), 512);
        assertEquals(LINK, observe(tracker, legacy, 3, BeatLinkRuntime.ResolvedTrackIdentity.noTrack()));
        assertEquals(LINK, observe(tracker, packet(true), 3, LOCAL));
        legacy[0x197] ^= 1;
        assertEquals(identity(2, 1030), observe(tracker, legacy, 3, BeatLinkRuntime.ResolvedTrackIdentity.noTrack()));
        assertEquals(LOCAL, observe(tracker, packet(true), 6, LOCAL));
    }

    private static CdjStatus nativeStatus(boolean cached, int state, int source, boolean coherent)
            throws Exception {
        byte[] bytes = packet(cached);
        System.arraycopy("Qspt1WmJOL".getBytes(StandardCharsets.US_ASCII), 0, bytes, 0, 10);
        System.arraycopy("CDJ-1500X".getBytes(StandardCharsets.US_ASCII), 0, bytes, 0x0b, 9);
        System.arraycopy("1.10".getBytes(StandardCharsets.US_ASCII), 0, bytes, 0x7c, 4);
        bytes[0x0a] = 0x0a;
        bytes[0x21] = 2;
        bytes[0x22] = 4;
        bytes[0x23] = 0x5c;
        bytes[0x28] = (byte)source;
        bytes[0x29] = 3;
        bytes[0x2a] = 1;
        bytes[0x2e] = 4;
        bytes[0x2f] = 7;
        bytes[0x7b] = (byte)state;
        bytes[0x8d] = 0x10;
        bytes[0x92] = (byte)(coherent ? 0x3c : 0xff);
        bytes[0x93] = (byte)(coherent ? 0x8c : 0xff);
        bytes[0xa3] = 64;
        bytes[0xa6] = 4;
        return new CdjStatus(new DatagramPacket(bytes, bytes.length, InetAddress.getLoopbackAddress(), 50002));
    }

    private static CdjStatus receivedLikeBeatLink(CdjStatus sent) throws Exception {
        // VirtualCdj 8.0 receiveLoop allocates exactly 512 bytes. Exercise
        // actual UDP truncation rather than a directly constructed full frame.
        var loopback = InetAddress.getLoopbackAddress();
        try (var receiver = new DatagramSocket(0, loopback);
             var sender = new DatagramSocket(0, loopback)) {
            receiver.setSoTimeout(1000);
            byte[] bytes = sent.getPacketBytes();
            sender.send(new DatagramPacket(bytes, bytes.length, loopback, receiver.getLocalPort()));
            var packet = new DatagramPacket(new byte[512], 512);
            receiver.receive(packet);
            assertEquals(512, packet.getLength());
            return new CdjStatus(packet);
        }
    }

    @Test void beatLink512ByteReceiveBufferMustNotDisableCachedLinkContinuity() throws Exception {
        var output = new ByteArrayOutputStream();
        var publisher = new BridgePublisher(output, new ObjectMapper());
        var runtime = new BeatLinkRuntime(publisher);
        runtime.receivedDeviceUpdate(receivedLikeBeatLink(nativeStatus(false, 6, 1, true)));
        for (int i = 0; i < 8; i++) {
            runtime.receivedDeviceUpdate(receivedLikeBeatLink(nativeStatus(true, 6, 2, true)));
        }
        publisher.close();
        int statuses = 0;
        for (String line : output.toString(StandardCharsets.UTF_8).strip().split("\\R")) {
            var message = new ObjectMapper().readTree(line);
            if (message.get("payload").has("rekordboxId")) {
                assertEquals(1, message.get("payload").get("sourcePlayer").asInt());
                statuses++;
            }
        }
        org.junit.jupiter.api.Assertions.assertTrue(statuses > 0);
    }

    @Test void truncatedFreshLoadStillDiscardsConfirmedRemoteOrigin() throws Exception {
        var tracker = new LoadedTrackOriginTracker();
        byte[] before = receivedLikeBeatLink(nativeStatus(false, 6, 1, true)).getPacketBytes();
        byte[] after = receivedLikeBeatLink(nativeStatus(true, 6, 2, true)).getPacketBytes();
        assertEquals(LINK, observe(tracker, before, 6, LINK));
        assertEquals(LINK, observe(tracker, after, 6, LOCAL));
        tracker.observe("CDJ-1500X", "1.10", 2, 2, after, LOCAL, false);
        assertEquals(LOCAL, observe(tracker, after, 6, LOCAL));
        observe(tracker, before, 6, LINK);
        tracker.forget(2);
        assertEquals(LOCAL, observe(tracker, after, 6, LOCAL));
    }

    @Test void nativeDecoderAndPublishedFactsKeepOriginThenHonorRealReload() throws Exception {
        var output = new ByteArrayOutputStream();
        var mapper = new ObjectMapper();
        var publisher = new BridgePublisher(output, mapper);
        var runtime = new BeatLinkRuntime(publisher);
        // Exercises the real CdjStatus decoder, coherence filter and outbound
        // transport facts, without starting any socket or device session.
        runtime.receivedDeviceUpdate(nativeStatus(false, 6, 1, true));
        runtime.receivedDeviceUpdate(nativeStatus(true, 6, 2, true));
        publisher.close();
        int statuses = 0;
        for (String line : output.toString(StandardCharsets.UTF_8).strip().split("\\R")) {
            var message = mapper.readTree(line);
            if (message.get("type").asText().endsWith("Status")
                    && message.get("payload").has("rekordboxId")) {
                assertEquals(1, message.get("payload").get("sourcePlayer").asInt());
                assertEquals(1031, message.get("payload").get("rekordboxId").asInt());
                statuses++;
            }
        }
        org.junit.jupiter.api.Assertions.assertTrue(statuses > 0);

        output = new ByteArrayOutputStream();
        publisher = new BridgePublisher(output, mapper);
        runtime = new BeatLinkRuntime(publisher);
        runtime.receivedDeviceUpdate(nativeStatus(false, 6, 1, true));
        // Incoherent loading must reset the retained origin before filtering.
        runtime.receivedDeviceUpdate(nativeStatus(true, 2, 2, false));
        runtime.receivedDeviceUpdate(nativeStatus(true, 6, 2, true));
        publisher.close();
        int lastSource = 0;
        for (String line : output.toString(StandardCharsets.UTF_8).strip().split("\\R")) {
            var message = mapper.readTree(line);
            if (message.get("payload").has("rekordboxId")) {
                lastSource = message.get("payload").get("sourcePlayer").asInt();
            }
        }
        assertEquals(2, lastSource);
    }
}
