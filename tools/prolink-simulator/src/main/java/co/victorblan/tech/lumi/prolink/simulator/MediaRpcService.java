package co.victorblan.tech.lumi.prolink.simulator;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

/** Disposable unprivileged child; owns no system configuration or persistent services. */
final class MediaRpcService implements AutoCloseable {
    private final Process process;
    private final List<PlayerState> sources;
    private final long[] generations;
    private final int[] faults;
    private final long[] faultEnds;
    private final ScheduledExecutorService monitor = Executors.newSingleThreadScheduledExecutor(
            Thread.ofPlatform().daemon(true).name("simulator-media-lifecycle").factory());
    private volatile String error;

    static MediaRpcService start(List<PlayerState> players, SimulatorTransport network) throws IOException {
        Path binary;
        try {
            Path location = Path.of(MediaRpcService.class.getProtectionDomain().getCodeSource().getLocation().toURI());
            binary = location.getParent().resolve("lumi-simulator-rpc");
        } catch (Exception failure) { throw new IOException("Cannot locate the bundled media service", failure); }
        String configured = System.getenv("LUMI_SIM_RPC_BINARY");
        if (configured != null && !configured.isBlank()) binary = Path.of(configured);
        if (!Files.isExecutable(binary)) throw new IOException("USB network service is missing. Reinstall the simulator.");
        List<PlayerState> sources = players.stream().filter(p -> p.usb().configured()).toList();
        ArrayList<String> args = new ArrayList<>(List.of(binary.toString(), "111"));
        for (PlayerState source : sources) {
            args.add(network.endpointForPlayer(source.snapshot().playerNumber()).localAddressText());
            args.add(source.usb().configuredLibrary().root().toString());
        }
        Process process = new ProcessBuilder(args).redirectError(ProcessBuilder.Redirect.INHERIT).start();
        try {
            long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(3);
            while (process.isAlive() && process.getInputStream().available() < 6 && System.nanoTime() < deadline)
                Thread.sleep(10);
            if (process.getInputStream().available() < 6 ||
                    !"READY\n".equals(new String(process.getInputStream().readNBytes(6), StandardCharsets.US_ASCII)))
                throw new IOException("USB network service could not open RPC port 111. Check for an existing simulator or RPC service; nothing was stopped or changed.");
            return new MediaRpcService(process, sources);
        } catch (InterruptedException failure) {
            Thread.currentThread().interrupt(); process.destroyForcibly(); throw new IOException("Media startup cancelled", failure);
        } catch (IOException failure) { process.destroyForcibly(); throw failure; }
    }

    private MediaRpcService(Process process, List<PlayerState> sources) {
        this.process = process; this.sources = sources;
        generations = new long[sources.size()]; faults = new int[sources.size()]; faultEnds = new long[sources.size()];
        for (int i=0; i<sources.size(); i++) generations[i] = sources.get(i).usb().current().generation();
        monitor.scheduleWithFixedDelay(this::refresh, 0, 25, TimeUnit.MILLISECONDS);
    }

    synchronized void fault(int player, String kind, int durationMillis) {
        if (durationMillis < 0 || durationMillis > 30_000) throw new IllegalArgumentException("Media fault duration must be 0..30000 ms");
        int fault = switch (kind) { case "none" -> 0; case "timeout" -> 1; case "missing" -> 2;
            case "malformed" -> 3; default -> throw new IllegalArgumentException("Unknown media fault"); };
        for (int i=0; i<sources.size(); i++) if (sources.get(i).snapshot().playerNumber()==player) {
            faults[i]=fault; faultEnds[i]=System.nanoTime()+TimeUnit.MILLISECONDS.toNanos(durationMillis);
            writeState(i, generations[i]!=0); return;
        }
        throw new IllegalArgumentException("This Player has no independent USB source");
    }

    private synchronized void refresh() {
        if (!process.isAlive()) { error="USB network service stopped; restart simulator"; return; }
        for (int i=0; i<sources.size(); i++) {
            MediaSlot.Mount mount=sources.get(i).usb().current();
            long generation=mount==null?0:mount.generation();
            if (generation!=generations[i]) {
                // Preserve insertion epochs even when eject/reinsert occurred between observations.
                if (generation!=0 && generations[i]!=0) writeState(i,false);
                writeState(i,generation!=0); generations[i]=generation;
            }
            if (faults[i]!=0 && System.nanoTime()>=faultEnds[i]) { faults[i]=0; writeState(i,generation!=0); }
        }
    }

    private void writeState(int index, boolean present) {
        try {
            process.getOutputStream().write((index+" "+(present?1:0)+" "+faults[index]+"\n").getBytes(StandardCharsets.US_ASCII));
            process.getOutputStream().flush();
        } catch (IOException failure) { error="USB network service disconnected; restart simulator"; }
    }

    synchronized String status() {
        if (error != null) return error;
        if (!process.isAlive()) return "USB network service stopped";
        for (int i=0;i<faults.length;i++) if(faults[i]!=0)
            return "Media fault active on Player " + sources.get(i).snapshot().playerNumber()
                    + " · " + switch(faults[i]) {case 1 -> "timeout";case 2 -> "marker missing";default -> "invalid marker";};
        return "Read-only RPC/NFS ready";
    }

    synchronized void clearFaults() {
        for (int i=0; i<faults.length; i++) { faults[i]=0; writeState(i,generations[i]!=0); }
    }

    @Override public void close() {
        monitor.shutdownNow();
        try { process.getOutputStream().close(); } catch (IOException ignored) {}
        try { if (!process.waitFor(2,TimeUnit.SECONDS)) process.destroyForcibly(); }
        catch (InterruptedException interrupted) { Thread.currentThread().interrupt(); process.destroyForcibly(); }
    }
}
