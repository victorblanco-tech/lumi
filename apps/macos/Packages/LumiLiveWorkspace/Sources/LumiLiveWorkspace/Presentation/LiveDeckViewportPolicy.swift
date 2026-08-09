import LumiDesignSystem

enum LiveDeckViewportPolicy {
    static let defaultVisibleBars = 40.0
    static let playheadFraction = 0.22

    static func overview(
        totalBeats: UInt64,
        beatsPerBar: UInt8 = 4
    ) -> LumiWaveformViewport {
        LumiWaveformViewport(
            startBeat: 0,
            visibleBeats: Double(max(1, totalBeats)),
            totalBeats: max(1, totalBeats),
            beatsPerBar: beatsPerBar
        )
    }

    static func live(
        playheadBeat: Double,
        totalBeats: UInt64,
        visibleBeats: Double? = nil,
        beatsPerBar: UInt8 = 4
    ) -> LumiWaveformViewport {
        let resolvedVisibleBeats = visibleBeats
            ?? defaultVisibleBars * Double(max(1, beatsPerBar))
        return LumiWaveformViewport(
            startBeat: playheadBeat - resolvedVisibleBeats * playheadFraction,
            visibleBeats: resolvedVisibleBeats,
            totalBeats: max(1, totalBeats),
            beatsPerBar: beatsPerBar
        )
    }
}
