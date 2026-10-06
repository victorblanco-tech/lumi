package co.victorblan.tech.lumi.prolink;

import org.acplt.oncrpc.XdrBufferDecodingStream;
import org.acplt.oncrpc.XdrBufferEncodingStream;
import org.deepsymmetry.cratedigger.rpc.*;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.time.Duration;
import java.util.Arrays;
import java.util.List;

import static co.victorblan.tech.lumi.prolink.MediaIdentityProbeMain.*;
import static org.junit.jupiter.api.Assertions.*;

final class MediaIdentityProbeTest {
    private static final byte[] VALID = ("{\"schemaVersion\":1,\"mediaId\":\"00000000-0000-0000-0000-000000000001\","
            + "\"sourceId\":\"usb-fs:v2-fixture\"}").getBytes(StandardCharsets.UTF_8);

    @Test void portLookupUsesOnlyGetPortAndPreservesTheUnderlyingRpcFailure() throws Exception {
        assertEquals(2049, queryPort((procedure, request, result) -> {
            assertEquals(3, procedure);
            assertInstanceOf(org.acplt.oncrpc.OncRpcServerIdent.class, request);
            ((org.acplt.oncrpc.OncRpcGetPortResult) result).port = 2049;
        }, nfs.NFS_PROGRAM, nfs.NFS_VERSION));
        var timeout = new org.acplt.oncrpc.OncRpcTimeoutException();
        assertSame(timeout, assertThrows(org.acplt.oncrpc.OncRpcTimeoutException.class,
                () -> queryPort((procedure, request, result) -> { throw timeout; },
                        mount.MOUNTPROG, mount.MOUNTVERS)));
    }

    @Test void readsExactMarkerAndRechecksRemoteVersion() throws Exception {
        FakeTransport transport = new FakeTransport(VALID);
        assertArrayEquals(VALID, readMarker(transport));
        assertEquals(2, transport.lookups);
        assertEquals(1, transport.reads);
        assertEquals("usb-fs:v2-fixture", validateMarker(VALID).sourceId());
        assertEquals(64, sha256(VALID).length());
    }

    @Test void neverReadsOversizedEmptyUnsignedOrSymlinkMarkers() {
        for (int size : List.of(0, MAX_BYTES + 1, -1, Integer.MIN_VALUE)) {
            FakeTransport transport = new FakeTransport(VALID);
            transport.attrs.size = size;
            assertEquals("size_rejected", assertThrows(ProbeFailure.class, () -> readMarker(transport)).code);
            assertEquals(0, transport.reads);
        }
        FakeTransport symlink = new FakeTransport(VALID);
        symlink.attrs.type = FType.NFLNK;
        assertEquals("not_regular_file", assertThrows(ProbeFailure.class, () -> readMarker(symlink)).code);
        assertEquals(0, symlink.reads);
    }

    @Test void readsAtMostFourBoundedChunks() throws Exception {
        byte[] content = new byte[MAX_BYTES];
        Arrays.fill(content, (byte) ' ');
        FakeTransport transport = new FakeTransport(content);
        assertArrayEquals(content, readMarker(transport));
        assertEquals(4, transport.reads);
        assertEquals(CHUNK, transport.maximumCount);
    }

    @Test void rejectsShortAndOverlongReads() {
        for (int adjustment : List.of(-1, 1)) {
            FakeTransport transport = new FakeTransport(VALID);
            transport.readAdjustment = adjustment;
            assertEquals("truncated_read", assertThrows(ProbeFailure.class, () -> readMarker(transport)).code);
        }
    }

    @Test void rejectsMediaSwapAfterReadAndChangesInReadAttributes() {
        FakeTransport transport = new FakeTransport(VALID);
        transport.swapAfterRead = true;
        assertEquals("media_changed", assertThrows(ProbeFailure.class, () -> readMarker(transport)).code);
        FakeTransport changed = new FakeTransport(VALID);
        changed.changeReadAttributes = true;
        assertEquals("media_changed", assertThrows(ProbeFailure.class, () -> readMarker(changed)).code);
    }

    @Test void missingMarkerCannotProceedToRead() {
        FakeTransport transport = new FakeTransport(VALID);
        transport.missing = true;
        assertEquals("marker_missing", assertThrows(ProbeFailure.class, () -> readMarker(transport)).code);
        assertEquals(0, transport.reads);
    }

    @Test void validatesSchemaAndRejectsExtraDuplicateOrTrailingFields() {
        String valid = new String(VALID, StandardCharsets.UTF_8);
        for (String invalid : List.of(
                valid.replace("usb-fs:v2-fixture", "other-source"),
                valid.replace("00000000-0000-0000-0000-000000000001", "not-a-uuid"),
                valid.replace("\"schemaVersion\":1", "\"schemaVersion\":2"),
                valid.replace("\"schemaVersion\":1", "\"schemaVersion\":1.0"),
                valid.replace("\"schemaVersion\":1", "\"schemaVersion\":4294967297"),
                valid.replace("\"schemaVersion\":1", "\"schemaVersion\":1,\"extra\":true"),
                valid.replace("\"schemaVersion\":1", "\"schemaVersion\":1,\"schemaVersion\":1"),
                valid + " {}", "[]", "null", "{", "")) {
            assertThrows(Exception.class, () -> validateMarker(invalid.getBytes(StandardCharsets.UTF_8)), invalid);
        }
    }

