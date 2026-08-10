package co.victorblan.tech.lumi.prolink.simulator;

import java.net.InetAddress;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.ConcurrentHashMap;

final class ProLinkPeerRegistry {
    private final long leaseNanos;
    private final ConcurrentHashMap<InetAddress, Long> observedAt = new ConcurrentHashMap<>();

    ProLinkPeerRegistry(long leaseNanos) {
        if (leaseNanos <= 0) {
            throw new IllegalArgumentException("Peer lease must be positive");
        }
        this.leaseNanos = leaseNanos;
    }

    void observe(InetAddress address, long now) {
        observedAt.put(Objects.requireNonNull(address, "address"), now);
    }

    List<InetAddress> active(long now) {
        observedAt.entrySet().removeIf(peer -> now - peer.getValue() > leaseNanos);
        return List.copyOf(observedAt.keySet());
    }

    int size(long now) {
        return active(now).size();
    }
}
