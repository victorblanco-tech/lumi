package co.victorblan.tech.lumi.prolink.simulator;

import com.fasterxml.jackson.databind.JsonNode;

import java.util.List;

final class SimulatorControls {
    private final UsbLibrary library;
    private final List<PlayerState> players;
    private final AutoMixController autoMix;
    private final SimulatorTransport transport;
    private final TrafficFaultController faults;

    SimulatorControls(
            UsbLibrary library,
            List<PlayerState> players,
            AutoMixController autoMix,
            SimulatorTransport transport,
            TrafficFaultController faults
    ) {
        this.library = library;
        this.players = List.copyOf(players);
        this.autoMix = autoMix;
        this.transport = transport;
        this.faults = faults;
    }

    void apply(String action, JsonNode body) {
        PlayerState state = requiresPlayer(action) ? player(body) : null;
        switch (action) {
            case "load" -> state.load(library.requireTrack(requiredInt(body, "trackId")));
            case "play" -> state.play();
            case "pause" -> state.pause();
            case "seek" -> state.seek(requiredLong(body, "positionMillis"));
            case "hot-cue" -> state.seek(requiredLong(body, "positionMillis"));
            case "beat-jump" -> state.jumpBeats(requiredInt(body, "beats"));
            case "pitch" -> state.setPitchPercent(requiredDouble(body, "pitchPercent"));
            case "master" -> setMaster(state, requiredBoolean(body, "enabled"));
            case "on-air" -> state.setOnAir(requiredBoolean(body, "enabled"));
            case "loop" -> state.setLoop(
                    requiredLong(body, "startMillis"), requiredLong(body, "endMillis")
            );
            case "loop-off" -> state.disableLoop();
            case "precise-burst" -> transport.triggerPreciseBurst(state.snapshot().playerNumber());
            case "player-online" -> faults.setPlayerOnline(
                    state.snapshot().playerNumber(), requiredBoolean(body, "enabled")
            );
            case "fault-position-gap" -> faults.startPositionGap(
                    state.snapshot().playerNumber(), requiredInt(body, "durationMillis")
            );
            case "fault-disconnect" -> faults.startDisconnect(
                    state.snapshot().playerNumber(), requiredInt(body, "durationMillis")
            );
            case "fault-packet-loss" -> faults.startPacketLoss(
                    state.snapshot().playerNumber(),
                    TrafficFaultController.Lane.parse(requiredText(body, "lane")),
                    requiredInt(body, "everyN"),
                    requiredInt(body, "durationMillis")
            );
            case "clear-faults" -> faults.clearFaults();
            case "master-handover" -> autoMix.transitionNowForTesting();
            case "recovery-soak" -> faults.setRecoverySoak(
                    requiredBoolean(body, "enabled"), requiredInt(body, "intervalSeconds")
            );
            case "auto-mix" -> autoMix.setEnabled(
                    requiredBoolean(body, "enabled"),
                    requiredInt(body, "intervalSeconds"),
                    optionalLong(body, "playlistId"),
                    optionalBoolean(body, "shuffle", false)
            );
            default -> throw new UnknownActionException(action);
        }
    }

    private PlayerState player(JsonNode body) {
        JsonNode requested = body.get("playerNumber");
        int playerNumber = requested == null
                ? players.getFirst().snapshot().playerNumber()
                : requiredInt(body, "playerNumber");
        return players.stream()
                .filter(player -> player.snapshot().playerNumber() == playerNumber)
                .findFirst()
                .orElseThrow(() -> new IllegalArgumentException("Unknown player number " + playerNumber));
    }

    private void setMaster(PlayerState selected, boolean enabled) {
        if (enabled) {
            players.stream().filter(player -> player != selected).forEach(player -> player.setMaster(false));
        }
        selected.setMaster(enabled);
    }

    private static boolean requiresPlayer(String action) {
        return !List.of("auto-mix", "clear-faults", "master-handover", "recovery-soak")
                .contains(action);
    }

    private static String requiredText(JsonNode body, String field) {
        JsonNode value = body.get(field);
        if (value == null || !value.isTextual() || value.textValue().isBlank()) {
            throw new IllegalArgumentException(field + " must be text");
        }
        return value.textValue();
    }

    private static int requiredInt(JsonNode body, String field) {
        JsonNode value = body.get(field);
        if (value == null || !value.canConvertToInt()) {
            throw new IllegalArgumentException(field + " must be an integer");
        }
        return value.intValue();
    }

    private static long requiredLong(JsonNode body, String field) {
        JsonNode value = body.get(field);
        if (value == null || !value.canConvertToLong()) {
            throw new IllegalArgumentException(field + " must be an integer");
        }
        return value.longValue();
    }

    private static double requiredDouble(JsonNode body, String field) {
        JsonNode value = body.get(field);
        if (value == null || !value.isNumber()) {
            throw new IllegalArgumentException(field + " must be a number");
        }
        return value.doubleValue();
    }

    private static boolean requiredBoolean(JsonNode body, String field) {
        JsonNode value = body.get(field);
        if (value == null || !value.isBoolean()) {
            throw new IllegalArgumentException(field + " must be a boolean");
        }
        return value.booleanValue();
    }

    private static Long optionalLong(JsonNode body, String field) {
        JsonNode value = body.get(field);
        if (value == null || value.isNull()) {
            return null;
        }
        if (!value.canConvertToLong()) {
            throw new IllegalArgumentException(field + " must be an integer or null");
        }
        return value.longValue();
    }

    private static boolean optionalBoolean(JsonNode body, String field, boolean fallback) {
        JsonNode value = body.get(field);
        if (value == null || value.isNull()) {
            return fallback;
        }
        if (!value.isBoolean()) {
            throw new IllegalArgumentException(field + " must be a boolean");
        }
        return value.booleanValue();
    }

    static final class UnknownActionException extends RuntimeException {
        private UnknownActionException(String action) {
            super("Unknown control action: " + action);
        }
    }
}
