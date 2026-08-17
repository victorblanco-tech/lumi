package co.victorblan.tech.lumi.prolink.simulator;

import java.nio.file.Path;
import java.security.SecureRandom;
import java.util.Base64;

record SimulatorConfig(
        Path usbRoot,
        String networkInterface,
        int playerNumber,
        String bindAddress,
        int controlPort,
        String controlToken,
        ProLinkTrafficProfile trafficProfile
) {
    private static final int DEFAULT_CONTROL_PORT = 17_840;

    static SimulatorConfig parse(String[] arguments) {
        Path usbRoot = null;
        String networkInterface = null;
        int playerNumber = 1;
        String bindAddress = "0.0.0.0";
        int controlPort = DEFAULT_CONTROL_PORT;
        String controlToken = System.getenv("LUMI_SIM_TOKEN");
        ProLinkTrafficProfile trafficProfile = ProLinkTrafficProfile.CDJ_1500X;

        for (int index = 0; index < arguments.length; index++) {
            String argument = arguments[index];
            switch (argument) {
                case "--usb" -> usbRoot = Path.of(requiredValue(arguments, ++index, argument));
                case "--interface" -> networkInterface = requiredValue(arguments, ++index, argument);
                case "--player" -> playerNumber = Integer.parseInt(requiredValue(arguments, ++index, argument));
                case "--bind" -> bindAddress = requiredValue(arguments, ++index, argument);
                case "--port" -> controlPort = Integer.parseInt(requiredValue(arguments, ++index, argument));
                case "--token" -> controlToken = requiredValue(arguments, ++index, argument);
                case "--traffic-profile" -> trafficProfile = ProLinkTrafficProfile.parse(
                        requiredValue(arguments, ++index, argument)
                );
                case "--help", "-h" -> throw new HelpRequested();
                default -> throw new IllegalArgumentException("Unknown argument: " + argument);
            }
        }

        if (usbRoot == null) {
            throw new IllegalArgumentException("--usb is required");
        }
        if (playerNumber < 1 || playerNumber > 4) {
            throw new IllegalArgumentException("--player must be between 1 and 4");
        }
        if (controlPort < 1 || controlPort > 65_535) {
            throw new IllegalArgumentException("--port must be between 1 and 65535");
        }
        if (controlToken == null || controlToken.isBlank()) {
            controlToken = generateToken();
        }
        if (controlToken.length() < 16) {
            throw new IllegalArgumentException("Control token must contain at least 16 characters");
        }
        return new SimulatorConfig(
                usbRoot.toAbsolutePath().normalize(), networkInterface, playerNumber,
                bindAddress, controlPort, controlToken, trafficProfile
        );
    }

    static String usage() {
        return """
                Lumi Pro DJ Link Simulator (development only)

                Usage:
                  java -jar lumi-prolink-simulator.jar --usb /Volumes/REKORDBOX [options]

                Options:
                  --interface en0     Network interface used for Pro DJ Link broadcasts
                  --player 1          Simulated player number (1-4, default 1)
                  --bind 0.0.0.0      Remote control bind address
                  --port 17840        Remote control HTTP port
                  --token VALUE       Remote control token (or LUMI_SIM_TOKEN)
                  --traffic-profile cdj-1500x|classic
                                      Packet cadence (default cdj-1500x)
                """;
    }

    private static String requiredValue(String[] arguments, int index, String option) {
        if (index >= arguments.length || arguments[index].startsWith("--")) {
            throw new IllegalArgumentException("Missing value for " + option);
        }
        return arguments[index];
    }

    private static String generateToken() {
        byte[] bytes = new byte[24];
        new SecureRandom().nextBytes(bytes);
        return Base64.getUrlEncoder().withoutPadding().encodeToString(bytes);
    }

    static final class HelpRequested extends RuntimeException {
        private HelpRequested() {
            super(null, null, false, false);
        }
    }
}
