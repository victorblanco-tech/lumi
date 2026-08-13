package co.victorblan.tech.lumi.prolink.simulator;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpHandler;
import com.sun.net.httpserver.HttpServer;

import java.io.IOException;
import java.io.InputStream;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.URLDecoder;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

final class RemoteControlServer implements AutoCloseable {
    private static final int MAX_REQUEST_BYTES = 64 * 1024;
    private static final ObjectMapper JSON = new ObjectMapper();

    private final UsbLibrary library;
    private final PlayerState state;
    private final ProLinkBroadcaster broadcaster;
    private final String token;
    private final HttpServer server;
    private final ExecutorService executor = Executors.newVirtualThreadPerTaskExecutor();

    RemoteControlServer(
            UsbLibrary library,
            PlayerState state,
            ProLinkBroadcaster broadcaster,
            String bindAddress,
            int port,
            String token
    ) throws IOException {
        this.library = library;
        this.state = state;
        this.broadcaster = broadcaster;
        this.token = token;
        server = HttpServer.create(new InetSocketAddress(InetAddress.getByName(bindAddress), port), 0);
        server.setExecutor(executor);
        server.createContext("/api/v1/health", this::health);
        server.createContext("/api/v1/status", authenticated(this::status));
        server.createContext("/api/v1/tracks", authenticated(this::tracks));
        server.createContext("/api/v1/control", authenticated(this::control));
        server.createContext("/", this::web);
    }

    void start() {
        server.start();
    }

    int port() {
        return server.getAddress().getPort();
    }

    private void health(HttpExchange exchange) throws IOException {
        if (!method(exchange, "GET")) {
            return;
        }
        sendJson(exchange, 200, Map.of(
                "status", "ready",
                "service", "lumi-prolink-simulator",
                "version", "0.4.0-dev-26"
        ));
    }

    private void status(HttpExchange exchange) throws IOException {
        if (!method(exchange, "GET")) {
            return;
        }
        sendJson(exchange, 200, statusPayload());
    }

    private void tracks(HttpExchange exchange) throws IOException {
        if (!method(exchange, "GET")) {
            return;
        }
        Map<String, String> query = query(exchange.getRequestURI().getRawQuery());
        String search = query.getOrDefault("q", "");
        int limit = parseInteger(query.getOrDefault("limit", "100"), "limit");
        List<UsbLibrary.TrackSummary> tracks = library.search(search, limit);
        sendJson(exchange, 200, Map.of("tracks", tracks, "count", tracks.size()));
    }

    private void control(HttpExchange exchange) throws IOException {
        if (!method(exchange, "POST")) {
            return;
        }
        String path = exchange.getRequestURI().getPath();
        String prefix = "/api/v1/control/";
        if (!path.startsWith(prefix) || path.length() == prefix.length()) {
            sendError(exchange, 404, "Unknown control endpoint");
            return;
        }
        String action = path.substring(prefix.length());
        try {
            JsonNode body = readBody(exchange);
            switch (action) {
                case "load" -> state.load(library.requireTrack(requiredInt(body, "trackId")));
                case "play" -> state.play();
                case "pause" -> state.pause();
                case "seek" -> state.seek(requiredLong(body, "positionMillis"));
                case "pitch" -> state.setPitchPercent(requiredDouble(body, "pitchPercent"));
                case "master" -> state.setMaster(requiredBoolean(body, "enabled"));
                case "on-air" -> state.setOnAir(requiredBoolean(body, "enabled"));
                default -> {
                    sendError(exchange, 404, "Unknown control action: " + action);
                    return;
                }
            }
            sendJson(exchange, 200, statusPayload());
        } catch (IllegalArgumentException | IllegalStateException failure) {
            sendError(exchange, 400, failure.getMessage());
        }
    }

    private void web(HttpExchange exchange) throws IOException {
        if (!method(exchange, "GET")) {
            return;
        }
        String path = exchange.getRequestURI().getPath();
        if (!"/".equals(path) && !"/index.html".equals(path)) {
            sendError(exchange, 404, "Not found");
            return;
        }
        try (InputStream resource = RemoteControlServer.class.getResourceAsStream("/web/index.html")) {
            if (resource == null) {
                sendError(exchange, 500, "Simulator control page is missing");
                return;
            }
            byte[] content = resource.readAllBytes();
            exchange.getResponseHeaders().set("Content-Type", "text/html; charset=utf-8");
            exchange.getResponseHeaders().set("Cache-Control", "no-store");
            exchange.sendResponseHeaders(200, content.length);
            exchange.getResponseBody().write(content);
            exchange.close();
        }
    }

