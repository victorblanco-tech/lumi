package co.victorblan.tech.lumi.prolink.simulator;

import org.deepsymmetry.beatlink.CdjStatus;
import org.deepsymmetry.beatlink.Util;
import org.deepsymmetry.beatlink.VirtualCdj;

import java.net.DatagramPacket;
import java.net.Inet4Address;
import java.lang.reflect.Field;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.HexFormat;

/**
 * Produces the narrow set of documented Pro DJ Link packets required by the
 * development simulator. The baseline templates correspond to beat-link 8.0.0
 * and are patched only at fields documented by Deep Symmetry's protocol
 * analysis. This class does not implement player remote control or media serving.
 */
final class ProLinkPackets {
    private static final byte[] MAGIC = HexFormat.of().parseHex("5173707431576d4a4f4c");
    private static final byte[] ANNOUNCEMENT_TEMPLATE = beatLinkTemplate("keepAliveBytes");
    private static final byte[] STATUS_TEMPLATE = beatLinkTemplate("STATUS_PAYLOAD");
    private static final byte[] BEAT_TEMPLATE = beatLinkTemplate("BEAT_PAYLOAD");

    private static final int STATUS_SOURCE_PLAYER = 9;
    private static final int STATUS_SOURCE_SLOT = 10;
    private static final int STATUS_TRACK_TYPE = 11;
    private static final int STATUS_REKORDBOX_ID = 13;
    private static final int STATUS_PLAY_STATE_1 = 92;
    private static final int STATUS_FLAGS = 106;
    private static final int STATUS_PLAY_STATE_2 = 108;
    private static final int STATUS_PITCH_1 = 110;
    private static final int STATUS_BPM = 115;
    private static final int STATUS_PITCH_2 = 122;
    private static final int STATUS_PLAY_STATE_3 = 126;
    private static final int STATUS_MASTER_MEANINGFUL = 127;
    private static final int STATUS_MASTER_HANDOFF = 128;
    private static final int STATUS_BEAT_NUMBER = 129;
    private static final int STATUS_BEAT_WITHIN_BAR = 135;
    private static final int STATUS_PITCH_3 = 162;
    private static final int STATUS_PITCH_4 = 166;
    private static final int STATUS_PACKET_COUNTER = 169;

    private ProLinkPackets() {
    }

    static DatagramPacket announcement(
            String deviceName,
            int playerNumber,
            byte[] hardwareAddress,
            Inet4Address localAddress,
            int peerCount
    ) {
        byte[] packet = ANNOUNCEMENT_TEMPLATE.clone();
        putName(packet, 12, deviceName);
        packet[36] = (byte) playerNumber;
        Arrays.fill(packet, 38, 44, (byte) 0);
        System.arraycopy(hardwareAddress, 0, packet, 38, Math.min(6, hardwareAddress.length));
        byte[] address = localAddress.getAddress();
        System.arraycopy(address, 0, packet, 44, address.length);
        packet[48] = (byte) Math.max(1, Math.min(peerCount, 255));
        return new DatagramPacket(packet, packet.length);
    }

    static DatagramPacket status(String deviceName, PlayerState.Snapshot state, int packetCounter) {
        byte[] payload = STATUS_TEMPLATE.clone();
        int player = state.playerNumber();
        payload[2] = (byte) player;
        payload[5] = (byte) player;
        payload[8] = state.playing() ? (byte) 1 : 0;

        boolean loaded = state.track() != null;
        payload[STATUS_SOURCE_PLAYER] = loaded ? (byte) player : 0;
        payload[STATUS_SOURCE_SLOT] = loaded ? CdjStatus.TrackSourceSlot.USB_SLOT.protocolValue : 0;
        payload[STATUS_TRACK_TYPE] = loaded ? CdjStatus.TrackType.REKORDBOX.protocolValue : 0;
        numberToBytes(loaded ? state.track().id() : 0, payload, STATUS_REKORDBOX_ID, 4);

        payload[STATUS_PLAY_STATE_1] = loaded
                ? state.playing() ? CdjStatus.PlayState1.PLAYING.protocolValue : CdjStatus.PlayState1.PAUSED.protocolValue
                : CdjStatus.PlayState1.NO_TRACK.protocolValue;
        payload[STATUS_FLAGS] = (byte) (0x84
                | (state.playing() ? 0x40 : 0)
                | (state.master() ? 0x20 : 0)
                | (state.onAir() ? 0x08 : 0));
        payload[STATUS_PLAY_STATE_2] = state.playing() ? (byte) 0x7a : (byte) 0x7e;

        int pitch = (int) Util.percentageToPitch(state.pitchPercent());
        numberToBytes(pitch, payload, STATUS_PITCH_1, 3);
        numberToBytes(pitch, payload, STATUS_PITCH_2, 3);
        numberToBytes(pitch, payload, STATUS_PITCH_3, 3);
        numberToBytes(state.playing() ? pitch : 0, payload, STATUS_PITCH_4, 3);

        int bpm = loaded ? state.originalTempoCentiBpm() : 0xffff;
        numberToBytes(bpm, payload, STATUS_BPM, 2);
        payload[STATUS_PLAY_STATE_3] = loaded
                ? state.playing() ? CdjStatus.PlayState3.FORWARD_CDJ.protocolValue : CdjStatus.PlayState3.PAUSED_OR_REVERSE.protocolValue
                : CdjStatus.PlayState3.NO_TRACK.protocolValue;
        payload[STATUS_MASTER_MEANINGFUL] = 1;
        payload[STATUS_MASTER_HANDOFF] = (byte) 0xff;
        numberToBytes(state.beatNumber(), payload, STATUS_BEAT_NUMBER, 4);
        payload[STATUS_BEAT_WITHIN_BAR] = (byte) state.beatWithinBar();
        numberToBytes(packetCounter, payload, STATUS_PACKET_COUNTER, 4);
        return build(Util.PacketType.CDJ_STATUS, deviceName, payload);
    }

