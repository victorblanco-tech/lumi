import Foundation

public struct TrackEditorBeat: Equatable, Sendable {
    public let beatIndex: UInt32
    public let timeMillis: UInt64
    public let barIndex: UInt32
    public let beatInBar: UInt8

    public init(beatIndex: UInt32, timeMillis: UInt64, barIndex: UInt32, beatInBar: UInt8) {
        self.beatIndex = beatIndex
        self.timeMillis = timeMillis
        self.barIndex = barIndex
        self.beatInBar = beatInBar
    }
}

public struct TrackEditorWaveformPoint: Equatable, Sendable {
    public let low: UInt8
    public let mid: UInt8
    public let high: UInt8

    public init(low: UInt8, mid: UInt8, high: UInt8) {
        self.low = low
        self.mid = mid
        self.high = high
    }
}

public struct TrackEditorPhrase: Identifiable, Equatable, Sendable {
    public let id: UInt64
    public let startBeat: UInt32
    public let endBeat: UInt32
    public let role: String
    public let origin: String

    public init(id: UInt64, startBeat: UInt32, endBeat: UInt32, role: String, origin: String) {
        self.id = id
        self.startBeat = startBeat
        self.endBeat = endBeat
        self.role = role
        self.origin = origin
    }
}

public struct TrackEditorAnalysis: Identifiable, Equatable, Sendable {
    public var id: UInt64 { track.id }

    public let track: LibraryTrack
    public let audioURI: String
    public let beatsPerBar: UInt8
    public let beats: [TrackEditorBeat]
    public let waveform: [TrackEditorWaveformPoint]
    public let phrases: [TrackEditorPhrase]

    public init(
        track: LibraryTrack,
        audioURI: String,
        beatsPerBar: UInt8,
        beats: [TrackEditorBeat],
        waveform: [TrackEditorWaveformPoint],
        phrases: [TrackEditorPhrase]
    ) {
        self.track = track
        self.audioURI = audioURI
        self.beatsPerBar = beatsPerBar
        self.beats = beats
        self.waveform = waveform
        self.phrases = phrases
    }

    public var totalBars: UInt32 {
        guard beatsPerBar > 0 else { return 0 }
        return UInt32(beats.count) / UInt32(beatsPerBar)
    }

    public func timeMillis(atBeat beat: UInt32) -> UInt64 {
        if let marker = beats.first(where: { $0.beatIndex == beat }) {
            return marker.timeMillis
        }
        return track.durationMillis
    }

    public func phraseTimeRange(_ phrase: TrackEditorPhrase) -> Range<UInt64> {
        timeMillis(atBeat: phrase.startBeat)..<timeMillis(atBeat: phrase.endBeat)
    }
}
