package co.victorblan.tech.lumi.prolink.simulator;

interface SimulatorTransport {
    void triggerPreciseBurst(int playerNumber);

    ProLinkBroadcaster.Endpoint endpoint();

    int peerCount();

    ProLinkBroadcaster.TrafficDiagnostics trafficDiagnostics();
}
