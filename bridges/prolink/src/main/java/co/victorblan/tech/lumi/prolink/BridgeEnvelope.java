package co.victorblan.tech.lumi.prolink;

record BridgeEnvelope(
        String protocol,
        int protocolVersion,
        long sequence,
        long observedAtNanos,
        String type,
        Object payload
) {
}
