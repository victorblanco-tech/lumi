package co.victorblan.tech.lumi.prolink;

import com.fasterxml.jackson.core.JsonParser;
import com.fasterxml.jackson.databind.DeserializationFeature;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.acplt.oncrpc.*;
import org.deepsymmetry.cratedigger.rpc.*;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.net.InetAddress;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.time.Duration;
import java.util.HexFormat;
import java.util.List;
import java.util.concurrent.TimeUnit;

/** Bounded read-only USB marker reader. Never instantiated by BridgeMain or the show pump. */
public final class MediaIdentityProbeMain {
    static final String MOUNT = "/C/";
    static final String FILE = ".lumi-media.json";
    static final int MAX_BYTES = 4096;
    static final int CHUNK = 1024;
    static final int RPC_TIMEOUT_MS = 1200;
    static final Duration WORKER_LIMIT = Duration.ofSeconds(5);
    static final Duration PROCESS_LIMIT = Duration.ofSeconds(8);
    static final ObjectMapper JSON = new ObjectMapper()
            .enable(JsonParser.Feature.STRICT_DUPLICATE_DETECTION)
            .enable(DeserializationFeature.FAIL_ON_TRAILING_TOKENS);

    private MediaIdentityProbeMain() {}

    public static void main(String[] args) throws Exception {
        long started = System.nanoTime();
        boolean markerOnly = args.length > 0 && (args[0].equals("--read-marker") || args[0].equals("--worker-marker"));
        boolean worker = args.length > 0 && (args[0].equals("--worker") || args[0].equals("--worker-marker"));
        try {
            int offset = worker || markerOnly ? 1 : 0;
            if (args.length != offset + (markerOnly ? 1 : 2)) {
                throw new ProbeFailure("invalid_arguments", "Use: MediaIdentityProbeMain PLAYER_IPV4 EXPECTED_LOCAL_SHA256");
            }
            InetAddress address = privateAddress(args[offset]);
            String expected = markerOnly ? "" : args[offset + 1];
            if (!markerOnly && !expected.matches("[a-f0-9]{64}")) {
                throw new ProbeFailure("invalid_arguments", "An exact local SHA-256 reference is required");
            }
            if (!worker) {
                var command = new java.util.ArrayList<>(List.of(
                        Path.of(System.getProperty("java.home"), "bin", "java").toString(),
                        "-Xmx64m", "-cp", System.getProperty("java.class.path"),
                        MediaIdentityProbeMain.class.getName(), markerOnly ? "--worker-marker" : "--worker", address.getHostAddress()));
                if (!markerOnly) command.add(expected);
                // Inherit output: no pipe can block a worker or grow a result buffer.
                Process child = new ProcessBuilder(command).inheritIO().start();
                int status = awaitChild(child, PROCESS_LIMIT);
                System.exit(status);
            }
            byte[] bytes;
            try (RpcTransport transport = new RpcTransport(address, new Deadline(WORKER_LIMIT))) {
                bytes = readMarker(transport);
            }
            Marker marker = validateMarker(bytes);
            String hash = sha256(bytes);
            boolean match = MessageDigest.isEqual(hash.getBytes(StandardCharsets.US_ASCII),
                    expected.getBytes(StandardCharsets.US_ASCII));
            System.out.println(JSON.writeValueAsString(new Result(
                    markerOnly ? "marker_read" : match ? "verified_exact_bytes" : "reference_mismatch", address.getHostAddress(), MOUNT,
                    "/" + FILE, bytes.length, hash, match, elapsedMillis(started),
                    "schema=" + marker.schemaVersion() + "; no USB writes; no Player/tempo/transport commands",
                    marker.mediaId(), marker.sourceId())));
            if (!markerOnly && !match) System.exit(1);
        } catch (Exception failure) {
            String code = failure instanceof ProbeFailure known ? known.code
                    : failure instanceof OncRpcTimeoutException ? "rpc_timeout" : "rpc_or_io_failure";
            System.out.println(JSON.writeValueAsString(new Result(code, "", MOUNT, "/" + FILE,
                    0, "", false, elapsedMillis(started), failure.getMessage(), null, null)));
            System.exit(2);
        }
    }

    static int awaitChild(Process child, Duration limit) throws Exception {
        try {
            if (!child.waitFor(limit.toMillis(), TimeUnit.MILLISECONDS)) {
                throw new ProbeFailure("process_deadline", "The isolated reader exceeded its total deadline");
            }
            return child.exitValue();
        } finally {
            if (child.isAlive()) {
                child.destroyForcibly();
                if (!child.waitFor(1, TimeUnit.SECONDS)) {
                    throw new ProbeFailure("cleanup_failed", "The isolated reader did not terminate");
                }
            }
        }
    }

