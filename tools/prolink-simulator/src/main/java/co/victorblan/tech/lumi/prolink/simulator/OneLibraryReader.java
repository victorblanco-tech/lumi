package co.victorblan.tech.lumi.prolink.simulator;

import com.fasterxml.jackson.core.JsonParser;
import com.fasterxml.jackson.databind.DeserializationFeature;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.concurrent.TimeUnit;

/** Calls the bundled, read-only Rust importer; never opens the Lumi database. */
final class OneLibraryReader {
    private static final long MAX_BYTES = 16 * 1024 * 1024;
    private static final ObjectMapper JSON = new ObjectMapper()
            .enable(JsonParser.Feature.STRICT_DUPLICATE_DETECTION)
            .enable(DeserializationFeature.FAIL_ON_TRAILING_TOKENS);

    private OneLibraryReader() {}

    static JsonNode read(Path root) throws IOException {
        Path helper = helperPath();
        if (!Files.isExecutable(helper)) {
            throw new IOException("The bundled OneLibrary reader is missing. Reinstall the simulator.");
        }
        Path output = Files.createTempFile("lumi-simulator-media-", ".json");
        Process process = null;
        try {
            process = new ProcessBuilder(helper.toString(), root.toString())
                    .redirectErrorStream(true).redirectOutput(output.toFile()).start();
            if (!process.waitFor(90, TimeUnit.SECONDS)) {
                throw new IOException("OneLibrary scan exceeded 90 seconds; the USB was not modified.");
            }
            if (Files.size(output) > MAX_BYTES) {
                throw new IOException("OneLibrary result exceeds the simulator size limit.");
            }
            if (process.exitValue() != 0) {
                String message = Files.readString(output);
                throw new IOException(message.substring(0, Math.min(1500, message.length())).trim());
            }
            JsonNode result = JSON.readTree(output.toFile());
            if (result == null || result.path("schemaVersion").asInt() != 1
                    || !"OneLibrary".equals(result.path("format").asText())
                    || !result.path("tracks").isArray() || !result.path("playlists").isArray()) {
                throw new IOException("Unsupported OneLibrary reader result.");
            }
            return result;
        } catch (InterruptedException failure) {
            Thread.currentThread().interrupt();
            throw new IOException("OneLibrary scan cancelled", failure);
        } finally {
            if (process != null && process.isAlive()) {
                process.destroyForcibly();
                try {
                    process.waitFor(2, TimeUnit.SECONDS);
                } catch (InterruptedException failure) {
                    Thread.currentThread().interrupt();
                }
            }
            Files.deleteIfExists(output);
        }
    }

    private static Path helperPath() throws IOException {
        String configured = System.getenv("LUMI_SIM_MEDIA_READER");
        if (configured != null && !configured.isBlank()) return Path.of(configured);
        try {
            Path jar = Path.of(OneLibraryReader.class.getProtectionDomain()
                    .getCodeSource().getLocation().toURI());
            return jar.getParent().resolve("lumi-simulator-media");
        } catch (Exception failure) {
            throw new IOException("Cannot locate the bundled OneLibrary reader", failure);
        }
    }
}
