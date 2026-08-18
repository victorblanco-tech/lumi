package co.victorblan.tech.lumi.prolink.simulator;

import java.util.Locale;

/**
 * Deterministic Pro DJ Link traffic profiles used by the development simulator.
 *
 * <p>The CDJ-1500X profile deliberately includes the high-frequency precise
 * position lane and short stale-position bursts observed on physical players.
 * It is the default acceptance profile; classic exists only as a low-traffic
 * comparison when diagnosing the simulator itself.</p>
 */
enum ProLinkTrafficProfile {
    CDJ_1500X("cdj-1500x", 100, 20, 5_000, 8, 5),
    CLASSIC("classic", 100, 0, 0, 0, 0);

    private final String externalName;
    private final long statusIntervalMillis;
    private final long precisePositionIntervalMillis;
    private final long burstIntervalMillis;
    private final int burstPacketCount;
    private final int burstRewindBeats;

    ProLinkTrafficProfile(
            String externalName,
            long statusIntervalMillis,
            long precisePositionIntervalMillis,
            long burstIntervalMillis,
            int burstPacketCount,
            int burstRewindBeats
    ) {
        this.externalName = externalName;
        this.statusIntervalMillis = statusIntervalMillis;
        this.precisePositionIntervalMillis = precisePositionIntervalMillis;
        this.burstIntervalMillis = burstIntervalMillis;
        this.burstPacketCount = burstPacketCount;
        this.burstRewindBeats = burstRewindBeats;
    }

    String externalName() {
        return externalName;
    }

    long statusIntervalMillis() {
        return statusIntervalMillis;
    }

    long precisePositionIntervalMillis() {
        return precisePositionIntervalMillis;
    }

    long burstIntervalMillis() {
        return burstIntervalMillis;
    }

    int burstPacketCount() {
        return burstPacketCount;
    }

    int burstRewindBeats() {
        return burstRewindBeats;
    }

    boolean publishesPrecisePosition() {
        return precisePositionIntervalMillis > 0;
    }

    boolean publishesBursts() {
        return burstIntervalMillis > 0 && burstPacketCount > 0;
    }

    static ProLinkTrafficProfile parse(String value) {
        String normalized = value.trim().toLowerCase(Locale.ROOT);
        for (ProLinkTrafficProfile profile : values()) {
            if (profile.externalName.equals(normalized)) {
                return profile;
            }
        }
        throw new IllegalArgumentException(
                "Unknown traffic profile: " + value + ". Expected cdj-1500x or classic"
        );
    }
}
