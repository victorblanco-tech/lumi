public struct PhraseRoleAffectedTrack: Identifiable, Equatable, Sendable {
    public var id: UInt64 { trackID }

    public let trackID: UInt64
    public let title: String
    public let phraseCount: UInt64

    public init(trackID: UInt64, title: String, phraseCount: UInt64) {
        self.trackID = trackID
        self.title = title
        self.phraseCount = phraseCount
    }
}

public struct PhraseRoleUsage: Equatable, Sendable {
    public let phraseCount: UInt64
    public let trackCount: UInt64
    public let catalogRowCount: UInt64
    public let affectedTracks: [PhraseRoleAffectedTrack]
    public let hasMoreAffectedTracks: Bool

    public init(
        phraseCount: UInt64,
        trackCount: UInt64,
        catalogRowCount: UInt64,
        affectedTracks: [PhraseRoleAffectedTrack],
        hasMoreAffectedTracks: Bool
    ) {
        self.phraseCount = phraseCount
        self.trackCount = trackCount
        self.catalogRowCount = catalogRowCount
        self.affectedTracks = affectedTracks
        self.hasMoreAffectedTracks = hasMoreAffectedTracks
    }
}

public struct PhraseRoleDefinition: Identifiable, Equatable, Sendable {
    public let id: String
    public let name: String
    public let sortOrder: UInt16
    public let archived: Bool
    public let usage: PhraseRoleUsage

    public init(
        id: String,
        name: String,
        sortOrder: UInt16,
        archived: Bool,
        usage: PhraseRoleUsage
    ) {
        self.id = id
        self.name = name
        self.sortOrder = sortOrder
        self.archived = archived
        self.usage = usage
    }
}

public struct SourcePhraseMapping: Identifiable, Equatable, Sendable {
    public var id: String { rawLabel.lowercased() }

    public let rawLabel: String
    public let roleID: String

    public init(rawLabel: String, roleID: String) {
        self.rawLabel = rawLabel
        self.roleID = roleID
    }
}

public struct SourcePhraseMappingProfile: Identifiable, Equatable, Sendable {
    public var id: String { providerKind }

    public let providerKind: String
    public let providerName: String
    public let mappings: [SourcePhraseMapping]

    public init(providerKind: String, providerName: String, mappings: [SourcePhraseMapping]) {
        self.providerKind = providerKind
        self.providerName = providerName
        self.mappings = mappings
    }
}

public struct PhraseRoleSettingsState: Equatable, Sendable {
    public let revision: UInt64
    public let defaultsVersion: UInt16
    public let roles: [PhraseRoleDefinition]
    public let mappingProfiles: [SourcePhraseMappingProfile]
    public let mappingPolicy: String

    public init(
        revision: UInt64,
        defaultsVersion: UInt16,
        roles: [PhraseRoleDefinition],
        mappingProfiles: [SourcePhraseMappingProfile],
        mappingPolicy: String
    ) {
        self.revision = revision
        self.defaultsVersion = defaultsVersion
        self.roles = roles
        self.mappingProfiles = mappingProfiles
        self.mappingPolicy = mappingPolicy
    }

    public var activeRoles: [PhraseRoleDefinition] {
        roles.filter { !$0.archived }
    }
}

public enum PhraseRoleMutationRequest: Equatable, Sendable {
    case add(displayName: String)
    case rename(roleID: String, displayName: String)
    case moveEarlier(roleID: String)
    case moveLater(roleID: String)
    case archive(roleID: String)
    case restore(roleID: String)
    case setSourceMapping(providerKind: String, rawLabel: String, roleID: String)
}
