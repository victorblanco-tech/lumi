package co.victorblan.tech.lumi.prolink;

record BridgeEnvelope(
        String protocol,
        int protocolVersion,
        long sequence,
        long observedAtNanos,
        String trafficClass,
        long bridgeQueueAgeMicros,
        String type,
        Object payload
) {
}
