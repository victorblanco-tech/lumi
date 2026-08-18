package co.victorblan.tech.lumi.prolink.simulator;

import org.junit.jupiter.api.Test;

import java.net.InetAddress;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

class ProLinkPeerRegistryTest {
    @Test
    void activePeersAreRefreshedAndExpiredByTheirAnnouncementLease() throws Exception {
        ProLinkPeerRegistry registry = new ProLinkPeerRegistry(100);
        InetAddress first = InetAddress.getByName("192.168.10.20");
        InetAddress second = InetAddress.getByName("192.168.10.21");

        registry.observe(first, 10);
        registry.observe(second, 50);
        registry.observe(first, 80);

        assertEquals(Set.of(first, second), Set.copyOf(registry.active(120)));
        assertEquals(Set.of(first), Set.copyOf(registry.active(151)));
        assertEquals(0, registry.size(181));
    }

    @Test
    void peerLeaseMustBePositive() {
        assertThrows(IllegalArgumentException.class, () -> new ProLinkPeerRegistry(0));
    }
}
