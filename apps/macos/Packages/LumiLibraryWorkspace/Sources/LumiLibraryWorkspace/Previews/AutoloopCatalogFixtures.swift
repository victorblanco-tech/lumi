public enum AutoloopCatalogFixtures {
    public static let incomplete = AutoloopCatalogState(
        revision: 3,
        defaultsVersion: 1,
        themes: [
            AutoloopThemeState(id: 1, name: "Electric Bloom", sortOrder: 1),
            AutoloopThemeState(id: 2, name: "Deep Ocean", sortOrder: 2),
            AutoloopThemeState(id: 3, name: "Solar Flare", sortOrder: 3),
            AutoloopThemeState(id: 4, name: "Ultraviolet", sortOrder: 4)
        ],
        roles: [
            role("intro-outro", "Intro / Outro", variants: 1),
            role("bridge", "Bridge", variants: 1),
            role("breakdown-1", "Breakdown 1", variants: 2),
            role("synth", "Synth", variants: 3, missing: [(3, 3), (4, 3)]),
            role("buildup-1", "Buildup 1", variants: 2),
            role("drop", "Drop", variants: 2, missing: [(4, 2)])
        ],
        preflight: AutoloopPreflightState(
            status: "incomplete",
            missingCellCount: 3,
            missingCells: [
                MissingAutoloopCellState(themeID: 3, roleID: "synth", variantID: "variant-3"),
                MissingAutoloopCellState(themeID: 4, roleID: "synth", variantID: "variant-3"),
                MissingAutoloopCellState(themeID: 4, roleID: "drop", variantID: "variant-2")
            ],
            hasMoreMissingCells: false,
            missingRoleCount: 0,
            missingRoleIDs: [],
            hasMoreMissingRoles: false
        ),
        targetValidationOwner: "targetAdapter",
        hardCodedPhysicalCapacity: false
    )

    private static func role(
        _ id: String,
        _ name: String,
        variants: Int,
        missing: [(Int, Int)] = []
    ) -> AutoloopRoleMatrixState {
        AutoloopRoleMatrixState(
            id: id,
            name: name,
            archived: false,
            variants: (1...variants).map { variant in
                AutoloopVariantState(
                    id: "variant-\(variant)",
                    name: "Variant \(variant)",
                    sortOrder: UInt16(variant),
                    archived: false,
                    cells: (1...4).map { theme in
                        let isMissing = missing.contains { $0 == (theme, variant) }
                        return AutoloopCellState(
                            themeID: UInt64(theme),
                            entryID: isMissing ? nil : "theme-\(theme)--\(id)--variant-\(variant)",
                            name: isMissing ? nil : "\(themeName(theme)) · \(name) · Variant \(variant)",
                            status: isMissing ? "missing" : "ready"
                        )
                    }
                )
            }
        )
    }

    private static func themeName(_ theme: Int) -> String {
        switch theme {
        case 1: "Electric Bloom"
        case 2: "Deep Ocean"
        case 3: "Solar Flare"
        default: "Ultraviolet"
        }
    }
}
