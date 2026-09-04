package co.victorblan.tech.lumi.prolink.simulator;

import org.junit.jupiter.api.Test;

import java.nio.file.Path;
import java.util.List;
import java.util.Random;
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
        UsbLibrary library = library(first.snapshot().track(), second.snapshot().track());

        try (AutoMixController controller = new AutoMixController(List.of(first, second), library)) {
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
        UsbLibrary library = library(loaded.snapshot().track());
        try (AutoMixController controller = new AutoMixController(List.of(loaded, empty), library)) {
            assertThrows(IllegalStateException.class, () -> controller.setEnabled(true, 30));
        }
    }

    @Test
    void playlistModePreloadsDifferentTracksAtEveryHandoff() {
        UsbLibrary.Track firstTrack = PlayerStateTest.loadedPlayer(1, new AtomicLong()).snapshot().track();
        UsbLibrary.Track secondTrack = PlayerStateTest.loadedPlayer(2, new AtomicLong()).snapshot().track();
        UsbLibrary.Track thirdTrack = PlayerStateTest.loadedPlayer(3, new AtomicLong()).snapshot().track();
        UsbLibrary.Playlist playlist = new UsbLibrary.Playlist(
                77L, "Sets / Soak", List.of(firstTrack, secondTrack, thirdTrack)
        );
        UsbLibrary library = UsbLibrary.forTesting(
                Path.of("/tmp"), List.of(firstTrack, secondTrack, thirdTrack), List.of(playlist)
        );
        PlayerState first = new PlayerState(1);
        PlayerState second = new PlayerState(2);

        try (AutoMixController controller = new AutoMixController(
                List.of(first, second), library, new Random(42)
        )) {
            controller.setEnabled(true, 30, 77L, false);
            assertEquals(firstTrack.id(), first.snapshot().track().id());
            assertEquals(secondTrack.id(), second.snapshot().track().id());
            assertEquals("playlist", controller.status().mode());
            assertEquals(3, controller.status().playlistTrackCount());

            controller.transitionNowForTesting();
            assertTrue(second.snapshot().master());
            assertEquals(thirdTrack.id(), first.snapshot().track().id());

            controller.transitionNowForTesting();
            assertTrue(first.snapshot().master());
            assertEquals(thirdTrack.id(), first.snapshot().track().id());
            assertEquals(firstTrack.id(), second.snapshot().track().id());
        }
    }

    @Test
    void playlistModeRejectsAPlaylistWithoutVariation() {
        UsbLibrary.Track onlyTrack = PlayerStateTest.loadedPlayer(1, new AtomicLong()).snapshot().track();
        UsbLibrary.Playlist playlist = new UsbLibrary.Playlist(
                88L, "Sets / One track", List.of(onlyTrack, onlyTrack)
        );
        UsbLibrary library = UsbLibrary.forTesting(
                Path.of("/tmp"), List.of(onlyTrack), List.of(playlist)
        );

        try (AutoMixController controller = new AutoMixController(
                List.of(new PlayerState(1), new PlayerState(2)), library
        )) {
            assertThrows(
                    IllegalStateException.class,
                    () -> controller.setEnabled(true, 30, 88L, false)
            );
        }
    }

    private static UsbLibrary library(UsbLibrary.Track... tracks) {
        return UsbLibrary.forTesting(Path.of("/tmp"), List.of(tracks));
    }
}
