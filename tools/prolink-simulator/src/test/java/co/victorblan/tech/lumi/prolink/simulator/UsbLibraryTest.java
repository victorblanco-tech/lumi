package co.victorblan.tech.lumi.prolink.simulator;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ObjectNode;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.nio.file.Path;
import static org.junit.jupiter.api.Assertions.*;

class UsbLibraryTest {
    @TempDir Path root;

    private ObjectNode projection(Path analysis, long id) {
        ObjectNode data = new ObjectMapper().createObjectNode();
        data.putArray("tracks").addObject().put("id", id).put("title", "Test")
                .put("artist", "Fixture").put("tempoCentiBpm", 12000)
                .put("durationMillis", 10000).put("analysisPath", analysis.toString());
        ObjectNode playlist = data.putArray("playlists").addObject().put("id", 7)
                .put("name", "Psy/Tech");
        playlist.putArray("folders").add("Genre 5 Stars").add("140+");
        playlist.putArray("trackIds").add(id);
        return data;
    }

    private Path analysis(long... times) throws IOException {
        ByteBuffer bytes = ByteBuffer.allocate(36 + times.length * 8);
        bytes.putInt(0x504d4149).putInt(12).putInt(bytes.capacity());
        bytes.putInt(0x5051545a).putInt(24).putInt(24 + times.length * 8);
        bytes.putInt(0).putInt(0).putInt(times.length);
        for (int i = 0; i < times.length; i++) {
            bytes.putShort((short) (i % 4 + 1)).putShort((short) 12000).putInt((int) times[i]);
        }
        return Files.write(root.resolve("ANLZ.DAT"), bytes.array());
    }

    @Test void preservesUnsignedIdsRealBeatTimesAndStructuralFolders() throws Exception {
        UsbLibrary library = UsbLibrary.fromOneLibrary(root, projection(analysis(125, 625, 1125), 0xfffffffeL));
        UsbLibrary.Track track = library.requireTrack(-2);
        assertTrue(track.exactBeatGrid());
        assertEquals(125, track.beatGrid().getFirst().timeMillis());
        assertEquals(-1, track.beatIndexAt(124));
        assertEquals(1, track.beatIndexAt(625));
        assertEquals("Genre 5 Stars / 140+ / Psy/Tech", library.requirePlaylist(7).path());
        assertSame(track, library.requirePlaylist(7).tracks().getFirst());
    }

    @Test void neverInventsMissingBeatGrid() throws Exception {
        UsbLibrary library = UsbLibrary.fromOneLibrary(root, projection(analysis(), 1));
        assertFalse(library.requireTrack(1).exactBeatGrid());
        assertTrue(library.requireTrack(1).beatGrid().isEmpty());
    }

    @Test void rejectsNonMonotonicGrid() throws Exception {
        ObjectNode data = projection(analysis(500, 400), 1);
        assertThrows(IOException.class, () -> UsbLibrary.fromOneLibrary(root, data));
    }

    @Test void rejectsEscapingAnalysisSymlink(@TempDir Path outside) throws Exception {
        Path link = Files.createSymbolicLink(root.resolve("outside.DAT"), Files.createFile(outside.resolve("outside.DAT")));
        assertThrows(IOException.class, () -> UsbLibrary.fromOneLibrary(root, projection(link, 1)));
    }

    @Test void rejectsOutOfRangeIdsRatherThanTruncating() throws Exception {
        ObjectNode data = projection(analysis(0, 500), 0x100000001L);
        assertThrows(IOException.class, () -> UsbLibrary.fromOneLibrary(root, data));
    }
}
