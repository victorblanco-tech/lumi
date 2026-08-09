package co.victorblan.tech.lumi.prolink.simulator;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SimulatorConfigTest {
    @Test
    void parsesExplicitRemoteAndPlayerConfiguration() {
        SimulatorConfig config = SimulatorConfig.parse(new String[]{
                "--usb", "/Volumes/DJ USB",
                "--interface", "en0",
                "--player", "2",
                "--bind", "127.0.0.1",
                "--port", "18000",
                "--token", "test-token-123456789"
        });

        assertEquals(2, config.playerNumber());
        assertEquals("en0", config.networkInterface());
        assertEquals("127.0.0.1", config.bindAddress());
        assertEquals(18_000, config.controlPort());
        assertEquals("test-token-123456789", config.controlToken());
    }

    @Test
    void generatesAStrongTokenWhenNoneWasProvided() {
        SimulatorConfig config = SimulatorConfig.parse(new String[]{"--usb", "/Volumes/USB"});
        assertTrue(config.controlToken().length() >= 32);
    }

    @Test
    void rejectsNonPlayerNumbers() {
        assertThrows(IllegalArgumentException.class, () -> SimulatorConfig.parse(new String[]{
                "--usb", "/Volumes/USB", "--player", "7"
        }));
    }
}
