public enum PhraseRoleSettingsFixtures {
    public static func ready() -> PhraseRoleSettingsState {
        let definitions: [(String, String, UInt64, UInt64)] = [
            ("intro-outro", "Intro / Outro", 3, 3),
            ("bridge", "Bridge", 1, 1),
            ("breakdown-1", "Breakdown 1", 2, 2),
            ("breakdown-2", "Breakdown 2", 1, 1),
            ("breakdown-3", "Breakdown 3", 0, 0),
            ("synth", "Synth", 1, 1),
            ("pre-drop", "Pre-drop", 0, 0),
            ("buildup-1", "Buildup 1", 2, 2),
            ("buildup-2", "Buildup 2", 1, 1),
            ("buildup-3", "Buildup 3", 1, 1),
            ("drop", "Drop", 3, 3)
        ]
        let roles = definitions.enumerated().map { index, definition in
            let (id, name, trackCount, phraseCount) = definition
            return PhraseRoleDefinition(
                id: id,
                name: name,
                sortOrder: UInt16(index + 1),
                archived: false,
                usage: PhraseRoleUsage(
                    phraseCount: phraseCount,
                    trackCount: trackCount,
                    catalogRowCount: 0,
                    affectedTracks: trackCount == 0 ? [] : [
                        PhraseRoleAffectedTrack(
                            trackID: UInt64(index + 1),
                            title: index == 5 ? "Northern Pulse" : "Afterglow Drive",
                            phraseCount: phraseCount
                        )
                    ],
                    hasMoreAffectedTracks: false
                )
            )
        }
        let mappings = [
            SourcePhraseMapping(rawLabel: "Intro", roleID: "intro-outro"),
            SourcePhraseMapping(rawLabel: "Outro", roleID: "intro-outro"),
            SourcePhraseMapping(rawLabel: "Verse", roleID: "bridge"),
            SourcePhraseMapping(rawLabel: "Bridge", roleID: "bridge"),
            SourcePhraseMapping(rawLabel: "Down", roleID: "breakdown-1"),
            SourcePhraseMapping(rawLabel: "Up", roleID: "buildup-1"),
            SourcePhraseMapping(rawLabel: "Chorus", roleID: "drop"),
            SourcePhraseMapping(rawLabel: "*", roleID: "bridge")
        ]
        return PhraseRoleSettingsState(
            revision: 4,
            defaultsVersion: 1,
            roles: roles,
            mappingProfiles: [
                SourcePhraseMappingProfile(
                    providerKind: "demo",
                    providerName: "Demo Library",
                    mappings: mappings
                ),
                SourcePhraseMappingProfile(
                    providerKind: "rekordbox7",
                    providerName: "Rekordbox 7",
                    mappings: mappings
                )
            ],
            mappingPolicy: "futureInitialTimelinesOnly"
        )
    }
}
