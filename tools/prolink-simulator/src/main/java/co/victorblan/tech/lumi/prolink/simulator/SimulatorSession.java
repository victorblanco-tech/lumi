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
        List<PlayerState> players = List.of(
                new PlayerState(config.playerNumber()),
                new PlayerState(config.secondPlayerNumber())
        );
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
        try {
            RemoteControlServer remote = new RemoteControlServer(
                    library, players, autoMix, broadcaster, faults, config.bindAddress(),
                    config.controlPort(), config.controlToken()
            );
            broadcaster.start();
            remote.start();
            return new SimulatorSession(library, players, autoMix, faults, broadcaster, remote, config);
        } catch (IOException | RuntimeException failure) {
            broadcaster.close();
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
        return broadcaster.endpoint().interfaceName() + " · "
                + broadcaster.endpoint().localAddressText() + " · "
                + config.trafficProfile().externalName();
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
        faults.close();
        autoMix.close();
    }
}
