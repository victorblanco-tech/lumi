import Foundation
import LumiDesignSystem

public typealias TrackEditorViewport = LumiWaveformViewport

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
