package co.victorblan.tech.lumi.prolink.simulator;

import org.junit.jupiter.api.Test;

import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.atomic.AtomicLong;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class PlayerStateTest {
    @Test
    void loopWrapsNaturallyAndPreservesOvershoot() {
        AtomicLong clock = new AtomicLong();
        PlayerState player = loadedPlayer(1, clock);
        player.setLoop(1_000, 2_000);
        player.seek(1_500);
        player.play();

        clock.addAndGet(750_000_000L);
        PlayerState.Snapshot snapshot = player.snapshot();

        assertTrue(snapshot.loopEnabled());
        assertEquals(1_250, snapshot.positionMillis());
        assertEquals(1, snapshot.loopWrapCount());
        assertTrue(snapshot.playing());
    }

    @Test
    void disablingLoopAllowsPlaybackToPassItsFormerEnd() {
        AtomicLong clock = new AtomicLong();
        PlayerState player = loadedPlayer(1, clock);
        player.setLoop(1_000, 2_000);
        player.seek(1_500);
        player.disableLoop();
        player.play();

        clock.addAndGet(750_000_000L);
        PlayerState.Snapshot snapshot = player.snapshot();

        assertFalse(snapshot.loopEnabled());
        assertEquals(2_250, snapshot.positionMillis());
    }

    static PlayerState loadedPlayer(int number, AtomicLong clock) {
        PlayerState player = new PlayerState(number, clock::get);
        player.load(new UsbLibrary.Track(
                10_000 + number,
                "Track " + number,
                "Lumi",
                12_000,
                10_000,
                Path.of("/tmp/track-" + number + ".DAT"),
                true,
                List.of(
                        new UsbLibrary.BeatPoint(1, 1, 12_000, 0),
                        new UsbLibrary.BeatPoint(2, 2, 12_000, 500),
                        new UsbLibrary.BeatPoint(3, 3, 12_000, 1_000),
                        new UsbLibrary.BeatPoint(4, 4, 12_000, 1_500),
                        new UsbLibrary.BeatPoint(5, 1, 12_000, 2_000)
                )
        ));
        return player;
    }
}
