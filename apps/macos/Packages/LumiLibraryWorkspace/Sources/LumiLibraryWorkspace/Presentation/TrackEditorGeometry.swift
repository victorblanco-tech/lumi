import Foundation

/// A continuous beat-space viewport. Navigation is deliberately independent
/// of bar boundaries; only phrase mutations are quantized to whole beats.
public struct TrackEditorViewport: Equatable, Sendable {
    public let startBeat: Double
    public let visibleBeats: Double
    public let totalBeats: UInt32
    public let beatsPerBar: UInt8

    public init(startBeat: Double, visibleBeats: Double, totalBeats: UInt32, beatsPerBar: UInt8) {
        let safeTotal = max(1, totalBeats)
        let safeVisible = min(max(1, visibleBeats), Double(safeTotal))
        self.startBeat = min(max(0, startBeat), Double(safeTotal) - safeVisible)
        self.visibleBeats = safeVisible
        self.totalBeats = safeTotal
        self.beatsPerBar = max(1, beatsPerBar)
    }

    public var endBeat: Double { startBeat + visibleBeats }
    public var visibleBars: Double { visibleBeats / Double(beatsPerBar) }

    public func x(forBeat beat: Double, width: Double) -> Double {
        (beat - startBeat) / visibleBeats * max(0, width)
    }

    public func beat(atX x: Double, width: Double) -> Double {
        guard width > 0 else { return startBeat }
        return startBeat + min(max(0, x / width), 1) * visibleBeats
    }

    public func moving(byBeats delta: Double) -> Self {
        Self(
            startBeat: startBeat + delta,
            visibleBeats: visibleBeats,
            totalBeats: totalBeats,
            beatsPerBar: beatsPerBar
        )
    }

    public func centered(onBeat beat: Double) -> Self {
        Self(
            startBeat: beat - visibleBeats / 2,
            visibleBeats: visibleBeats,
            totalBeats: totalBeats,
            beatsPerBar: beatsPerBar
        )
    }

    public func zoomed(to beats: Double, aroundBeat beat: Double) -> Self {
        let anchor = visibleBeats > 0 ? (beat - startBeat) / visibleBeats : 0.5
        let boundedAnchor = min(max(0, anchor), 1)
        return Self(
            startBeat: beat - boundedAnchor * beats,
            visibleBeats: beats,
            totalBeats: totalBeats,
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
    public static func quantizedBeat(_ beat: Double, totalBeats: UInt32) -> UInt32 {
        UInt32(min(max(0, beat.rounded()), Double(totalBeats)))
    }

    public static func beatSelection(
        anchorBeat: UInt32,
        currentBeat: UInt32,
        totalBeats: UInt32
    ) -> Range<UInt32> {
        let safeTotal = max(1, totalBeats)
        let anchor = min(anchorBeat, safeTotal - 1)
        let current = min(currentBeat, safeTotal - 1)
        return min(anchor, current)..<min(safeTotal, max(anchor, current) + 1)
    }
}
