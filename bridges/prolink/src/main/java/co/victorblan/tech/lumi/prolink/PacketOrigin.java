package co.victorblan.tech.lumi.prolink;

import org.deepsymmetry.beatlink.DeviceUpdate;

/** Receive-side evidence, before Lumi publication; not a sender timestamp. */
record PacketOrigin(String address, long receivedAtNanos) {
    static PacketOrigin from(DeviceUpdate update) {
        return new PacketOrigin(update.getAddress().getHostAddress(), update.getTimestamp());
    }
}
