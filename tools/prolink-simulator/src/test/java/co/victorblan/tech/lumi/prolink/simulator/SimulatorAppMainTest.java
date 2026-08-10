package co.victorblan.tech.lumi.prolink.simulator;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;

class SimulatorAppMainTest {
    @Test
    void findsOnlyVolumesWithARekordboxDatabase(@TempDir Path volumes) throws Exception {
        Path first = Files.createDirectories(volumes.resolve("USB B/PIONEER/rekordbox"));
        Files.createFile(first.resolve("export.pdb"));
        Files.createDirectories(volumes.resolve("NOT A DJ USB"));
        Path second = Files.createDirectories(volumes.resolve("USB A/PIONEER/rekordbox"));
        Files.createFile(second.resolve("export.pdb"));

        assertEquals(
                java.util.List.of(volumes.resolve("USB A"), volumes.resolve("USB B")),
                SimulatorAppMain.findRekordboxVolumes(volumes)
        );
    }
}
