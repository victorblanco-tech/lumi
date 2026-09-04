package co.victorblan.tech.lumi.prolink.simulator;

import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.net.Inet4Address;
import java.net.InetAddress;
import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.atomic.AtomicLong;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SimulatorControlsTest {
    private static final ObjectMapper JSON = new ObjectMapper();

    @Test
    void controlsBothPlayersLoopsAndAutoMixWithoutNetworkTiming(@TempDir Path root) throws Exception {
        PlayerState trackOne = PlayerStateTest.loadedPlayer(1, new AtomicLong());
        PlayerState trackTwo = PlayerStateTest.loadedPlayer(2, new AtomicLong());
        List<UsbLibrary.Track> sourceTracks = List.of(
                trackOne.snapshot().track(), trackTwo.snapshot().track()
        );
        UsbLibrary library = UsbLibrary.forTesting(
                root,
                sourceTracks,
                List.of(new UsbLibrary.Playlist(77L, "Sets / Soak", sourceTracks))
        );
        PlayerState first = new PlayerState(1);
        PlayerState second = new PlayerState(2);
        List<PlayerState> players = List.of(first, second);
        TestTransport transport = new TestTransport();

        try (AutoMixController autoMix = new AutoMixController(players, library)) {
            SimulatorControls controls = new SimulatorControls(library, players, autoMix, transport);
            controls.apply("load", JSON.readTree("{\"playerNumber\":1,\"trackId\":10001}"));
            controls.apply("load", JSON.readTree("{\"playerNumber\":2,\"trackId\":10002}"));
            controls.apply("loop", JSON.readTree(
                    "{\"playerNumber\":1,\"startMillis\":1000,\"endMillis\":2000}"
            ));
            assertTrue(first.snapshot().loopEnabled());

            controls.apply("auto-mix", JSON.readTree(
                    "{\"enabled\":true,\"intervalSeconds\":5,\"playlistId\":77,\"shuffle\":false}"
            ));
            assertTrue(autoMix.status().enabled());
            assertEquals("playlist", autoMix.status().mode());
            assertEquals(77L, autoMix.status().playlistId());
            assertTrue(first.snapshot().master());
            assertFalse(second.snapshot().master());

            controls.apply("master", JSON.readTree("{\"playerNumber\":2,\"enabled\":true}"));
            assertFalse(first.snapshot().master());
            assertTrue(second.snapshot().master());

            controls.apply("precise-burst", JSON.readTree("{\"playerNumber\":2}"));
            assertEquals(2, transport.lastBurstPlayer);
            assertThrows(
                    SimulatorControls.UnknownActionException.class,
                    () -> controls.apply("mystery", JSON.createObjectNode())
            );
        }
    }

    private static final class TestTransport implements SimulatorTransport {
        private final ProLinkBroadcaster.Endpoint endpoint;
        private int lastBurstPlayer;

        private TestTransport() throws Exception {
            endpoint = new ProLinkBroadcaster.Endpoint(
                    "test0",
                    (Inet4Address) InetAddress.getByName("127.0.0.1"),
                    (Inet4Address) InetAddress.getByName("127.255.255.255"),
                    new byte[]{0, 1, 2, 3, 4, 5}
            );
        }

        @Override
        public void triggerPreciseBurst(int playerNumber) {
            lastBurstPlayer = playerNumber;
        }

        @Override
        public ProLinkBroadcaster.Endpoint endpoint() {
            return endpoint;
        }

        @Override
        public int peerCount() {
            return 0;
        }

        @Override
        public ProLinkBroadcaster.TrafficDiagnostics trafficDiagnostics() {
            return new ProLinkBroadcaster.TrafficDiagnostics(
                    "test", 100, 20, 2, 4, 6, 8, 0, null
            );
        }
    }
}
