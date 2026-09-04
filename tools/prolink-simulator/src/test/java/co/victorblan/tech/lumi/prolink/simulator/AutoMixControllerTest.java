package co.victorblan.tech.lumi.prolink.simulator;

import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.concurrent.atomic.AtomicLong;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class AutoMixControllerTest {
    @Test
    void alternatesTwoPlayersWithOneExclusiveMaster() {
        AtomicLong firstClock = new AtomicLong();
        AtomicLong secondClock = new AtomicLong();
        PlayerState first = PlayerStateTest.loadedPlayer(1, firstClock);
        PlayerState second = PlayerStateTest.loadedPlayer(2, secondClock);
        second.setLoop(1_000, 2_000);

        try (AutoMixController controller = new AutoMixController(List.of(first, second))) {
            controller.setEnabled(true, 30);
            assertTrue(first.snapshot().master());
            assertFalse(second.snapshot().master());

            controller.transitionNowForTesting();

            assertFalse(first.snapshot().master());
            assertFalse(first.snapshot().onAir());
            assertFalse(first.snapshot().playing());
            assertTrue(second.snapshot().master());
            assertTrue(second.snapshot().onAir());
            assertTrue(second.snapshot().playing());
            assertEquals(1_000, second.snapshot().positionMillis());
            assertEquals(1, controller.status().transitionCount());
        }
    }

    @Test
    void refusesToStartWithoutTwoLoadedTracks() {
        PlayerState loaded = PlayerStateTest.loadedPlayer(1, new AtomicLong());
        PlayerState empty = new PlayerState(2);
        try (AutoMixController controller = new AutoMixController(List.of(loaded, empty))) {
            assertThrows(IllegalStateException.class, () -> controller.setEnabled(true, 30));
        }
    }
}
