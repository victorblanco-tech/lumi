package co.victorblan.tech.lumi.prolink.simulator;

import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class TrafficFaultControllerTest {
    @Test
    void temporaryDisconnectExpiresWithoutChangingAuthoritativePlayback() {
        AtomicLong clock = new AtomicLong();
        PlayerState first = PlayerStateTest.loadedPlayer(1, clock);
        PlayerState second = PlayerStateTest.loadedPlayer(2, clock);
        first.play();

        try (TrafficFaultController faults = new TrafficFaultController(
                List.of(first, second), clock::get, false
        )) {
            faults.startDisconnect(1, 1_000);
            assertFalse(faults.permit(1, TrafficFaultController.Lane.ANNOUNCEMENT));
            assertFalse(faults.permit(1, TrafficFaultController.Lane.STATUS));

            clock.addAndGet(TimeUnit.MILLISECONDS.toNanos(1_001));

            assertTrue(faults.permit(1, TrafficFaultController.Lane.STATUS));
            assertTrue(first.snapshot().playing());
            assertEquals(1_001, first.snapshot().positionMillis());
            assertTrue(faults.status().activeFaults().isEmpty());
        }
    }

    @Test
    void packetLossIsDeterministicAndLaneScoped() {
        AtomicLong clock = new AtomicLong();
        PlayerState first = PlayerStateTest.loadedPlayer(1, clock);
        PlayerState second = PlayerStateTest.loadedPlayer(2, clock);

        try (TrafficFaultController faults = new TrafficFaultController(
                List.of(first, second), clock::get, false
        )) {
            faults.startPacketLoss(1, TrafficFaultController.Lane.TIMING, 3, 5_000);

            assertTrue(faults.permit(1, TrafficFaultController.Lane.STATUS));
            assertTrue(faults.permit(1, TrafficFaultController.Lane.BEAT));
            assertFalse(faults.permit(1, TrafficFaultController.Lane.PRECISE_POSITION));
            assertTrue(faults.permit(1, TrafficFaultController.Lane.ANNOUNCEMENT));
            assertTrue(faults.permit(2, TrafficFaultController.Lane.PRECISE_POSITION));
            assertEquals(1, faults.status().totalSuppressedPackets());
        }
    }

    @Test
    void recoverySoakCyclesThroughRepeatableFaultAndTransportEvents() {
        AtomicLong clock = new AtomicLong();
        PlayerState first = PlayerStateTest.loadedPlayer(1, clock);
        PlayerState second = PlayerStateTest.loadedPlayer(2, clock);
        first.setMaster(true);
        first.setOnAir(true);
        first.play();

        try (TrafficFaultController faults = new TrafficFaultController(
                List.of(first, second), clock::get, false
        )) {
            faults.runRecoveryEventForTesting();
            assertEquals("position-gap", faults.status().activeFaults().getFirst().kind());

            faults.runRecoveryEventForTesting();
            assertEquals(2, faults.status().activeFaults().size());

            faults.runRecoveryEventForTesting();
            assertEquals("temporary-disconnect", faults.status().activeFaults().getLast().kind());

            faults.runRecoveryEventForTesting();
            assertEquals(2_000, first.snapshot().positionMillis());

            faults.runRecoveryEventForTesting();
            assertFalse(first.snapshot().master());
            assertTrue(second.snapshot().master());
            assertTrue(second.snapshot().playing());
            assertEquals(5, faults.status().recoverySoak().eventCount());
        }
    }

    @Test
    void recoverySoakFailsEarlyWithoutACompleteScenarioAndRestoreStopsIt() {
        AtomicLong clock = new AtomicLong();
        PlayerState first = PlayerStateTest.loadedPlayer(1, clock);
        PlayerState second = new PlayerState(2, clock::get);

        try (TrafficFaultController faults = new TrafficFaultController(
                List.of(first, second), clock::get, false
        )) {
            assertThrows(IllegalStateException.class, () -> faults.setRecoverySoak(true, 8));

            second.load(PlayerStateTest.loadedPlayer(2, clock).snapshot().track());
            first.setMaster(true);
            faults.setRecoverySoak(true, 8);
            faults.startPositionGap(1, 1_000);
            faults.setPlayerOnline(2, false);

            faults.clearFaults();

            assertFalse(faults.status().recoverySoak().enabled());
            assertTrue(faults.status().activeFaults().isEmpty());
            assertTrue(faults.status().manuallyOfflinePlayers().isEmpty());
        }
    }
}
