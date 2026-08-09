package co.victorblan.tech.lumi.prolink;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.Test;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

final class BridgePublisherTest {
    private final ObjectMapper mapper = new ObjectMapper();

    @Test
    void publishesVersionedMonotoneNdjsonWithoutDroppedEvents() throws Exception {
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        BridgePublisher publisher = new BridgePublisher(output, mapper);

        assertTrue(publisher.publish("hello", new BridgePayloads.Hello("0.4.0-dev", "8.0.0", true)));
        assertTrue(publisher.publish("sourceStatus", new BridgePayloads.SourceStatus("starting", "test")));
        publisher.close();

        String[] lines = output.toString(StandardCharsets.UTF_8).strip().split("\\R");
        assertEquals(2, lines.length);

        JsonNode hello = mapper.readTree(lines[0]);
        JsonNode status = mapper.readTree(lines[1]);
        assertEquals("lumi-prolink-bridge", hello.get("protocol").asText());
        assertEquals(1, hello.get("protocolVersion").asInt());
        assertEquals(1, hello.get("sequence").asLong());
        assertEquals("hello", hello.get("type").asText());
        assertEquals("8.0.0", hello.get("payload").get("beatLinkVersion").asText());
        assertEquals(2, status.get("sequence").asLong());
        assertEquals("sourceStatus", status.get("type").asText());
        assertEquals(0, publisher.droppedEvents());
    }
}
