package co.victorblan.tech.lumi.prolink.simulator;

import com.fasterxml.jackson.databind.JsonNode;
import org.deepsymmetry.cratedigger.pdb.RekordboxAnlz;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

final class UsbLibrary {
    private static final Path DATABASE_PATH = Path.of("PIONEER", "rekordbox", "exportLibrary.db");
    private final Path root;
    private final String displayName;
    private final Map<Integer, Track> tracks;
    private final List<Track> sortedTracks;
    private final Map<Long, Playlist> playlists;
    private final List<PlaylistSummary> sortedPlaylists;
    private List<String> scanWarnings = List.of();

    List<String> scanWarnings() { return scanWarnings; }

    private UsbLibrary(Path root, Map<Integer, Track> tracks, List<Playlist> playlists) {
        this.root = root;
        this.displayName = root.getFileName() == null ? "Rekordbox USB" : root.getFileName().toString();
        this.tracks = Map.copyOf(tracks);
        this.sortedTracks = tracks.values().stream()
                .sorted(Comparator.comparing(Track::artist, String.CASE_INSENSITIVE_ORDER)
                        .thenComparing(Track::title, String.CASE_INSENSITIVE_ORDER))
                .toList();
        LinkedHashMap<Long, Playlist> indexedPlaylists = new LinkedHashMap<>();
        for (Playlist playlist : playlists) {
            if (indexedPlaylists.put(playlist.id(), playlist) != null) {
                throw new IllegalArgumentException("Duplicate Rekordbox playlist ID " + playlist.id());
            }
        }
        this.playlists = Map.copyOf(indexedPlaylists);
        this.sortedPlaylists = playlists.stream()
                .map(PlaylistSummary::from)
                .sorted(Comparator.comparing(PlaylistSummary::path, String.CASE_INSENSITIVE_ORDER))
                .toList();
    }

    static UsbLibrary open(Path requestedRoot) throws IOException {
        Path root = requestedRoot.toRealPath();
        if (!Files.isDirectory(root)) {
            throw new IOException("USB root is not a directory: " + root);
        }
        Path databasePath = checkedChild(root, DATABASE_PATH);
        if (!Files.isRegularFile(databasePath)) {
            throw new IOException("Rekordbox OneLibrary database not found: " + databasePath);
        }
        return fromOneLibrary(root, OneLibraryReader.read(root));
    }

    static UsbLibrary fromOneLibrary(Path root, JsonNode data) throws IOException {
        root = root.toRealPath();
        HashMap<Integer, Track> tracks = new HashMap<>();
        java.util.HashSet<Integer> seenIds = new java.util.HashSet<>();
        java.util.HashSet<Integer> skippedIds = new java.util.HashSet<>();
        ArrayList<String> warnings = new ArrayList<>();
        for (JsonNode row : data.path("tracks")) {
                int id = trackId(row.path("id"));
                if (!seenIds.add(id)) throw new IOException("Duplicate Rekordbox track ID " + id);
                Path analysisPath = Path.of(row.path("analysisPath").asText()).toRealPath();
                if (!analysisPath.startsWith(root)) throw new IOException("Analysis path escapes USB root");
                List<BeatPoint> beatGrid;
                try {
                    beatGrid = readBeatGrid(analysisPath);
                } catch (InvalidBeatGrid failure) {
                    skippedIds.add(id);
                    String warning = row.path("title").asText() + " (ID "
                            + Integer.toUnsignedString(id) + "): " + failure.getMessage();
                    warnings.add(warning);
                    System.err.println("USB track skipped: " + warning);
                    continue;
                }
                boolean exactBeatGrid = !beatGrid.isEmpty();
                Track track = new Track(
                        id,
                        row.path("title").asText(),
                        row.path("artist").asText(),
                        row.path("tempoCentiBpm").asInt(),
                        row.path("durationMillis").asLong(),
                        analysisPath,
                        exactBeatGrid,
                        beatGrid
                );
                if (tracks.put(id, track) != null) {
                    throw new IOException("Duplicate Rekordbox track ID " + id);
                }
        }
        ArrayList<Playlist> playlists = new ArrayList<>();
        for (JsonNode row : data.path("playlists")) {
            ArrayList<String> path = new ArrayList<>();
            row.path("folders").forEach(folder -> path.add(folder.asText()));
            path.add(row.path("name").asText());
            ArrayList<Track> members = new ArrayList<>();
            for (JsonNode id : row.path("trackIds")) {
                if (skippedIds.contains(trackId(id))) continue;
                Track track = tracks.get(trackId(id));
                if (track == null) throw new IOException("Playlist references an unknown OneLibrary track");
                members.add(track);
            }
            playlists.add(new Playlist(row.path("id").asLong(), String.join(" / ", path), members));
        }
        UsbLibrary library = new UsbLibrary(root, tracks, playlists);
        library.scanWarnings = List.copyOf(warnings);
        return library;
    }

    private static int trackId(JsonNode value) throws IOException {
        if (!value.isIntegralNumber() || value.asLong() < 1 || value.asLong() > 0xffffffffL) {
            throw new IOException("Invalid OneLibrary track ID");
        }
        // Wire IDs are unsigned 32-bit; preserve all bits in Java's signed int.
        return (int) value.asLong();
    }

