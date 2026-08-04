public struct SoundSwitchOutputProfileState: Equatable, Sendable {
    public static let builtIn = Self(
        id: "soundswitch-autoloops",
        name: "SoundSwitch Autoloops",
        targetName: "SoundSwitch",
        controllerName: "Lumi Virtual MIDI Controller",
        bankCount: 4,
        slotsPerBank: 32,
        slotsPerPage: 8
    )

    public let id: String
    public let name: String
    public let targetName: String
    public let controllerName: String
    public let bankCount: UInt16
    public let slotsPerBank: UInt16
    public let slotsPerPage: UInt16

    public init(
        id: String,
        name: String,
        targetName: String,
        controllerName: String,
        bankCount: UInt16,
        slotsPerBank: UInt16,
        slotsPerPage: UInt16
    ) {
        self.id = id
        self.name = name
        self.targetName = targetName
        self.controllerName = controllerName
        self.bankCount = bankCount
        self.slotsPerBank = slotsPerBank
        self.slotsPerPage = slotsPerPage
    }

    public var pageCount: UInt16 {
        slotsPerBank / slotsPerPage
    }
}

public enum SoundSwitchBankOrganization: String, CaseIterable, Equatable, Sendable {
    case theme
    case genre
    case function
    case custom

    public var displayName: String {
        rawValue.capitalized
    }
}

public struct SoundSwitchOutputBankState: Identifiable, Equatable, Sendable {
    public let id: UInt64
    public let number: UInt16
    public let name: String
    public let organization: SoundSwitchBankOrganization
    public let groupID: String
    public let groupName: String

    public init(
        id: UInt64,
        number: UInt16,
        name: String,
        organization: SoundSwitchBankOrganization,
        groupID: String,
        groupName: String
    ) {
        self.id = id
        self.number = number
        self.name = name
        self.organization = organization
        self.groupID = groupID
        self.groupName = groupName
    }
}

public enum SoundSwitchAutoloopSlotStatus: String, Equatable, Sendable {
    case mapped
    case incomplete
    case available
}

public struct SoundSwitchAutoloopSlotState: Identifiable, Equatable, Sendable {
    public var id: UInt16 { number }

    public let number: UInt16
    public let roleID: String?
    public let roleName: String?
    public let variantID: String?
    public let variantName: String?
    public let entryID: String?
    public let entryName: String?
    public let status: SoundSwitchAutoloopSlotStatus

    public init(
        number: UInt16,
        roleID: String?,
        roleName: String?,
        variantID: String?,
        variantName: String?,
        entryID: String?,
        entryName: String?,
        status: SoundSwitchAutoloopSlotStatus
    ) {
        self.number = number
        self.roleID = roleID
        self.roleName = roleName
        self.variantID = variantID
        self.variantName = variantName
        self.entryID = entryID
        self.entryName = entryName
        self.status = status
    }
}

public enum SoundSwitchOutputProfileProjection {
    public static func banks(catalog: AutoloopCatalogState) -> [SoundSwitchOutputBankState] {
        catalog.themes.map { theme in
            SoundSwitchOutputBankState(
                id: theme.id,
                number: theme.sortOrder,
                name: theme.name,
                organization: .theme,
                groupID: "theme-\(theme.id)",
                groupName: theme.name
            )
        }
    }

    public static func slots(
        for bankID: UInt64,
        catalog: AutoloopCatalogState,
        profile: SoundSwitchOutputProfileState = .builtIn
    ) -> [SoundSwitchAutoloopSlotState] {
        return (1...profile.slotsPerBank).map { slotNumber in
            let mapping = catalog.roles.lazy.compactMap { role in
                role.variants.lazy.compactMap { variant in
                    variant.cells.first {
                        $0.themeID == bankID && $0.buttonNumber == slotNumber && !$0.isMissing
                    }.map { (role, variant, $0) }
                }.first
            }.first
            guard let (role, variant, cell) = mapping else {
                return SoundSwitchAutoloopSlotState(
                    number: slotNumber,
                    roleID: nil,
                    roleName: nil,
                    variantID: nil,
                    variantName: nil,
                    entryID: nil,
                    entryName: nil,
                    status: .available
                )
            }
            return SoundSwitchAutoloopSlotState(
                number: slotNumber,
                roleID: role.id,
                roleName: role.name,
                variantID: variant.id,
                variantName: variant.name,
                entryID: cell.entryID,
                entryName: cell.name,
                status: .mapped
            )
        }
    }

    public static func mappedCount(
        for bankID: UInt64,
        catalog: AutoloopCatalogState,
        profile: SoundSwitchOutputProfileState = .builtIn
    ) -> Int {
        slots(for: bankID, catalog: catalog, profile: profile)
            .filter { $0.status == .mapped }
            .count
    }
}
