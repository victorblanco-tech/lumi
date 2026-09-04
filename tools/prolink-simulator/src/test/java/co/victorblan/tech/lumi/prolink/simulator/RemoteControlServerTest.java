package co.victorblan.tech.lumi.prolink.simulator;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.net.Inet4Address;
import java.net.InetAddress;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.atomic.AtomicLong;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class RemoteControlServerTest {
    private static final ObjectMapper JSON = new ObjectMapper();
    private static final String TOKEN = "simulator-http-test-token";

    @Test
    void authenticatedApiAndBrowserExposeRecoveryControls(@TempDir Path root) throws Exception {
        UsbLibrary.Track firstTrack = PlayerStateTest.loadedPlayer(1, new AtomicLong()).snapshot().track();
        UsbLibrary.Track secondTrack = PlayerStateTest.loadedPlayer(2, new AtomicLong()).snapshot().track();
        UsbLibrary library = UsbLibrary.forTesting(root, List.of(firstTrack, secondTrack));
        PlayerState first = new PlayerState(1);
        PlayerState second = new PlayerState(2);
        List<PlayerState> players = List.of(first, second);
        TestTransport transport = new TestTransport();

        try (AutoMixController autoMix = new AutoMixController(players, library);
             TrafficFaultController faults = new TrafficFaultController(players, System::nanoTime, false);
             RemoteControlServer server = new RemoteControlServer(
                     library, players, autoMix, transport, faults, "127.0.0.1", 0, TOKEN
             );
             HttpClient client = HttpClient.newHttpClient()) {
            server.start();
            URI base = URI.create("http://127.0.0.1:" + server.port());

            HttpResponse<String> web = client.send(
                    HttpRequest.newBuilder(base.resolve("/")).GET().build(),
                    HttpResponse.BodyHandlers.ofString()
            );
            assertEquals(200, web.statusCode());
            assertTrue(web.body().contains("Recovery &amp; fault scenarios"));
            assertTrue(web.body().contains("Start Recovery Soak"));

            HttpResponse<String> unauthorized = client.send(
                    HttpRequest.newBuilder(base.resolve("/api/v1/status")).GET().build(),
                    HttpResponse.BodyHandlers.ofString()
            );
            assertEquals(401, unauthorized.statusCode());

            post(client, base, "load", "{\"playerNumber\":1,\"trackId\":10001}");
            post(client, base, "load", "{\"playerNumber\":2,\"trackId\":10002}");
            post(client, base, "master", "{\"playerNumber\":1,\"enabled\":true}");
            post(client, base, "play", "{\"playerNumber\":1}");
            post(client, base, "beat-jump", "{\"playerNumber\":1,\"beats\":4}");
            JsonNode fault = post(
                    client,
                    base,
                    "fault-disconnect",
                    "{\"playerNumber\":1,\"durationMillis\":1000}"
            );

            long jumpedPosition = fault.at("/players/0/positionMillis").asLong();
            assertTrue(jumpedPosition >= 2_000 && jumpedPosition < 2_250);
            assertEquals("temporary-disconnect", fault.at("/faults/activeFaults/0/kind").asText());
            assertEquals(1, fault.at("/faults/activeFaults/0/playerNumber").asInt());

            JsonNode restored = post(client, base, "clear-faults", "{}");
            assertTrue(restored.at("/faults/activeFaults").isEmpty());
            assertTrue(restored.at("/faults/manuallyOfflinePlayers").isEmpty());
            assertFalse(restored.at("/faults/recoverySoak/enabled").asBoolean());

            JsonNode removed = post(
                    client, base, "player-online", "{\"playerNumber\":2,\"enabled\":false}"
            );
            assertEquals(2, removed.at("/faults/manuallyOfflinePlayers/0").asInt());
            JsonNode rejoined = post(
                    client, base, "player-online", "{\"playerNumber\":2,\"enabled\":true}"
            );
            assertTrue(rejoined.at("/faults/manuallyOfflinePlayers").isEmpty());

            JsonNode handover = post(client, base, "master-handover", "{}");
            assertFalse(handover.at("/players/0/master").asBoolean());
            assertTrue(handover.at("/players/1/master").asBoolean());

            JsonNode soak = post(
                    client, base, "recovery-soak", "{\"enabled\":true,\"intervalSeconds\":8}"
            );
            assertTrue(soak.at("/faults/recoverySoak/enabled").asBoolean());
            assertEquals(8, soak.at("/faults/recoverySoak/intervalSeconds").asInt());
            JsonNode soakStopped = post(client, base, "clear-faults", "{}");
            assertFalse(soakStopped.at("/faults/recoverySoak/enabled").asBoolean());
        }
    }

    private static JsonNode post(
            HttpClient client,
            URI base,
            String action,
            String body
    ) throws Exception {
        HttpRequest request = HttpRequest.newBuilder(base.resolve("/api/v1/control/" + action))
                .header("Authorization", "Bearer " + TOKEN)
                .header("Content-Type", "application/json")
                .POST(HttpRequest.BodyPublishers.ofString(body))
                .build();
        HttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());
        assertEquals(200, response.statusCode(), response.body());
        return JSON.readTree(response.body());
    }

    private static final class TestTransport implements SimulatorTransport {
        private final ProLinkBroadcaster.Endpoint endpoint;

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
                    "test", 100, 20, 0, 0, 0, 0, 0, null
            );
        }
    }
}
