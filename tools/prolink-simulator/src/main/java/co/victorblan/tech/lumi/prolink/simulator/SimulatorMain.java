package co.victorblan.tech.lumi.prolink.simulator;

import java.util.concurrent.CountDownLatch;

public final class SimulatorMain {
    private SimulatorMain() {
    }

    public static void main(String[] arguments) throws Exception {
        SimulatorConfig config;
        try {
            config = SimulatorConfig.parse(arguments);
        } catch (SimulatorConfig.HelpRequested ignored) {
            System.out.println(SimulatorConfig.usage());
            return;
        } catch (IllegalArgumentException failure) {
            System.err.println(failure.getMessage());
            System.err.println();
            System.err.println(SimulatorConfig.usage());
            System.exit(2);
            return;
        }

        UsbLibrary library = UsbLibrary.open(config.usbRoot());
        PlayerState player = new PlayerState(config.playerNumber());
        ProLinkBroadcaster broadcaster = new ProLinkBroadcaster(player, config.networkInterface());
        RemoteControlServer remote = new RemoteControlServer(
                library, player, broadcaster.endpoint(), config.bindAddress(),
                config.controlPort(), config.controlToken()
        );
        CountDownLatch shutdown = new CountDownLatch(1);
        Runtime.getRuntime().addShutdownHook(Thread.ofPlatform().unstarted(() -> {
            remote.close();
            broadcaster.close();
            shutdown.countDown();
        }));

        broadcaster.start();
        remote.start();
        String remoteHost = "127.0.0.1".equals(config.bindAddress())
                ? "127.0.0.1"
                : broadcaster.endpoint().localAddressText();
        System.out.println("Lumi Pro DJ Link Simulator is ready");
        System.out.println("USB: " + library.root() + " (" + library.size() + " tracks)");
        System.out.println("Player: " + config.playerNumber());
        System.out.println("Network: " + broadcaster.endpoint().interfaceName()
                + " / " + broadcaster.endpoint().localAddressText());
        System.out.println("Remote control: http://" + remoteHost + ":" + remote.port()
                + "/?token=" + config.controlToken());
        System.out.println("API token: " + config.controlToken());
        shutdown.await();
    }
}
