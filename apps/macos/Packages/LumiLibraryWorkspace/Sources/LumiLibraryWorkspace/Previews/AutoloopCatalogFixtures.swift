public enum AutoloopCatalogFixtures {
    private struct Mapping {
        let roleID: String
        let roleName: String
        let autoloopName: String
    }

    private static let banks: [[Mapping]] = [
        [
            mapping("intro-outro", "Intro / Outro", "INTRO BLUE PINK"),
            mapping("breakdown-1", "Breakdown 1", "BREAKDOWN BLUE PINK"),
            mapping("buildup-1", "Buildup 1", "BUILDUP BLUE PINK"),
            mapping("drop", "Drop", "DROP BLUE PINK - NEW 1"),
            mapping("synth", "Synth", "SYNTH BLUE PINK"),
            mapping("breakdown-2", "Breakdown 2", "BREAKDOWN 2 BLUE PINK"),
            mapping("buildup-3", "Buildup 3", "BUILDUP 3 BLUE PINK"),
            mapping("pre-drop", "Pre-Drop", "PRE DROP BLUE PINK")
        ],
        [
            mapping("intro-outro", "Intro / Outro", "INTRO GREEN PINK"),
            mapping("breakdown-1", "Breakdown 1", "BREAKDOWN GREEN PINK"),
            mapping("buildup-1", "Buildup 1", "BUILDUP GREEN PINK"),
            mapping("drop", "Drop", "DROP GREEN PINK"),
            mapping("synth", "Synth", "SYNTH GREEN PINK"),
            mapping("bridge", "Bridge", "BRIDGE GREEN PINK"),
            mapping("pre-drop", "Pre-Drop", "PRE DROP GREEN PINK"),
            mapping("breakdown-2", "Breakdown 2", "BREAKDOWN 2 GREEN PINK")
        ],
        [
            mapping("intro-outro", "Intro / Outro", "INTRO BLUE RED GREEN"),
            mapping("breakdown-1", "Breakdown 1", "BREAKDOWN BLUE RED GREEN"),
            mapping("buildup-1", "Buildup 1", "BUILDUP BLUE RED GREEN"),
            mapping("drop", "Drop", "DROP BLUE RED GREEN"),
            mapping("synth", "Synth", "SYNTH BLUE RED GREEN"),
            mapping("bridge", "Bridge", "BRIDGE BLUE RED GREEN"),
            mapping("breakdown-3", "Breakdown 3", "BREAKDOWN 3 BLUE RED GREEN"),
            mapping("buildup-3", "Buildup 3", "BUILDUP 3 BLUE RED GREEN")
        ],
        [
            mapping("intro-outro", "Intro / Outro", "INTRO THEME 4"),
            mapping("breakdown-1", "Breakdown 1", "BREAKDOWN THEME 4"),
            mapping("buildup-1", "Buildup 1", "BUILDUP THEME 4"),
            mapping("drop", "Drop", "DROP THEME 4"),
            mapping("synth", "Synth", "SYNTH THEME 4"),
            mapping("bridge", "Bridge", "BRIDGE THEME 4"),
            mapping("buildup-2", "Buildup 2", "BUILDUP 2 THEME 4"),
            mapping("pre-drop", "Pre-Drop", "PRE DROP THEME 4")
        ]
    ]

    public static let incomplete = AutoloopCatalogState(
        revision: 4,
        defaultsVersion: 2,
        themes: [
            AutoloopThemeState(id: 1, name: "Blue Pink", sortOrder: 1),
            AutoloopThemeState(id: 2, name: "Green Pink", sortOrder: 2),
            AutoloopThemeState(id: 3, name: "Blue Red Green", sortOrder: 3),
            AutoloopThemeState(id: 4, name: "To Do", sortOrder: 4)
        ],
        roles: roleStates(),
        preflight: AutoloopPreflightState(
            status: "ready",
            missingCellCount: 0,
            missingCells: [],
            hasMoreMissingCells: false,
            missingRoleCount: 0,
            missingRoleIDs: [],
            hasMoreMissingRoles: false
        ),
        targetValidationOwner: "targetAdapter",
        hardCodedPhysicalCapacity: false
    )

    private static func roleStates() -> [AutoloopRoleMatrixState] {
        let identities = banks.flatMap { $0 }.reduce(into: [String: String]()) {
            $0[$1.roleID] = $1.roleName
        }
        return identities.keys.sorted().map { roleID in
            let buttonNumbers = Set(
                banks.flatMap { bank in
                    bank.enumerated().compactMap { index, mapping in
                        mapping.roleID == roleID ? UInt16(index + 1) : nil
                    }
                }
            ).sorted()
            return AutoloopRoleMatrixState(
                id: roleID,
                name: identities[roleID] ?? roleID,
                archived: false,
                variants: buttonNumbers.enumerated().map { index, buttonNumber in
                    AutoloopVariantState(
                        id: "mapping-\(buttonNumber)",
                        name: "Output mapping \(buttonNumber)",
                        sortOrder: UInt16(index + 1),
                        archived: false,
                        cells: (1...4).map { bankNumber in
                            let mapping = banks[bankNumber - 1][Int(buttonNumber - 1)]
                            let matches = mapping.roleID == roleID
                            return AutoloopCellState(
                                themeID: UInt64(bankNumber),
                                buttonNumber: matches ? buttonNumber : nil,
                                entryID: matches ? "theme-\(bankNumber)--mapping-\(buttonNumber)" : nil,
                                name: matches ? mapping.autoloopName : nil,
                                status: matches ? "ready" : "missing"
                            )
                        }
                    )
                }
            )
        }
    }

    private static func mapping(_ id: String, _ name: String, _ autoloop: String) -> Mapping {
        Mapping(roleID: id, roleName: name, autoloopName: autoloop)
    }
}
