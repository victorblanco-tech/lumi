package co.victorblan.tech.lumi.prolink.simulator;

import java.io.IOException;
import java.util.List;
import java.util.concurrent.atomic.AtomicBoolean;

final class SimulatorSession implements AutoCloseable {
    private final UsbLibrary library;
    private final List<PlayerState> players;
    private final AutoMixController autoMix;
    private final TrafficFaultController faults;
    private final ProLinkBroadcaster broadcaster;
    private final RemoteControlServer remote;
    private final SimulatorConfig config;
    private final AtomicBoolean closed = new AtomicBoolean();
    private MediaRpcService media;

    private SimulatorSession(
            UsbLibrary library,
            List<PlayerState> players,
            AutoMixController autoMix,
            TrafficFaultController faults,
            ProLinkBroadcaster broadcaster,
            RemoteControlServer remote,
            SimulatorConfig config
    ) {
        this.library = library;
        this.players = players;
        this.autoMix = autoMix;
        this.faults = faults;
        this.broadcaster = broadcaster;
        this.remote = remote;
        this.config = config;
    }

    static SimulatorSession start(SimulatorConfig config) throws IOException {
        UsbLibrary library = UsbLibrary.open(config.usbRoot());
        UsbLibrary secondLibrary = config.secondUsbRoot() == null
                ? null
                : UsbLibrary.open(config.secondUsbRoot());
        if (secondLibrary != null && secondLibrary.root().equals(library.root()))
            throw new IOException("The same USB cannot occupy both Players. Leave Player 2's USB empty and load it over LINK from Player 1.");
        List<PlayerState> players = List.of(
                new PlayerState(config.playerNumber()),
                new PlayerState(config.secondPlayerNumber())
        );
        players.getFirst().configureUsb(library);
        if (secondLibrary != null) players.get(1).configureUsb(secondLibrary);
        AutoMixController autoMix = new AutoMixController(players, library);
        TrafficFaultController faults = new TrafficFaultController(players, autoMix);
        ProLinkBroadcaster broadcaster;
        try {
            broadcaster = new ProLinkBroadcaster(
                    players, config.networkInterface(), config.trafficProfile(), faults
            );
        } catch (IOException | RuntimeException failure) {
            faults.close();
            autoMix.close();
            throw failure;
        }
        MediaRpcService media = null;
        try {
            if (secondLibrary != null && !broadcaster.independentSources())
                throw new IOException("Only one network address available. Use one USB in Player 1; Player 2 loads over LINK. A second independent USB needs a second active connection on the same LAN.");
            media = MediaRpcService.start(players, broadcaster);
            broadcaster.attachMedia(media);
            RemoteControlServer remote = new RemoteControlServer(
                    library, players, autoMix, broadcaster, faults, config.bindAddress(),
                    config.controlPort(), config.controlToken()
            );
            broadcaster.start();
            remote.start();
            SimulatorSession session = new SimulatorSession(library, players, autoMix, faults, broadcaster, remote, config);
            session.media = media;
            return session;
        } catch (IOException | RuntimeException failure) {
            broadcaster.close();
            if (media != null) media.close();
            faults.close();
            autoMix.close();
            throw failure;
        }
    }

    String remoteUrl() {
        String remoteHost = "127.0.0.1".equals(config.bindAddress())
                ? "127.0.0.1"
                : broadcaster.endpoint().localAddressText();
        return "http://" + remoteHost + ":" + remote.port() + "/?token=" + config.controlToken();
    }

    String networkSummary() {
        return players.stream().map(p -> "P" + p.snapshot().playerNumber() + " "
                + broadcaster.endpointForPlayer(p.snapshot().playerNumber()).interfaceName() + " "
                + broadcaster.endpointForPlayer(p.snapshot().playerNumber()).localAddressText())
                .collect(java.util.stream.Collectors.joining(" · "));
    }

    UsbLibrary library() {
        return library;
    }

    List<PlayerState> players() {
        return players;
    }

    @Override
    public void close() {
        if (!closed.compareAndSet(false, true)) {
            return;
        }
        remote.close();
        broadcaster.close();
        if (media != null) media.close();
        faults.close();
        autoMix.close();
    }
}
