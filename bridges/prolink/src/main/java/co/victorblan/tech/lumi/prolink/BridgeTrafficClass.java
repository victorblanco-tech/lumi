package co.victorblan.tech.lumi.prolink;

enum BridgeTrafficClass {
    CRITICAL("critical"),
    TEMPO("tempo"),
    TRANSPORT("transport"),
    DISPLAY("display");

    private final String externalName;

    BridgeTrafficClass(String externalName) {
        this.externalName = externalName;
    }

    String externalName() {
        return externalName;
    }
}