    static InetAddress privateAddress(String literal) throws Exception {
        if (!literal.matches("[0-9]{1,3}(\\.[0-9]{1,3}){3}")) {
            throw new ProbeFailure("invalid_arguments", "Use a discovered private IPv4 literal; no DNS lookup");
        }
        byte[] bytes = new byte[4];
        String[] parts = literal.split("\\.");
        for (int i = 0; i < 4; i++) {
            int value = Integer.parseInt(parts[i]);
            if (value > 255) throw new ProbeFailure("invalid_arguments", "Invalid IPv4 octet");
            bytes[i] = (byte) value;
        }
        InetAddress address = InetAddress.getByAddress(bytes);
        if (!(address.isSiteLocalAddress() || address.isLinkLocalAddress())
                || address.isMulticastAddress() || (bytes[3] & 255) == 255) {
            throw new ProbeFailure("invalid_arguments", "Only a discovered private/link-local Player is permitted");
        }
        return address;
    }

    static byte[] readMarker(Transport transport) throws Exception {
        RemoteFile file = transport.lookup();
        checkFile(file.attributes());
        int size = file.attributes().size;
        ByteArrayOutputStream output = new ByteArrayOutputStream(size);
        for (int offset = 0; offset < size;) {
            int count = Math.min(CHUNK, size - offset);
            Piece piece = transport.read(file, offset, count);
            checkFile(piece.attributes());
            if (!sameVersion(file.attributes(), piece.attributes())) {
                throw new ProbeFailure("media_changed", "File attributes changed while reading the marker");
            }
            if (piece.bytes() == null || piece.bytes().length != count) {
                throw new ProbeFailure("truncated_read", "Marker read did not return the requested byte count");
            }
            output.write(piece.bytes());
            offset += count;
        }
        // Recheck identity after reading, not just after LOOKUP.
        RemoteFile after = transport.lookup();
        checkFile(after.attributes());
        if (!sameVersion(file.attributes(), after.attributes())
                || !MessageDigest.isEqual(file.handle().value, after.handle().value)) {
            throw new ProbeFailure("media_changed", "Marker changed during the read");
        }
        return output.toByteArray();
    }

    static void checkFile(FAttr attrs) throws ProbeFailure {
        if (attrs == null || attrs.type != FType.NFREG) {
            throw new ProbeFailure("not_regular_file", "The marker must be a regular file, not a directory or symlink");
        }
        if (attrs.size < 1 || attrs.size > MAX_BYTES) {
            throw new ProbeFailure("size_rejected", "Marker size is outside 1..4096 bytes; no file data was requested");
        }
    }

    static boolean sameVersion(FAttr a, FAttr b) {
        return a.type == b.type && a.size == b.size && a.fsid == b.fsid && a.fileid == b.fileid
                && sameTime(a.mtime, b.mtime) && sameTime(a.ctime, b.ctime);
    }

    private static boolean sameTime(TimeVal a, TimeVal b) {
        return a != null && b != null && a.seconds == b.seconds && a.useconds == b.useconds;
    }

    static Marker validateMarker(byte[] bytes) throws Exception {
        if (bytes.length < 1 || bytes.length > MAX_BYTES) throw new ProbeFailure("size_rejected", "Invalid marker size");
        com.fasterxml.jackson.databind.JsonNode tree;
        try { tree = JSON.readTree(bytes); }
        catch (com.fasterxml.jackson.core.JsonProcessingException failure) {
            throw new ProbeFailure("invalid_marker", "Marker is not one valid, duplicate-free JSON object");
        }
        if (tree == null || !tree.isObject() || tree.size() != 3
                || !tree.has("schemaVersion") || !tree.has("mediaId") || !tree.has("sourceId")
                || !tree.get("schemaVersion").isIntegralNumber() || !tree.get("schemaVersion").canConvertToInt()
                || tree.get("schemaVersion").intValue() != 1
                || !tree.get("mediaId").isTextual() || !tree.get("sourceId").isTextual()) {
            throw new ProbeFailure("invalid_marker", "Unsupported schema or fields");
        }
        String mediaId = tree.get("mediaId").textValue();
        String sourceId = tree.get("sourceId").textValue();
        if (!mediaId.matches("[a-fA-F0-9]{8}(-[a-fA-F0-9]{4}){3}-[a-fA-F0-9]{12}")
                || sourceId.length() > 200 || !sourceId.matches("usb-(fs|local):[a-zA-Z0-9:_-]+")) {
            throw new ProbeFailure("invalid_marker", "Invalid media or source identity");
        }
        return new Marker(1, mediaId, sourceId);
    }