    static UsbLibrary forTesting(Path root, List<Track> sourceTracks) {
        return forTesting(root, sourceTracks, List.of());
    }

    static UsbLibrary forTesting(Path root, List<Track> sourceTracks, List<Playlist> playlists) {
        HashMap<Integer, Track> indexed = new HashMap<>();
        for (Track track : sourceTracks) {
            indexed.put(track.id(), track);
        }
        return new UsbLibrary(root, indexed, playlists);
    }

    Path root() {
        return root;
    }

    String displayName() { return displayName; }

    int size() {
        return tracks.size();
    }

    int playlistCount() {
        return playlists.size();
    }

    Track requireTrack(int trackId) {
        Track track = tracks.get(trackId);
        if (track == null) {
            throw new IllegalArgumentException("Unknown Rekordbox track ID: " + trackId);
        }
        return track;
    }

    Playlist requirePlaylist(long playlistId) {
        Playlist playlist = playlists.get(playlistId);
        if (playlist == null) {
            throw new IllegalArgumentException("Unknown Rekordbox playlist ID: " + playlistId);
        }
        return playlist;
    }

    List<PlaylistSummary> playlists() {
        return sortedPlaylists;
    }

    List<TrackSummary> search(String query, int requestedLimit) {
        String normalized = query == null ? "" : query.trim().toLowerCase(Locale.ROOT);
        int limit = Math.max(1, Math.min(requestedLimit, 500));
        return sortedTracks.stream()
                .filter(track -> normalized.isEmpty()
                        || track.title().toLowerCase(Locale.ROOT).contains(normalized)
                        || track.artist().toLowerCase(Locale.ROOT).contains(normalized)
                        || Integer.toString(track.id()).equals(normalized))
                .limit(limit)
                .map(TrackSummary::from)
                .toList();
    }


    private static Path checkedChild(Path root, Path relative) throws IOException {
        Path normalized = root.resolve(relative).normalize();
        if (!normalized.startsWith(root)) {
            throw new IOException("Rekordbox path escapes USB root: " + relative);
        }
        Path resolved = normalized.toRealPath();
        if (!resolved.startsWith(root)) {
            throw new IOException("Rekordbox symlink escapes USB root: " + relative);
        }
        return resolved;
    }

    private static List<BeatPoint> readBeatGrid(Path analysisPath) throws IOException {
        RekordboxAnlz analysis = RekordboxAnlz.fromFile(analysisPath.toString());
        for (RekordboxAnlz.TaggedSection section : analysis.sections()) {
            if (!(section.body() instanceof RekordboxAnlz.BeatGridTag beatGridTag)) {
                continue;
            }
            ArrayList<BeatPoint> points = new ArrayList<>(beatGridTag.beats().size());
            int index = 1;
            for (RekordboxAnlz.BeatGridBeat beat : beatGridTag.beats()) {
                if (beat.beatNumber() < 1 || beat.beatNumber() > 4 || beat.tempo() == 0
                        || (!points.isEmpty() && beat.time() <= points.getLast().timeMillis())) {
                    throw new InvalidBeatGrid("Beat " + index + ": bar beat=" + beat.beatNumber()
                            + ", tempo=" + beat.tempo() + ", time=" + beat.time()
                            + "ms, previous=" + (points.isEmpty() ? "none" : points.getLast().timeMillis() + "ms"));
                }
                points.add(new BeatPoint(index++, beat.beatNumber(), (int) beat.tempo(), beat.time()));
            }
            return List.copyOf(points);
        }
        return List.of();
    }

    private static final class InvalidBeatGrid extends IOException {
        InvalidBeatGrid(String message) { super(message); }
    }


    record Track(
            int id,
            String title,
            String artist,
            int originalTempoCentiBpm,
            long durationMillis,
            Path analysisPath,
            boolean exactBeatGrid,
            List<BeatPoint> beatGrid
    ) {
        Track {
            beatGrid = List.copyOf(beatGrid);
        }

        int beatIndexAt(long positionMillis) {
            if (beatGrid.isEmpty()) {
                return -1;
            }
            int low = 0;
            int high = beatGrid.size() - 1;
            int result = -1;
            while (low <= high) {
                int middle = (low + high) >>> 1;
                if (beatGrid.get(middle).timeMillis() <= positionMillis) {
                    result = middle;
                    low = middle + 1;
                } else {
                    high = middle - 1;
                }
            }
            return result;
        }
    }

    record BeatPoint(int absoluteBeat, int beatWithinBar, int tempoCentiBpm, long timeMillis) {
    }

    record Playlist(long id, String path, List<Track> tracks) {
        Playlist {
            path = path == null ? "" : path;
            tracks = List.copyOf(tracks);
        }
    }

    record PlaylistSummary(long playlistId, String path, int trackCount) {
        static PlaylistSummary from(Playlist playlist) {
            return new PlaylistSummary(playlist.id(), playlist.path(), playlist.tracks().size());
        }
    }

    record TrackSummary(
            int trackId,
            String title,
            String artist,
            double bpm,
            long durationMillis,
            boolean exactBeatGrid
    ) {
        static TrackSummary from(Track track) {
            return new TrackSummary(
                    track.id(), track.title(), track.artist(),
                    track.originalTempoCentiBpm() / 100.0,
                    track.durationMillis(), track.exactBeatGrid()
            );
        }
    }
}
