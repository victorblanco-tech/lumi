package co.victorblan.tech.lumi.prolink.simulator;

import org.junit.jupiter.api.Test;
import java.net.InetAddress;
import java.net.Inet4Address;
import static org.junit.jupiter.api.Assertions.*;

class NetworkSelectionTest {
    private ProLinkBroadcaster.Endpoint endpoint(String name, String ip, String broadcast) throws Exception {
        return new ProLinkBroadcaster.Endpoint(name, (Inet4Address)InetAddress.getByName(ip),
                (Inet4Address)InetAddress.getByName(broadcast),new byte[6]);
    }
    @Test void secondaryRequiresDistinctInterfaceAddressAndSameLan() throws Exception {
        var first=endpoint("wired", "192.168.42.10", "192.168.42.255");
        assertTrue(ProLinkBroadcaster.suitableSecondary(first,endpoint("wireless","192.168.42.11","192.168.42.255")));
        assertFalse(ProLinkBroadcaster.suitableSecondary(first,first));
        assertFalse(ProLinkBroadcaster.suitableSecondary(first,endpoint("wireless","192.168.43.11","192.168.43.255")));
        assertFalse(ProLinkBroadcaster.suitableSecondary(first,endpoint("wireless","192.168.42.10","192.168.42.255")));
    }
}