    static String sha256(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private static double elapsedMillis(long started) { return (System.nanoTime() - started) / 1_000_000.0; }

    record Marker(int schemaVersion, String mediaId, String sourceId) {}
    record Result(String outcome, String playerAddress, String mount, String path, int bytes,
                  String sha256, boolean exactLocalMatch, double elapsedMillis, String detail,
                  String mediaId, String sourceId) {}
    record RemoteFile(FHandle handle, FAttr attributes) {}
    record Piece(FAttr attributes, byte[] bytes) {}
    interface Transport { RemoteFile lookup() throws Exception; Piece read(RemoteFile file, int offset, int count) throws Exception; }

    static final class ProbeFailure extends IOException {
        final String code;
        ProbeFailure(String code, String message) { super(message); this.code = code; }
    }

    static final class Deadline {
        private final long end;
        Deadline(Duration budget) { end = System.nanoTime() + budget.toNanos(); }
        int timeout() throws ProbeFailure {
            long remaining = end - System.nanoTime();
            if (remaining <= 0) throw new ProbeFailure("deadline", "NFS read deadline expired");
            return (int) Math.max(1, Math.min(RPC_TIMEOUT_MS, TimeUnit.NANOSECONDS.toMillis(remaining)));
        }
    }

    static final class RpcTransport implements Transport, AutoCloseable {
        private final InetAddress address;
        private final Deadline deadline;
        private final FHandle root;
        private OncRpcUdpClient mountClient;
        private OncRpcUdpClient nfsClient;

        RpcTransport(InetAddress address, Deadline deadline) throws Exception {
            this.address = address;
            this.deadline = deadline;
            try {
                mountClient = open(mount.MOUNTPROG, mount.MOUNTVERS);
                FHStatus result = new FHStatus();
                prepare(mountClient);
                mountClient.call(mount.MOUNTPROC_MNT_1, new DirPath(MOUNT.getBytes(StandardCharsets.UTF_16LE)), result);
                checkStatus(result.status, "mount");
                root = result.directory;
                nfsClient = open(nfs.NFS_PROGRAM, nfs.NFS_VERSION);
            } catch (Exception failure) {
                close();
                throw failure;
            }
        }

        private OncRpcUdpClient open(int program, int version) throws Exception {
            // Explicit portmapper timeout: the convenience factory otherwise waits 30 seconds.
            OncRpcPortmapClient portmap = new OncRpcPortmapClient(address, OncRpcProtocols.ONCRPC_UDP);
            int port;
            try {
                prepare((OncRpcUdpClient) portmap.getOncRpcClient());
                port = portmap.getPort(program, version, OncRpcProtocols.ONCRPC_UDP);
            } finally {
                portmap.close();
            }
            if (port < 1 || port > 65535) throw new ProbeFailure("service_unavailable", "NFS/mount service is not registered");
            OncRpcUdpClient client = new OncRpcUdpClient(address, program, version, port, 8192);
            try { prepare(client); return client; }
            catch (Exception failure) { client.close(); throw failure; }
        }

        private void prepare(OncRpcUdpClient client) throws ProbeFailure {
            client.setTimeout(deadline.timeout());
            client.setRetransmissionTimeout(250);
            client.setRetransmissionMode(OncRpcUdpRetransmissionMode.EXPONENTIAL);
        }

        @Override public RemoteFile lookup() throws Exception {
            DirOpArgs args = new DirOpArgs();
            args.dir = root;
            args.name = new Filename(FILE.getBytes(StandardCharsets.UTF_16LE));
            DirOpRes result = new DirOpRes();
            prepare(nfsClient);
            nfsClient.call(nfs.NFSPROC_LOOKUP_2, args, result);
            checkStatus(result.status, "lookup");
            return new RemoteFile(result.diropok.file, result.diropok.attributes);
        }

        @Override public Piece read(RemoteFile file, int offset, int count) throws Exception {
            ReadArgs args = new ReadArgs();
            args.file = file.handle(); args.offset = offset; args.count = count;
            BoundedReadResult result = new BoundedReadResult(count);
            prepare(nfsClient);
            nfsClient.call(nfs.NFSPROC_READ_2, args, result);
            checkStatus(result.status, "read");
            return new Piece(result.attributes, result.bytes);
        }

        private static void checkStatus(int status, String stage) throws ProbeFailure {
            if (status != Stat.NFS_OK) {
                String code = status == Stat.NFSERR_NOENT ? "marker_missing"
                        : status == Stat.NFSERR_ACCES || status == Stat.NFSERR_PERM ? "access_denied"
                        : status == Stat.NFSERR_STALE ? "media_changed" : "nfs_error";
                throw new ProbeFailure(code, stage + " returned NFS status " + status);
            }
        }

        @Override public void close() {
            if (nfsClient != null) { try { nfsClient.close(); } catch (OncRpcException ignored) {} nfsClient = null; }
            if (mountClient != null) { try { mountClient.close(); } catch (OncRpcException ignored) {} mountClient = null; }
        }
    }

    /** Check the XDR payload length before allocation, not after decoding untrusted NFSData. */
    static final class BoundedReadResult implements XdrAble {
        private final int requested;
        int status;
        FAttr attributes;
        byte[] bytes;
        BoundedReadResult(int requested) {
            if (requested < 1 || requested > CHUNK) throw new IllegalArgumentException("Invalid READ bound");
            this.requested = requested;
        }
        @Override public void xdrEncode(XdrEncodingStream xdr) throws IOException {
            throw new IOException("This result is decode-only");
        }
        @Override public void xdrDecode(XdrDecodingStream xdr) throws OncRpcException, IOException {
            status = xdr.xdrDecodeInt();
            if (status != Stat.NFS_OK) return;
            attributes = new FAttr(xdr);
            checkFile(attributes);
            int length = xdr.xdrDecodeInt();
            if (length < 0 || length > requested) {
                throw new ProbeFailure("response_size_rejected", "NFS response exceeds the requested chunk size");
            }
            bytes = xdr.xdrDecodeOpaque(length);
        }
    }
}
