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

        SimulatorSession session = SimulatorSession.start(config);
        CountDownLatch shutdown = new CountDownLatch(1);
        Runtime.getRuntime().addShutdownHook(Thread.ofPlatform().unstarted(() -> {
            session.close();
            shutdown.countDown();
        }));

        System.out.println("Lumi Pro DJ Link Simulator is ready");
        System.out.println("USB: " + session.library().root() + " ("
                + session.library().size() + " tracks, "
                + session.library().playlistCount() + " playlists)");
        System.out.println("Players: " + config.playerNumber() + " and " + config.secondPlayerNumber());
        System.out.println("Network: " + session.networkSummary());
        System.out.println("Remote control: " + session.remoteUrl());
        System.out.println("API token: " + config.controlToken());
        shutdown.await();
    }
}
