package co.victorblan.tech.lumi.prolink.simulator;

import org.deepsymmetry.cratedigger.Database;
import org.deepsymmetry.cratedigger.pdb.RekordboxAnlz;
import org.deepsymmetry.cratedigger.pdb.RekordboxPdb;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.Map;

final class UsbLibrary {
    private static final Path DATABASE_PATH = Path.of("PIONEER", "rekordbox", "export.pdb");
    private final Path root;
    private final Map<Integer, Track> tracks;
    private final List<Track> sortedTracks;

    private UsbLibrary(Path root, Map<Integer, Track> tracks) {
        this.root = root;
        this.tracks = Map.copyOf(tracks);
        this.sortedTracks = tracks.values().stream()
                .sorted(Comparator.comparing(Track::artist, String.CASE_INSENSITIVE_ORDER)
                        .thenComparing(Track::title, String.CASE_INSENSITIVE_ORDER))
                .toList();
    }

    static UsbLibrary open(Path requestedRoot) throws IOException {
        Path root = requestedRoot.toRealPath();
        if (!Files.isDirectory(root)) {
            throw new IOException("USB root is not a directory: " + root);
        }
        Path databasePath = checkedChild(root, DATABASE_PATH);
        if (!Files.isRegularFile(databasePath)) {
            throw new IOException("Rekordbox DeviceSQL database not found: " + databasePath);
        }

        java.util.HashMap<Integer, Track> tracks = new java.util.HashMap<>();
        try (Database database = new Database(databasePath.toFile())) {
            for (RekordboxPdb.TrackRow row : database.trackIndex.values()) {
                int id = Math.toIntExact(row.id());
                String title = Database.getText(row.title());
                String artist = artist(database, row.artistId());
                String analysisPathValue = Database.getText(row.analyzePath());
                Path analysisPath = checkedDeclaredChild(root, analysisPathValue);
                List<BeatPoint> beatGrid = readBeatGrid(analysisPath);
                boolean exactBeatGrid = !beatGrid.isEmpty();
                if (!exactBeatGrid) {
                    beatGrid = syntheticBeatGrid((int) row.tempo(), row.duration());
                }
                Track track = new Track(
                        id,
                        title,
                        artist,
                        (int) row.tempo(),
                        row.duration() * 1_000L,
                        analysisPath,
                        exactBeatGrid,
                        beatGrid
                );
                if (tracks.put(id, track) != null) {
                    throw new IOException("Duplicate Rekordbox track ID " + id);
                }
            }
        }
        return new UsbLibrary(root, tracks);
    }

    static UsbLibrary forTesting(Path root, List<Track> sourceTracks) {
        java.util.HashMap<Integer, Track> indexed = new java.util.HashMap<>();
        for (Track track : sourceTracks) {
            indexed.put(track.id(), track);
        }
        return new UsbLibrary(root, indexed);
    }

    Path root() {
        return root;
    }

    int size() {
        return tracks.size();
    }

    Track requireTrack(int trackId) {
        Track track = tracks.get(trackId);
        if (track == null) {
            throw new IllegalArgumentException("Unknown Rekordbox track ID: " + trackId);
        }
        return track;
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

    private static String artist(Database database, long artistId) {
        RekordboxPdb.ArtistRow artist = database.artistIndex.get(artistId);
        return artist == null ? "" : Database.getText(artist.name());
    }

    private static Path checkedDeclaredChild(Path root, String declaredPath) throws IOException {
        String relative = declaredPath == null ? "" : declaredPath.trim().replace('\\', '/');
        while (relative.startsWith("/")) {
            relative = relative.substring(1);
        }
        if (relative.isBlank()) {
            throw new IOException("Track has no Rekordbox analysis path");
        }
        return checkedChild(root, Path.of(relative));
    }

    private static Path checkedChild(Path root, Path relative) throws IOException {
        Path normalized = root.resolve(relative).normalize();
        if (!normalized.startsWith(root)) {
            throw new IOException("Rekordbox path escapes USB root: " + relative);
        }
        return normalized.toRealPath();
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
                points.add(new BeatPoint(index++, beat.beatNumber(), (int) beat.tempo(), beat.time()));
            }
            return List.copyOf(points);
        }
        return List.of();
    }

    private static List<BeatPoint> syntheticBeatGrid(int tempoCentiBpm, int durationSeconds) {
        if (tempoCentiBpm <= 0 || durationSeconds <= 0) {
            return List.of();
        }
        double interval = 6_000_000.0 / tempoCentiBpm;
        int count = Math.max(1, (int) Math.ceil(durationSeconds * 1_000.0 / interval));
        ArrayList<BeatPoint> points = new ArrayList<>(count);
        for (int index = 0; index < count; index++) {
            points.add(new BeatPoint(index + 1, index % 4 + 1, tempoCentiBpm, Math.round(index * interval)));
        }
        return List.copyOf(points);
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
