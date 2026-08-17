package co.victorblan.tech.lumi.prolink.simulator;

import org.deepsymmetry.beatlink.Beat;
import org.deepsymmetry.beatlink.CdjStatus;
import org.deepsymmetry.beatlink.DeviceAnnouncement;
import org.deepsymmetry.beatlink.PrecisePosition;
import org.deepsymmetry.beatlink.Util;
import org.junit.jupiter.api.Test;

import java.net.DatagramPacket;
import java.net.Inet4Address;
import java.net.InetAddress;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ProLinkPacketsTest {
    @Test
    void announcementLooksLikeAPlayerToBeatLink() throws Exception {
        Inet4Address local = (Inet4Address) InetAddress.getByName("192.168.10.20");
        DatagramPacket packet = ProLinkPackets.announcement(
                "LUMI-SIM", 1, new byte[]{0, 1, 2, 3, 4, 5}, local, 2
        );
        packet.setAddress(local);

        DeviceAnnouncement announcement = new DeviceAnnouncement(packet);

        assertTrue(ProLinkPackets.hasMagicHeader(packet));
        assertEquals("LUMI-SIM", announcement.getDeviceName());
        assertEquals(1, announcement.getDeviceNumber());
        assertEquals(2, announcement.getPeerCount());
        assertEquals(List.of((byte) 0, (byte) 1, (byte) 2, (byte) 3, (byte) 4, (byte) 5),
                bytes(announcement.getHardwareAddress()));
    }

    @Test
    void statusCarriesUsbIdentityAndTransport() throws Exception {
        PlayerState state = loadedState();
        state.setMaster(true);
        state.setOnAir(true);
        state.setPitchPercent(5.0);
        state.play();

        DatagramPacket packet = ProLinkPackets.status("LUMI-SIM", state.snapshot(), 42);
        packet.setAddress(InetAddress.getByName("192.168.10.20"));
        CdjStatus status = new CdjStatus(packet);

        assertEquals("LUMI-SIM", status.getDeviceName());
        assertEquals(1, status.getDeviceNumber());
        assertEquals(1, status.getTrackSourcePlayer());
        assertEquals(CdjStatus.TrackSourceSlot.USB_SLOT, status.getTrackSourceSlot());
        assertEquals(CdjStatus.TrackType.REKORDBOX, status.getTrackType());
        assertEquals(12_345, status.getRekordboxId());
        assertEquals(12_800, status.getBpm());
        assertTrue(status.isPlaying());
        assertTrue(status.isTempoMaster());
        assertTrue(status.isOnAir());
        assertEquals(134.4, status.getEffectiveTempo(), 0.02);
    }

    @Test
    void beatCarriesGridPositionAndEffectiveTempo() throws Exception {
        PlayerState state = loadedState();
        state.seek(500);
        state.setPitchPercent(-2.5);
        state.play();

        DatagramPacket packet = ProLinkPackets.beat("LUMI-SIM", state.snapshot());
        packet.setAddress(InetAddress.getByName("192.168.10.20"));
        Beat beat = new Beat(packet);

        assertEquals(1, beat.getDeviceNumber());
        assertEquals(12_800, beat.getBpm());
        assertEquals(2, beat.getBeatWithinBar());
        assertEquals(124.8, beat.getEffectiveTempo(), 0.02);
    }

    @Test
    void precisePositionMatchesTheModernPlayerPacketContract() throws Exception {
        PlayerState state = loadedState();
        state.setPitchPercent(3.25);

        DatagramPacket packet = ProLinkPackets.precisePosition(
                "LUMI-SIM", state.snapshot(), 42_125
        );
        packet.setAddress(InetAddress.getByName("192.168.10.20"));
        PrecisePosition position = new PrecisePosition(packet);

        assertEquals(60, packet.getLength());
        assertEquals("LUMI-SIM", position.getDeviceName());
        assertEquals(1, position.getDeviceNumber());
        assertEquals(120_000, position.getTrackLength());
        assertEquals(42_125, position.getPlaybackPosition());
        assertEquals(3.25, Util.pitchToPercentage(position.getPitch()), 0.01);
        assertEquals(132.16, position.getEffectiveTempo(), 0.05);
    }

    private static PlayerState loadedState() {
        UsbLibrary.Track track = new UsbLibrary.Track(
                12_345,
                "Test Track",
                "Lumi",
                12_800,
                120_000,
                Path.of("/tmp/ANLZ0000.DAT"),
                true,
                List.of(
                        new UsbLibrary.BeatPoint(1, 1, 12_800, 0),
                        new UsbLibrary.BeatPoint(2, 2, 12_800, 500),
                        new UsbLibrary.BeatPoint(3, 3, 12_800, 1_000),
                        new UsbLibrary.BeatPoint(4, 4, 12_800, 1_500),
                        new UsbLibrary.BeatPoint(5, 1, 12_800, 2_000)
                )
        );
        PlayerState state = new PlayerState(1);
        state.load(track);
        return state;
    }

    private static List<Byte> bytes(byte[] values) {
        java.util.ArrayList<Byte> result = new java.util.ArrayList<>(values.length);
        for (byte value : values) {
            result.add(value);
        }
        return result;
    }
}
