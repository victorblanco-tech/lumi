package co.victorblan.tech.lumi.prolink.simulator;

interface SimulatorTransport {
    void triggerPreciseBurst(int playerNumber);

    ProLinkBroadcaster.Endpoint endpoint();
    default ProLinkBroadcaster.Endpoint endpointForPlayer(int player) { return endpoint(); }
    default String mediaStatus() { return "Not running"; }
    default void clearMediaFaults() {}
    default void mediaFault(int player, String kind, int durationMillis) { throw new IllegalStateException("Media service not running"); }

    int peerCount();

    ProLinkBroadcaster.TrafficDiagnostics trafficDiagnostics();
}
