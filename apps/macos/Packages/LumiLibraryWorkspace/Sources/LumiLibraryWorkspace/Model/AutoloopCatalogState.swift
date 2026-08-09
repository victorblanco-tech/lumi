public struct AutoloopThemeState: Identifiable, Equatable, Sendable {
    public let id: UInt64
    public let name: String
    public let sortOrder: UInt16

    public init(id: UInt64, name: String, sortOrder: UInt16) {
        self.id = id
        self.name = name
        self.sortOrder = sortOrder
    }
}

public struct AutoloopCellState: Identifiable, Equatable, Sendable {
    public var id: UInt64 { themeID }

    public let themeID: UInt64
    public let buttonNumber: UInt16?
    public let entryID: String?
    public let name: String?
    public let status: String

    public init(
        themeID: UInt64,
        buttonNumber: UInt16? = nil,
        entryID: String?,
        name: String?,
        status: String
    ) {
        self.themeID = themeID
        self.buttonNumber = buttonNumber
        self.entryID = entryID
        self.name = name
        self.status = status
    }

    public var isMissing: Bool { status == "missing" }
}

public struct AutoloopVariantState: Identifiable, Equatable, Sendable {
    public let id: String
    public let name: String
    public let sortOrder: UInt16
    public let archived: Bool
    public let cells: [AutoloopCellState]

    public init(
        id: String,
        name: String,
        sortOrder: UInt16,
        archived: Bool,
        cells: [AutoloopCellState]
    ) {
        self.id = id
        self.name = name
        self.sortOrder = sortOrder
        self.archived = archived
        self.cells = cells
    }
}

public struct AutoloopRoleMatrixState: Identifiable, Equatable, Sendable {
    public let id: String
    public let name: String
    public let archived: Bool
    public let variants: [AutoloopVariantState]

    public init(id: String, name: String, archived: Bool, variants: [AutoloopVariantState]) {
        self.id = id
        self.name = name
        self.archived = archived
        self.variants = variants
    }
}

public struct MissingAutoloopCellState: Identifiable, Equatable, Sendable {
    public var id: String { "\(themeID)-\(roleID)-\(variantID)" }

    public let themeID: UInt64
    public let roleID: String
    public let variantID: String

    public init(themeID: UInt64, roleID: String, variantID: String) {
        self.themeID = themeID
        self.roleID = roleID
        self.variantID = variantID
    }
}

public struct AutoloopPreflightState: Equatable, Sendable {
    public let status: String
    public let missingCellCount: UInt64
    public let missingCells: [MissingAutoloopCellState]
    public let hasMoreMissingCells: Bool
    public let missingRoleCount: UInt64
    public let missingRoleIDs: [String]
    public let hasMoreMissingRoles: Bool

    public init(
        status: String,
        missingCellCount: UInt64,
        missingCells: [MissingAutoloopCellState],
        hasMoreMissingCells: Bool,
        missingRoleCount: UInt64,
        missingRoleIDs: [String],
        hasMoreMissingRoles: Bool
    ) {
        self.status = status
        self.missingCellCount = missingCellCount
        self.missingCells = missingCells
        self.hasMoreMissingCells = hasMoreMissingCells
        self.missingRoleCount = missingRoleCount
        self.missingRoleIDs = missingRoleIDs
        self.hasMoreMissingRoles = hasMoreMissingRoles
    }
}

public struct AutoloopCatalogState: Equatable, Sendable {
    public let revision: UInt64
    public let defaultsVersion: UInt16
    public let themes: [AutoloopThemeState]
    public let roles: [AutoloopRoleMatrixState]
    public let preflight: AutoloopPreflightState
    public let targetValidationOwner: String
    public let hardCodedPhysicalCapacity: Bool

    public init(
        revision: UInt64,
        defaultsVersion: UInt16,
        themes: [AutoloopThemeState],
        roles: [AutoloopRoleMatrixState],
        preflight: AutoloopPreflightState,
        targetValidationOwner: String,
        hardCodedPhysicalCapacity: Bool
    ) {
        self.revision = revision
        self.defaultsVersion = defaultsVersion
        self.themes = themes
        self.roles = roles
        self.preflight = preflight
        self.targetValidationOwner = targetValidationOwner
        self.hardCodedPhysicalCapacity = hardCodedPhysicalCapacity
    }
}

public enum AutoloopCatalogMutationRequest: Equatable, Sendable {
    case renameTheme(themeID: UInt64, displayName: String)
    case addVariant(roleID: String, displayName: String)
    case renameVariant(roleID: String, variantID: String, displayName: String)
    case moveVariantEarlier(roleID: String, variantID: String)
    case moveVariantLater(roleID: String, variantID: String)
    case archiveVariant(roleID: String, variantID: String)
    case restoreVariant(roleID: String, variantID: String)
    case setCell(themeID: UInt64, roleID: String, variantID: String, displayName: String?)
    case setButton(themeID: UInt64, buttonNumber: UInt16, roleID: String, displayName: String?)
    case clearButton(themeID: UInt64, buttonNumber: UInt16)
}
