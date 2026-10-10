package co.victorblan.tech.lumi.prolink.simulator;

import java.util.Objects;

/** A physical slot's insertion lifetime, separate from any loaded/cached track. */
final class MediaSlot {
    private final int playerNumber;
    private long generation;
    private volatile Mount current;
    private volatile UsbLibrary configuredLibrary;

    MediaSlot(int playerNumber) { this.playerNumber = playerNumber; }

    synchronized void insert(UsbLibrary library) {
        configuredLibrary = Objects.requireNonNull(library);
        current = new Mount(playerNumber, ++generation, Objects.requireNonNull(library));
    }

    synchronized void configure(UsbLibrary library) {
        configuredLibrary = Objects.requireNonNull(library);
        current = new Mount(playerNumber, ++generation, library);
    }

    synchronized void reinsert() {
        if (configuredLibrary == null) throw new IllegalStateException("No USB source configured for Player " + playerNumber);
        current = new Mount(playerNumber, ++generation, configuredLibrary);
    }

    boolean configured() { return configuredLibrary != null; }
    UsbLibrary configuredLibrary() { return configuredLibrary; }

    synchronized void eject() { current = null; }

    Mount current() { return current; }

    Mount requireMounted() {
        Mount mount = current;
        if (mount == null) throw new IllegalStateException("No USB inserted in Player " + playerNumber);
        return mount;
    }

    boolean stillPresent(Mount mount) { return current == mount; }

    record Mount(int playerNumber, long generation, UsbLibrary library) {}
}