    static DatagramPacket beat(String deviceName, PlayerState.Snapshot state) {
        if (state.track() == null || state.beat() == null) {
            throw new IllegalArgumentException("A loaded track with a beat grid is required");
        }
        byte[] payload = BEAT_TEMPLATE.clone();
        payload[2] = (byte) state.playerNumber();
        int beatInterval = nextBeatInterval(state);
        int beatsToBar = 5 - state.beatWithinBar();
        numberToBytes(beatInterval, payload, 5, 4);
        numberToBytes(beatInterval * 2, payload, 9, 4);
        numberToBytes(beatInterval * beatsToBar, payload, 13, 4);
        numberToBytes(beatInterval * 4, payload, 17, 4);
        numberToBytes(beatInterval * (beatsToBar + 4), payload, 21, 4);
        numberToBytes(beatInterval * 8, payload, 25, 4);
        numberToBytes((int) Util.percentageToPitch(state.pitchPercent()), payload, 53, 4);
        numberToBytes(state.originalTempoCentiBpm(), payload, 59, 2);
        payload[61] = (byte) state.beatWithinBar();
        payload[64] = (byte) state.playerNumber();
        return build(Util.PacketType.BEAT, deviceName, payload);
    }

    static DatagramPacket precisePosition(
            String deviceName,
            PlayerState.Snapshot state,
            long playbackPositionMillis
    ) {
        if (state.track() == null) {
            throw new IllegalArgumentException("A loaded track is required");
        }
        // PrecisePosition is exactly 60 bytes: the common 31-byte header and
        // this 29-byte payload. Offsets are pinned to beat-link 8.0.0 and are
        // parsed back through beat-link in the packet contract tests.
        byte[] payload = new byte[29];
        payload[2] = (byte) state.playerNumber();
        numberToBytes(state.track().durationMillis(), payload, 5, 4);
        numberToBytes(
                Math.max(0L, Math.min(playbackPositionMillis, state.track().durationMillis())),
                payload,
                9,
                4
        );
        numberToBytes(Math.round(state.pitchPercent() * 100.0), payload, 13, 4);
        numberToBytes(Math.round(state.effectiveBpm() * 10.0), payload, 25, 4);
        return build(Util.PacketType.PRECISE_POSITION, deviceName, payload);
    }

    private static int nextBeatInterval(PlayerState.Snapshot state) {
        int nextIndex = state.beatIndex() + 1;
        if (nextIndex < state.track().beatGrid().size()) {
            long interval = state.track().beatGrid().get(nextIndex).timeMillis() - state.beat().timeMillis();
            if (interval > 0 && interval <= Integer.MAX_VALUE) {
                return (int) interval;
            }
        }
        if (state.originalTempoCentiBpm() <= 0) {
            return 500;
        }
        return Math.max(1, (int) Math.round(6_000_000.0 / state.originalTempoCentiBpm()));
    }

    private static DatagramPacket build(Util.PacketType type, String deviceName, byte[] payload) {
        return Util.buildPacket(type, nameBuffer(deviceName), ByteBuffer.wrap(payload));
    }

    private static ByteBuffer nameBuffer(String deviceName) {
        byte[] name = new byte[20];
        putName(name, 0, deviceName);
        return ByteBuffer.wrap(name);
    }

    private static void putName(byte[] target, int offset, String deviceName) {
        byte[] encoded = deviceName.getBytes(StandardCharsets.US_ASCII);
        int length = Math.min(20, encoded.length);
        Arrays.fill(target, offset, offset + 20, (byte) 0);
        System.arraycopy(encoded, 0, target, offset, length);
    }

    private static void numberToBytes(long value, byte[] target, int offset, int length) {
        if (value < Integer.MIN_VALUE || value > 0xffff_ffffL) {
            throw new IllegalArgumentException("Packet number does not fit in 32 bits: " + value);
        }
        Util.numberToBytes((int) value, target, offset, length);
    }

    private static byte[] beatLinkTemplate(String fieldName) {
        try {
            Field field = VirtualCdj.class.getDeclaredField(fieldName);
            field.setAccessible(true);
            return ((byte[]) field.get(null)).clone();
        } catch (ReflectiveOperationException failure) {
            throw new ExceptionInInitializerError(
                    "Pinned beat-link 8.0.0 no longer exposes packet template " + fieldName
            );
        }
    }

    static boolean hasMagicHeader(DatagramPacket packet) {
        return packet.getLength() >= MAGIC.length
                && Arrays.equals(MAGIC, Arrays.copyOf(packet.getData(), MAGIC.length));
    }
}
