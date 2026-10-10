import Foundation

/// Restores an existing user grant. This never opens a permission dialog,
/// changes macOS settings, or treats a bookmark as USB identity evidence.
public enum USBBookmarkResolver {
    public struct Resolution {
        public let url: URL
        public let renewedBookmark: Data?
    }

    public static func resolve(_ bookmark: Data, expectedRoot: URL) throws -> Resolution? {
        var stale = false
        let resolved = try URL(
            resolvingBookmarkData: bookmark,
            options: [.withSecurityScope, .withoutUI],
            relativeTo: nil,
            bookmarkDataIsStale: &stale
        )
        return try validateAndRenew(resolved: resolved, expectedRoot: expectedRoot, stale: stale) {
            guard resolved.startAccessingSecurityScopedResource() else { return nil }
            defer { resolved.stopAccessingSecurityScopedResource() }
            return try resolved.bookmarkData(
                options: .withSecurityScope,
                includingResourceValuesForKeys: nil,
                relativeTo: nil
            )
        }
    }

    static func validateAndRenew(
        resolved: URL,
        expectedRoot: URL,
        stale: Bool,
        renew: () throws -> Data?
    ) rethrows -> Resolution? {
        // Never substitute a same-model stick, relocated path, or a broader
        // parent directory. Physical identity remains independently verified.
        guard resolved.standardizedFileURL.path == expectedRoot.standardizedFileURL.path else {
            return nil
        }
        guard stale else { return Resolution(url: resolved, renewedBookmark: nil) }
        guard let renewed = try renew() else { return nil }
        return Resolution(url: resolved, renewedBookmark: renewed)
    }
}
