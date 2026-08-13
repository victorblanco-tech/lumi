import LumiDesignSystem

public enum TrackEditorFixtures {
    public static let ready: TrackEditorAnalysis = {
        let beatsPerBar: UInt8 = 4
        let barCount: UInt32 = 16
        let beatDuration: UInt64 = 500
        let beats = (0..<(barCount * UInt32(beatsPerBar))).map { index in
            TrackEditorBeat(
                beatIndex: index,
                timeMillis: UInt64(index) * beatDuration,
                barIndex: index / UInt32(beatsPerBar) + 1,
                beatInBar: UInt8(index % UInt32(beatsPerBar) + 1)
            )
        }
        let waveform: [TrackEditorWaveformPoint] = (0..<128).map { index in
            let low = UInt8(40 + (index * 37) % 180)
            let mid = UInt8(30 + (index * 53) % 190)
            let high = UInt8(20 + (index * 71) % 200)
            return TrackEditorWaveformPoint(low: low, mid: mid, high: high)
        }
        let track = LibraryTrack(
            id: 1,
            sourceTrackID: "horizon-lines",
            title: "Horizon Lines",
            artist: "Lumi Procedural Audio",
            bpmMilli: 120_000,
            musicalKey: MusicalKey(pitchClass: .a, mode: .minor),
            durationMillis: UInt64(barCount * UInt32(beatsPerBar)) * beatDuration,
            colorRGB: 0x4870CD,
            analysisRevision: "horizon-lines-v1",
            timelineRevision: nil,
            readiness: .ready,
            missingCapabilities: [],
            warnings: []
        )
        return TrackEditorAnalysis(
            track: track,
            audioURI: "lumi-demo://horizon-lines",
            beatsPerBar: beatsPerBar,
            beats: beats,
            waveform: waveform,
            hotCues: [
                TrackEditorHotCue(
                    index: 1,
                    timeMillis: 8_000,
                    name: "",
                    colorRGB: 0x30_5A_FF
                ),
                TrackEditorHotCue(
                    index: 2,
                    timeMillis: 16_000,
                    name: "",
                    colorRGB: 0xFF_A0_00
                ),
                TrackEditorHotCue(
                    index: 3,
                    timeMillis: 24_000,
                    loopEndMillis: 28_000,
                    name: "",
                    colorRGB: 0xE6_28_28
                )
            ],
            phrases: [
                TrackEditorPhrase(
                    id: 0,
                    startBeat: 0,
                    endBeat: 16,
                    roleID: "intro-outro",
                    role: "Intro / Outro",
                    origin: "sourceImport",
                    loopStrategy: TrackEditorLoopStrategy(
                        kind: "fixedVariant",
                        locked: true,
                        provenance: "userSelection",
                        rowRoleID: "intro-outro",
                        fixedVariantID: "variant-1",
                        themeOverrides: [],
                        validatedCatalogRevision: 3,
                        status: "ready",
                        issues: []
                    )
                ),
                TrackEditorPhrase(id: 1, startBeat: 16, endBeat: 32, roleID: "breakdown-1", role: "Breakdown 1", origin: "sourceImport"),
                TrackEditorPhrase(id: 2, startBeat: 32, endBeat: 48, roleID: "buildup-1", role: "Buildup 1", origin: "sourceImport"),
                TrackEditorPhrase(id: 3, startBeat: 48, endBeat: 64, roleID: "drop", role: "Drop", origin: "sourceImport")
            ],
            roles: [
                TrackEditorRole(id: "intro-outro", name: "Intro / Outro"),
                TrackEditorRole(id: "breakdown-1", name: "Breakdown 1"),
                TrackEditorRole(id: "synth", name: "Synth"),
                TrackEditorRole(id: "buildup-1", name: "Buildup 1"),
                TrackEditorRole(id: "drop", name: "Drop")
            ],
            timeline: TrackEditorTimeline(
                revision: 3,
                baselineRevision: "horizon-lines-v1",
                origin: "userEdit",
                reason: "moveBoundary",
                canUndo: true,
                canRedo: false,
                revisions: [
                    TrackEditorRevision(revision: 3, origin: "userEdit", reason: "moveBoundary", phraseCount: 4, restoredFrom: nil),
                    TrackEditorRevision(revision: 2, origin: "userEdit", reason: "changeRole", phraseCount: 4, restoredFrom: nil),
                    TrackEditorRevision(revision: 1, origin: "sourceImport", reason: "initialSourceMapping", phraseCount: 4, restoredFrom: nil)
                ]
            ),
            sourceReconciliation: TrackSourceReconciliation(
                fromRevision: "horizon-lines-v1",
                toRevision: "horizon-lines-v2",
                sourceLibraryRevision: "lumi-demo-library-v2",
                changes: ["waveform", "rawPhrases"],
                metadataOnly: false,
                requiresTimelineDecision: true,
                sourceTotalBeats: 64,
                rebaseAmbiguities: [1],
                conflicts: [
                    TrackSourceConflict(
                        phraseIndex: 1,
                        lumi: TrackSourcePhraseVersion(
                            startBeat: 16,
                            endBeat: 32,
                            roleID: "breakdown-1"
                        ),
                        source: TrackSourcePhraseVersion(
                            startBeat: 16,
                            endBeat: 36,
                            roleID: "breakdown-1"
                        )
                    ),
                    TrackSourceConflict(
                        phraseIndex: 2,
                        lumi: TrackSourcePhraseVersion(
                            startBeat: 32,
                            endBeat: 48,
                            roleID: "buildup-1"
                        ),
                        source: TrackSourcePhraseVersion(
                            startBeat: 36,
                            endBeat: 48,
                            roleID: "buildup-1"
                        )
                    )
                ]
            )
        )
    }()
}
