import Foundation

struct LiveDeckVisualTimeline: Sendable {
    static func playheadBeat(
        trackLoadID: UInt64,
        durationBeats: UInt64,
        fallbackBeat: Double,
        visualClock: DeckVisualClockSnapshot?,
        beatGrid: LiveBeatGridTimeline?,
        at date: Date
    ) -> Double {
        let totalBeats = Double(max(1, durationBeats))
        guard let visualClock,
              visualClock.trackLoadID == trackLoadID,
              visualClock.durationMillis > 0 else {
            return min(totalBeats, max(0, fallbackBeat))
        }
        let positionMillis = visualClock.positionMillis(at: date)
        if let beatGrid {
            return min(totalBeats, max(
                0,
                beatGrid.beat(atTimeMillis: positionMillis)
            ))
        }
        return min(totalBeats, max(
            0,
            positionMillis / Double(visualClock.durationMillis) * totalBeats
        ))
    }
}

struct LiveBeatGridTimeline: Equatable, Sendable {
    let durationMillis: UInt64
    let totalBeats: UInt64
    let beatsPerBar: UInt8
    let timesMillis: [UInt64]

    init?(grid: DeckBeatGridSnapshot?, totalBeats: UInt64) {
        guard let grid,
              totalBeats > 0,
              grid.durationMillis > 0,
              grid.timesMillis.count >= 2 else {
            return nil
        }
        self.durationMillis = grid.durationMillis
        self.totalBeats = totalBeats
        self.beatsPerBar = grid.beatsPerBar
        self.timesMillis = grid.timesMillis
    }

    func timeMillis(atBeat beat: Double) -> Double {
        let boundedBeat = min(max(0, beat), Double(totalBeats))
        let lowerIndex = Int(boundedBeat.rounded(.down))
        guard lowerIndex < timesMillis.count else { return Double(durationMillis) }
        let markerTime = timesMillis[lowerIndex]
        let nextTime = lowerIndex + 1 < timesMillis.count
            ? timesMillis[lowerIndex + 1]
            : durationMillis
        guard nextTime > markerTime else { return Double(markerTime) }
        let fraction = boundedBeat - Double(lowerIndex)
        return Double(markerTime) + Double(nextTime - markerTime) * fraction
    }

    func beat(atTimeMillis timeMillis: Double) -> Double {
        let boundedTime = min(max(0, timeMillis), Double(durationMillis))
        guard boundedTime > Double(timesMillis[0]) else { return 0 }
        let upperIndex = timesMillis.partitioningIndex {
            Double($0) > boundedTime
        }
        guard upperIndex < timesMillis.count else {
            let lastIndex = timesMillis.count - 1
            let lastTime = timesMillis[lastIndex]
            guard durationMillis > lastTime else {
                return min(Double(totalBeats), Double(lastIndex))
            }
            let fraction = (boundedTime - Double(lastTime))
                / Double(durationMillis - lastTime)
            return min(
                Double(totalBeats),
                Double(lastIndex)
                    + fraction * (Double(totalBeats) - Double(lastIndex))
            )
        }
        let lowerTime = timesMillis[upperIndex - 1]
        let upperTime = timesMillis[upperIndex]
        let interval = Double(upperTime - lowerTime)
        guard interval > 0 else { return Double(upperIndex - 1) }
        let fraction = (boundedTime - Double(lowerTime)) / interval
        return Double(upperIndex - 1) + fraction
    }

    func trackProgress(atBeat beat: Double) -> Double {
        min(max(0, timeMillis(atBeat: beat) / Double(durationMillis)), 1)
    }
}

private extension RandomAccessCollection {
    func partitioningIndex(where predicate: (Element) throws -> Bool) rethrows -> Index {
        var lower = startIndex
        var count = self.count
        while count > 0 {
            let step = count / 2
            let middle = index(lower, offsetBy: step)
            if try predicate(self[middle]) {
                count = step
            } else {
                lower = index(after: middle)
                count -= step + 1
            }
        }
        return lower
    }
}
