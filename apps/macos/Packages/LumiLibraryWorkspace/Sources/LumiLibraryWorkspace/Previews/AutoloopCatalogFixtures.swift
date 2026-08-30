public enum AutoloopCatalogFixtures {
    private struct Mapping {
        let roleID: String
        let roleName: String
        let autoloopName: String
    }

    private static let baseBanks: [[Mapping]] = [
        [
            mapping("intro-outro", "Intro / Outro", "INTRO THEME 1"),
            mapping("breakdown-1", "Breakdown 1", "BREAKDOWN THEME 1"),
            mapping("buildup-1", "Buildup 1", "BUILDUP THEME 1"),
            mapping("drop", "Drop", "DROP THEME 1"),
            mapping("synth", "Synth", "SYNTH THEME 1"),
            mapping("breakdown-2", "Breakdown 2", "BREAKDOWN 2 THEME 1"),
            mapping("buildup-3", "Buildup 3", "BUILDUP 3 THEME 1"),
            mapping("pre-drop", "Pre-Drop", "PRE DROP THEME 1")
        ],
        [
            mapping("intro-outro", "Intro / Outro", "INTRO THEME 2"),
            mapping("breakdown-1", "Breakdown 1", "BREAKDOWN THEME 2"),
            mapping("buildup-1", "Buildup 1", "BUILDUP THEME 2"),
            mapping("drop", "Drop", "DROP THEME 2"),
            mapping("synth", "Synth", "SYNTH THEME 2"),
            mapping("bridge", "Bridge", "BRIDGE THEME 2"),
            mapping("pre-drop", "Pre-Drop", "PRE DROP THEME 2"),
            mapping("breakdown-2", "Breakdown 2", "BREAKDOWN 2 THEME 2")
        ],
        [
            mapping("intro-outro", "Intro / Outro", "INTRO THEME 3"),
            mapping("breakdown-1", "Breakdown 1", "BREAKDOWN THEME 3"),
            mapping("buildup-1", "Buildup 1", "BUILDUP THEME 3"),
            mapping("drop", "Drop", "DROP THEME 3"),
            mapping("synth", "Synth", "SYNTH THEME 3"),
            mapping("bridge", "Bridge", "BRIDGE THEME 3"),
            mapping("breakdown-3", "Breakdown 3", "BREAKDOWN 3 THEME 3"),
            mapping("buildup-3", "Buildup 3", "BUILDUP 3 THEME 3")
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

    private static let extendedRoleIDs = [
        "buildup-2", "breakdown-3", "buildup-3", "drop",
        "synth", "bridge", "pre-drop", "intro-outro",
        "breakdown-1", "buildup-1", "breakdown-2", "buildup-2",
        "breakdown-3", "buildup-3", "synth", "drop",
        "bridge", "pre-drop", "intro-outro", "breakdown-1",
        "buildup-1", "drop", "synth", "bridge"
    ]

    private static let roleNames = [
        "intro-outro": "Intro / Outro",
        "bridge": "Bridge",
        "breakdown-1": "Breakdown 1",
        "breakdown-2": "Breakdown 2",
        "breakdown-3": "Breakdown 3",
        "synth": "Synth",
        "pre-drop": "Pre-Drop",
        "buildup-1": "Buildup 1",
        "buildup-2": "Buildup 2",
        "buildup-3": "Buildup 3",
        "drop": "Drop"
    ]

    private static let banks: [[Mapping]] = baseBanks.enumerated().map { bankIndex, base in
        base + extendedRoleIDs.enumerated().map { index, roleID in
            let buttonNumber = index + 9
            return mapping(
                roleID,
                roleNames[roleID] ?? roleID,
                "AUTOLOOP \(buttonNumber) · BANK \(bankIndex + 1)"
            )
        }
    }

    public static let incomplete = AutoloopCatalogState(
        revision: 4,
        defaultsVersion: 3,
        themes: [
            AutoloopThemeState(id: 1, name: "Theme 1", sortOrder: 1),
            AutoloopThemeState(id: 2, name: "Theme 2", sortOrder: 2),
            AutoloopThemeState(id: 3, name: "Theme 3", sortOrder: 3),
            AutoloopThemeState(id: 4, name: "Theme 4", sortOrder: 4)
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
