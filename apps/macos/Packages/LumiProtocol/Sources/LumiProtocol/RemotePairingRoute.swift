/// App routing only. Invitation validation, TLS pinning and approval remain separate.
public enum RemotePairingRoute {
    public static func scheme(for releaseChannel: String) -> String? {
        switch releaseChannel {
        // Keep existing Production QR codes usable. Only Production owns this scheme.
        case "production", "release": "lumi"
        case "dev": "lumi-dev"
        case "rc": "lumi-rc"
        default: nil
        }
    }
}
