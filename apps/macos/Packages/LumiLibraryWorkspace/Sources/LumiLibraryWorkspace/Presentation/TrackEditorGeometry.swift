import Foundation

public struct TrackEditorViewport: Equatable, Sendable {
    public let startBar: UInt32
    public let visibleBars: UInt32
    public let totalBars: UInt32
    public let beatsPerBar: UInt8

    public init(startBar: UInt32, visibleBars: UInt32, totalBars: UInt32, beatsPerBar: UInt8) {
        let safeTotal = max(1, totalBars)
        let safeVisible = min(max(1, visibleBars), safeTotal)
        self.startBar = min(startBar, safeTotal - safeVisible)
        self.visibleBars = safeVisible
        self.totalBars = safeTotal
        self.beatsPerBar = max(1, beatsPerBar)
    }

    public var startBeat: UInt32 { startBar * UInt32(beatsPerBar) }
    public var visibleBeats: UInt32 { visibleBars * UInt32(beatsPerBar) }
    public var endBeat: UInt32 { startBeat + visibleBeats }

    public func x(forBeat beat: Double, width: Double) -> Double {
        let progress = (beat - Double(startBeat)) / Double(visibleBeats)
        return min(max(0, progress), 1) * max(0, width)
    }

    public func beat(atX x: Double, width: Double) -> Double {
        guard width > 0 else { return Double(startBeat) }
        let progress = min(max(0, x / width), 1)
        return Double(startBeat) + progress * Double(visibleBeats)
    }

    public func moving(byBars delta: Int) -> Self {
        let target = max(0, Int(startBar) + delta)
        return Self(
            startBar: UInt32(target),
            visibleBars: visibleBars,
            totalBars: totalBars,
            beatsPerBar: beatsPerBar
        )
    }

    public func zoomed(to bars: UInt32, aroundBar bar: UInt32) -> Self {
        let half = bars / 2
        let start = bar > half ? bar - half : 0
        return Self(
            startBar: start,
            visibleBars: bars,
            totalBars: totalBars,
            beatsPerBar: beatsPerBar
        )
    }
}

public enum TrackEditorCoordinateMapper {
    public static func beat(atTimeMillis time: UInt64, beats: [TrackEditorBeat]) -> Double {
        guard let first = beats.first else { return 0 }
        if time <= first.timeMillis { return Double(first.beatIndex) }
        for pair in zip(beats, beats.dropFirst()) where time < pair.1.timeMillis {
            let duration = max(1, pair.1.timeMillis - pair.0.timeMillis)
            let progress = Double(time - pair.0.timeMillis) / Double(duration)
            return Double(pair.0.beatIndex) + progress
        }
        return Double(beats.last?.beatIndex ?? 0) + 1
    }

    public static func timeMillis(atBeat beat: Double, analysis: TrackEditorAnalysis) -> UInt64 {
        let bounded = min(max(0, beat), Double(analysis.beats.count))
        let lower = Int(bounded.rounded(.down))
        guard lower < analysis.beats.count else { return analysis.track.durationMillis }
        let marker = analysis.beats[lower]
        let nextTime = lower + 1 < analysis.beats.count
            ? analysis.beats[lower + 1].timeMillis
            : analysis.track.durationMillis
        let fraction = bounded - Double(lower)
        return marker.timeMillis + UInt64(Double(nextTime - marker.timeMillis) * fraction)
    }
}

public enum TrackEditorEditGeometry {
    public static func containingBar(
        beat: Double,
        beatsPerBar: UInt8,
        totalBars: UInt32
    ) -> UInt32 {
        let safeTotal = max(1, totalBars)
        let safeBeatsPerBar = UInt32(max(1, beatsPerBar))
        let boundedBeat = UInt32(max(0, beat.rounded(.down)))
        return min(safeTotal - 1, boundedBeat / safeBeatsPerBar)
    }

    public static func barSelection(
        anchorBar: UInt32,
        currentBar: UInt32,
        totalBars: UInt32
    ) -> Range<UInt32> {
        let safeTotal = max(1, totalBars)
        let anchor = min(anchorBar, safeTotal - 1)
        let current = min(currentBar, safeTotal - 1)
        return min(anchor, current)..<min(safeTotal, max(anchor, current) + 1)
    }
}