    private Map<String, Object> statusPayload() {
        PlayerState.Snapshot snapshot = state.snapshot();
        LinkedHashMap<String, Object> payload = new LinkedHashMap<>();
        payload.put("service", "lumi-prolink-simulator");
        payload.put("playerNumber", snapshot.playerNumber());
        payload.put("playing", snapshot.playing());
        payload.put("master", snapshot.master());
        payload.put("onAir", snapshot.onAir());
        payload.put("pitchPercent", snapshot.pitchPercent());
        payload.put("effectiveBpm", snapshot.effectiveBpm());
        payload.put("positionMillis", snapshot.positionMillis());
        payload.put("beatNumber", snapshot.beatNumber());
        payload.put("beatWithinBar", snapshot.beatWithinBar());
        payload.put("revision", snapshot.revision());
        payload.put("usbRoot", library.root().toString());
        payload.put("usbTrackCount", library.size());
        ProLinkBroadcaster.Endpoint networkEndpoint = broadcaster.endpoint();
        payload.put("networkInterface", networkEndpoint.interfaceName());
        payload.put("networkAddress", networkEndpoint.localAddressText());
        payload.put("broadcastAddress", networkEndpoint.broadcastAddressText());
        payload.put("proLinkPeerCount", broadcaster.peerCount());
        if (snapshot.track() == null) {
            payload.put("track", null);
        } else {
            payload.put("track", UsbLibrary.TrackSummary.from(snapshot.track()));
        }
        return payload;
    }

    private Handler authenticated(Handler next) {
        return exchange -> {
            String authorization = exchange.getRequestHeaders().getFirst("Authorization");
            String supplied = authorization != null && authorization.startsWith("Bearer ")
                    ? authorization.substring("Bearer ".length())
                    : "";
            if (!MessageDigest.isEqual(
                    supplied.getBytes(StandardCharsets.UTF_8), token.getBytes(StandardCharsets.UTF_8)
            )) {
                exchange.getResponseHeaders().set("WWW-Authenticate", "Bearer");
                sendError(exchange, 401, "A valid simulator control token is required");
                return;
            }
            try {
                next.handle(exchange);
            } catch (IllegalArgumentException | IllegalStateException failure) {
                sendError(exchange, 400, failure.getMessage());
            } catch (RuntimeException failure) {
                System.err.println("Simulator control request failed: " + failure.getMessage());
                sendError(exchange, 500, "Simulator control request failed");
            }
        };
    }

    private static JsonNode readBody(HttpExchange exchange) throws IOException {
        byte[] bytes = exchange.getRequestBody().readNBytes(MAX_REQUEST_BYTES + 1);
        if (bytes.length > MAX_REQUEST_BYTES) {
            throw new IllegalArgumentException("Request body is too large");
        }
        if (bytes.length == 0) {
            return JSON.createObjectNode();
        }
        return JSON.readTree(bytes);
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

    private static boolean method(HttpExchange exchange, String expected) throws IOException {
        if (expected.equals(exchange.getRequestMethod())) {
            return true;
        }
        exchange.getResponseHeaders().set("Allow", expected);
        sendError(exchange, 405, "Method not allowed");
        return false;
    }

    private static Map<String, String> query(String rawQuery) {
        LinkedHashMap<String, String> result = new LinkedHashMap<>();
        if (rawQuery == null || rawQuery.isBlank()) {
            return result;
        }
        for (String pair : rawQuery.split("&")) {
            String[] parts = pair.split("=", 2);
            String key = URLDecoder.decode(parts[0], StandardCharsets.UTF_8);
            String value = parts.length == 2
                    ? URLDecoder.decode(parts[1], StandardCharsets.UTF_8)
                    : "";
            result.put(key, value);
        }
        return result;
    }

    private static int parseInteger(String value, String name) {
        try {
            return Integer.parseInt(value);
        } catch (NumberFormatException failure) {
            throw new IllegalArgumentException(name + " must be an integer", failure);
        }
    }

    private static void sendJson(HttpExchange exchange, int status, Object value) throws IOException {
        byte[] content = JSON.writeValueAsBytes(value);
        exchange.getResponseHeaders().set("Content-Type", "application/json; charset=utf-8");
        exchange.getResponseHeaders().set("Cache-Control", "no-store");
        exchange.getResponseHeaders().set("X-Content-Type-Options", "nosniff");
        exchange.sendResponseHeaders(status, content.length);
        exchange.getResponseBody().write(content);
        exchange.close();
    }

    private static void sendError(HttpExchange exchange, int status, String message) throws IOException {
        sendJson(exchange, status, Map.of("error", message == null ? "Unknown error" : message));
    }

    @Override
    public void close() {
        server.stop(0);
        executor.shutdownNow();
    }

    @FunctionalInterface
    private interface Handler extends HttpHandler {
    }
}