    @Test void rejectsHostnamesPublicMulticastAndInvalidAddressesWithoutDns() throws Exception {
        assertEquals("192.168.1.10", privateAddress("192.168.1.10").getHostAddress());
        assertEquals("169.254.1.10", privateAddress("169.254.1.10").getHostAddress());
        for (String invalid : List.of("example.com", "127.0.0.1", "8.8.8.8", "224.0.0.1",
                "192.168.1.255", "192.168.1.256", "192.168.1.1/24", "::1")) {
            assertThrows(ProbeFailure.class, () -> privateAddress(invalid), invalid);
        }
    }

    @Test void readDecoderRejectsUntrustedLengthBeforeAllocatingPayload() throws Exception {
        for (int length : List.of(CHUNK + 1, -1, Integer.MAX_VALUE)) {
            var result = new BoundedReadResult(CHUNK);
            assertEquals("response_size_rejected", assertThrows(ProbeFailure.class,
                    () -> result.xdrDecode(encodedReadLength(length))).code);
            assertNull(result.bytes);
        }
    }

    @Test void readDecoderReportsTruncatedDatagramAndAcceptsBoundedReply() throws Exception {
        assertThrows(Exception.class, () -> new BoundedReadResult(CHUNK).xdrDecode(encodedReadLength(CHUNK)));
        var encoder = new XdrBufferEncodingStream(2048);
        encoder.xdrEncodeInt(Stat.NFS_OK);
        attrs(VALID.length).xdrEncode(encoder);
        encoder.xdrEncodeInt(VALID.length);
        encoder.xdrEncodeOpaque(VALID, 0, VALID.length);
        var result = new BoundedReadResult(VALID.length);
        result.xdrDecode(decoder(encoder));
        assertArrayEquals(VALID, result.bytes);
    }

    @Test void monotonicDeadlineExpiresWithoutUnboundedRpcTimeout() throws Exception {
        assertEquals(RPC_TIMEOUT_MS, new Deadline(Duration.ofSeconds(10)).timeout());
        assertEquals("deadline", assertThrows(ProbeFailure.class, () -> new Deadline(Duration.ZERO).timeout()).code);
    }

    @Test void supervisorKillsAndReapsHungChildAndPreservesNormalExit() throws Exception {
        Process hung = child("hang");
        assertEquals("process_deadline", assertThrows(ProbeFailure.class,
                () -> awaitChild(hung, Duration.ofMillis(300))).code);
        assertFalse(hung.isAlive());
        Process normal = child("exit");
        assertEquals(7, awaitChild(normal, Duration.ofSeconds(3)));
        assertFalse(normal.isAlive());
    }

    private static Process child(String mode) throws Exception {
        return new ProcessBuilder(Path.of(System.getProperty("java.home"), "bin", "java").toString(),
                "-cp", System.getProperty("surefire.test.class.path", System.getProperty("java.class.path")),
                TestChild.class.getName(), mode).start();
    }

    public static final class TestChild {
        public static void main(String[] args) throws Exception {
            if (args[0].equals("hang")) Thread.sleep(60_000);
            System.exit(7);
        }
    }

    private static XdrBufferDecodingStream encodedReadLength(int length) throws Exception {
        var encoder = new XdrBufferEncodingStream(256);
        encoder.xdrEncodeInt(Stat.NFS_OK);
        attrs(108).xdrEncode(encoder);
        encoder.xdrEncodeInt(length);
        return decoder(encoder);
    }

    private static XdrBufferDecodingStream decoder(XdrBufferEncodingStream encoder) throws Exception {
        var decoder = new XdrBufferDecodingStream(encoder.getXdrData(), encoder.getXdrLength());
        decoder.beginDecoding();
        return decoder;
    }

    private static FAttr attrs(int size) {
        FAttr attrs = new FAttr();
        attrs.type = FType.NFREG; attrs.size = size; attrs.fsid = 1; attrs.fileid = 42;
        attrs.mtime = new TimeVal(); attrs.ctime = new TimeVal(); attrs.atime = new TimeVal();
        return attrs;
    }

    private static final class FakeTransport implements Transport {
        final byte[] content;
        final FAttr attrs;
        int reads, lookups, maximumCount, readAdjustment;
        boolean swapAfterRead, changeReadAttributes, missing;
        FakeTransport(byte[] content) { this.content = content; this.attrs = attrs(content.length); }
        @Override public RemoteFile lookup() throws Exception {
            lookups++;
            if (missing) throw new ProbeFailure("marker_missing", "fixture: absent marker");
            byte[] handle = new byte[32];
            if (swapAfterRead && lookups > 1) handle[0] = 1;
            return new RemoteFile(new FHandle(handle), attrs);
        }
        @Override public Piece read(RemoteFile file, int offset, int count) {
            reads++; maximumCount = Math.max(maximumCount, count);
            FAttr result = attrs;
            if (changeReadAttributes) { result = attrs(content.length); result.fileid++; }
            byte[] chunk = Arrays.copyOfRange(content, offset, offset + count);
            return new Piece(result, Arrays.copyOf(chunk, count + readAdjustment));
        }
    }
}
