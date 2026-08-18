package co.victorblan.tech.lumi.prolink.simulator;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ProLinkTrafficProfileTest {
    @Test
    void cdj1500xProfileCarriesSustainedAndBurstPositionTraffic() {
        ProLinkTrafficProfile profile = ProLinkTrafficProfile.CDJ_1500X;

        assertEquals(100, profile.statusIntervalMillis());
        assertEquals(20, profile.precisePositionIntervalMillis());
        assertTrue(profile.publishesPrecisePosition());
        assertTrue(profile.publishesBursts());
        assertTrue(profile.burstPacketCount() >= 8);
        assertTrue(profile.burstRewindBeats() >= 5);
    }

    @Test
    void classicProfileRemainsAnExplicitLowTrafficControl() {
        ProLinkTrafficProfile profile = ProLinkTrafficProfile.CLASSIC;

        assertFalse(profile.publishesPrecisePosition());
        assertFalse(profile.publishesBursts());
    }

    @Test
    void externalNamesAreStableAndStrict() {
        assertEquals(ProLinkTrafficProfile.CDJ_1500X, ProLinkTrafficProfile.parse("CDJ-1500X"));
        assertThrows(IllegalArgumentException.class, () -> ProLinkTrafficProfile.parse("modern"));
    }
}
