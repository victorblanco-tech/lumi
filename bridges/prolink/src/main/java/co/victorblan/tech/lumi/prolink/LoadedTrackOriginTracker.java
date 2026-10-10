package co.victorblan.tech.lumi.prolink;

import java.util.Arrays;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/** Separates a previously observed load origin from a cached Player's reported ownership. */
final class LoadedTrackOriginTracker {
    private final Map<Integer, Load> loads = new ConcurrentHashMap<>();

    void forget(int player) {
        loads.remove(player);
    }

    BeatLinkRuntime.ResolvedTrackIdentity observe(
            String model, String firmware, int player, int playState,
            byte[] packet, BeatLinkRuntime.ResolvedTrackIdentity reported, boolean coherent
    ) {
        // A native loading observation starts a new load, including colliding
        // numeric IDs. Do this before the runtime's tempo/coherence filter.
        if (playState == 2) forget(player);
        Load previous = loads.get(player);
        var extended = BeatLinkRuntime.resolveCdj1500xExtendedTrackIdentity(model, player, packet);
        var identity = BeatLinkRuntime.resolveLoadedTrackIdentity(
                reported, extended, previous == null ? null : previous.origin());
        if (identity.rekordboxId() == 0) {
            forget(player);
            return identity;
        }

        byte[] witness = contentWitness(model, firmware, packet, identity.rekordboxId());
        if (playState != 2 && previous != null
                && previous.origin().sourcePlayer() != player
                && "USB_SLOT".equals(previous.origin().sourceSlot())
                && "REKORDBOX".equals(previous.origin().trackType())
                && identity.sourcePlayer() == player
                && "USB_SLOT".equals(identity.sourceSlot())
                && "REKORDBOX".equals(identity.trackType())
                && identity.rekordboxId() == previous.origin().rekordboxId()
                && witness != null && previous.witness() != null
                && Arrays.equals(witness, previous.witness())
                && packet[0x125] == 4) {
            // Hardware captures: 1.10/1152-byte/subtype-8 status retains the
            // exact extended track block after remote USB Stop, but rebases
            // legacy source ownership to this Player and sets 0x125 to 4.
            // Beat Link 8.0 supplies only its first 512 bytes; the declared
            // wire length still identifies this specific supported layout.
            // This is continuity evidence for an existing LINK load ONLY:
            // never a global track ID, content hash or a new matching method.
            identity = previous.origin();
        }
        // The already supported 512-byte form omits the native origin. It
        // may retain this same load, but cannot supply a new content witness.
        // Do not erase a confirmed witness merely because that form intervenes.
        if (witness == null && previous != null && playState != 2
                && identity.equals(previous.origin())
                && "CDJ-1500X".equals(model) && "1.10".equals(firmware)
                && packet.length == 512 && extended.rekordboxId() == identity.rekordboxId()) {
            witness = previous.witness();
        }
        if (coherent) loads.put(player, new Load(identity, witness));
        return identity;
    }

    private static byte[] contentWitness(String model, String firmware, byte[] packet, int id) {
        if (!"CDJ-1500X".equals(model) || !"1.10".equals(firmware)
                || (packet.length != 512 && packet.length != 1152)
                || packet[0x20] != 8) return null;
        int declaredLength = ((packet[0x22] & 0xff) << 8 | (packet[0x23] & 0xff)) + 0x24;
        if (declaredLength != 1152) return null;
        int extendedId = ((packet[0x194] & 0xff) << 24)
                | ((packet[0x195] & 0xff) << 16)
                | ((packet[0x196] & 0xff) << 8) | (packet[0x197] & 0xff);
        if (id == 0 || extendedId != id) return null;
        // Fixed bounded block, observed unchanged in both real captures.
        // Unknown fields are compared as opaque bytes, not interpreted as IDs.
        return Arrays.copyOfRange(packet, 0x170, 0x1b0);
    }

    private record Load(BeatLinkRuntime.ResolvedTrackIdentity origin, byte[] witness) {}
}
