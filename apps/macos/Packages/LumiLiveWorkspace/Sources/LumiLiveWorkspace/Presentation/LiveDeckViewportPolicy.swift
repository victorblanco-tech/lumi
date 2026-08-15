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

    static func manualPan(
        renderedViewport: LumiWaveformViewport,
        deltaPixels: Double,
        width: Double,
        reversesDirection: Bool
    ) -> (viewport: LumiWaveformViewport, usesLiveViewport: Bool) {
        let direction = reversesDirection ? -1.0 : 1.0
        return (
            renderedViewport.panned(
                byPixels: deltaPixels * direction,
                width: width
            ),
            false
        )
    }

    static func resumesFollow(
        previousDiscontinuityRevision: UInt64?,
        currentDiscontinuityRevision: UInt64?,
        isMaster: Bool
    ) -> Bool {
        guard isMaster,
              let previousDiscontinuityRevision,
              let currentDiscontinuityRevision else {
            return false
        }
        return currentDiscontinuityRevision != previousDiscontinuityRevision
    }
}
