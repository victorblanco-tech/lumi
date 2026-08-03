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
            phrases: [
                TrackEditorPhrase(id: 0, startBeat: 0, endBeat: 16, role: "Intro", origin: "source"),
                TrackEditorPhrase(id: 1, startBeat: 16, endBeat: 32, role: "Breakdown", origin: "source"),
                TrackEditorPhrase(id: 2, startBeat: 32, endBeat: 48, role: "Build", origin: "source"),
                TrackEditorPhrase(id: 3, startBeat: 48, endBeat: 64, role: "Drop", origin: "source")
            ]
        )
    }()
}
