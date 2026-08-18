package co.victorblan.tech.lumi.prolink;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;

public final class BridgeMain {
    private BridgeMain() {
    }

    public static void main(String[] arguments) {
        System.setProperty("java.awt.headless", "true");
        try (BridgePublisher publisher = new BridgePublisher(System.out)) {
            publisher.publish("hello", new BridgePayloads.Hello(
                    BridgeProtocol.BRIDGE_VERSION,
                    BridgeProtocol.BEAT_LINK_VERSION,
                    true
            ));
            try (BeatLinkRuntime runtime = new BeatLinkRuntime(publisher)) {
                runtime.start();
                waitForSupervisor();
            } catch (Exception failure) {
                String message = failure.getMessage() == null
                        ? failure.getClass().getSimpleName()
                        : failure.getMessage();
                System.err.println("Pro Link bridge startup failed: " + message);
                publisher.publish("error", new BridgePayloads.Error("startup", message));
            }
        }
    }

    private static void waitForSupervisor() throws Exception {
        try (BufferedReader reader = new BufferedReader(new InputStreamReader(
                System.in,
                StandardCharsets.UTF_8
        ))) {
            while (reader.readLine() != null) {
                // Protocol commands are introduced version by version. For v1,
                // EOF is the only lifecycle command and means the engine exited.
            }
        }
    }
}
