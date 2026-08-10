package co.victorblan.tech.lumi.prolink.simulator;

import java.io.IOException;
import java.util.concurrent.atomic.AtomicBoolean;

final class SimulatorSession implements AutoCloseable {
    private final UsbLibrary library;
    private final PlayerState player;
    private final ProLinkBroadcaster broadcaster;
    private final RemoteControlServer remote;
    private final SimulatorConfig config;
    private final AtomicBoolean closed = new AtomicBoolean();

    private SimulatorSession(
            UsbLibrary library,
            PlayerState player,
            ProLinkBroadcaster broadcaster,
            RemoteControlServer remote,
            SimulatorConfig config
    ) {
        this.library = library;
        this.player = player;
        this.broadcaster = broadcaster;
        this.remote = remote;
        this.config = config;
    }

    static SimulatorSession start(SimulatorConfig config) throws IOException {
        UsbLibrary library = UsbLibrary.open(config.usbRoot());
        PlayerState player = new PlayerState(config.playerNumber());
        ProLinkBroadcaster broadcaster = new ProLinkBroadcaster(player, config.networkInterface());
        try {
            RemoteControlServer remote = new RemoteControlServer(
                    library, player, broadcaster, config.bindAddress(),
                    config.controlPort(), config.controlToken()
            );
            broadcaster.start();
            remote.start();
            return new SimulatorSession(library, player, broadcaster, remote, config);
        } catch (IOException | RuntimeException failure) {
            broadcaster.close();
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
                + broadcaster.endpoint().localAddressText();
    }

    UsbLibrary library() {
        return library;
    }

    PlayerState player() {
        return player;
    }

    @Override
    public void close() {
        if (!closed.compareAndSet(false, true)) {
            return;
        }
        remote.close();
        broadcaster.close();
    }
}
