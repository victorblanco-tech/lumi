import Foundation
import Security

public actor KeychainRemoteCredentialStore: RemoteCredentialStore {
    private let service: String
    private let releaseChannel: RemoteReleaseChannel
    private let encoder = JSONEncoder()
    private let decoder = JSONDecoder()

    public init(service: String, releaseChannel: RemoteReleaseChannel) {
        self.service = service
        self.releaseChannel = releaseChannel
    }

    public func credential(for installationID: String) throws -> RemoteDeviceCredential? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account(for: installationID),
            kSecMatchLimit as String: kSecMatchLimitOne,
            kSecReturnData as String: true
        ]
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = result as? Data else {
            throw RemoteCredentialStoreError.keychain(status)
        }
        guard data.count <= 4_096 else {
            throw RemoteCredentialStoreError.invalidStoredCredential
        }
        do {
            let credential = try decoder.decode(RemoteDeviceCredential.self, from: data)
            try credential.validate(
                for: installationID,
                expectedChannel: releaseChannel
            )
            return credential
        } catch let error as RemoteTrustError {
            throw error
        } catch {
            throw RemoteCredentialStoreError.invalidStoredCredential
        }
    }

    public func save(_ credential: RemoteDeviceCredential) throws {
        try credential.validate(
            for: credential.installationID,
            expectedChannel: releaseChannel
        )
        let data = try encoder.encode(credential)
        guard data.count <= 4_096 else {
            throw RemoteCredentialStoreError.invalidStoredCredential
        }
        let base: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account(for: credential.installationID)
        ]
        let updateStatus = SecItemUpdate(
            base as CFDictionary,
            [kSecValueData as String: data] as CFDictionary
        )
        if updateStatus == errSecSuccess { return }
        guard updateStatus == errSecItemNotFound else {
            throw RemoteCredentialStoreError.keychain(updateStatus)
        }
        var insertion = base
        insertion[kSecValueData as String] = data
        insertion[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        let addStatus = SecItemAdd(insertion as CFDictionary, nil)
        guard addStatus == errSecSuccess else {
            throw RemoteCredentialStoreError.keychain(addStatus)
        }
    }

    public func remove(installationID: String) throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account(for: installationID)
        ]
        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw RemoteCredentialStoreError.keychain(status)
        }
    }

    private func account(for installationID: String) -> String {
        "\(releaseChannel.rawValue):\(installationID)"
    }
}

public enum RemoteCredentialStoreError: Error, Equatable {
    case keychain(OSStatus)
    case invalidStoredCredential
}
