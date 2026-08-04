import AVFoundation
import Combine
import Foundation

public enum TrackAudioPreviewResolution: Equatable, Sendable {
    case syntheticDemo(String)
    case localFile(URL)
    case unavailable(String)
}

public enum TrackAudioPreviewResolver {
    public static func resolve(_ uri: String) -> TrackAudioPreviewResolution {
        if uri.hasPrefix("lumi-demo://") {
            return .syntheticDemo(uri)
        }
        let url: URL?
        if uri.hasPrefix("/") {
            url = URL(fileURLWithPath: uri)
        } else if let candidate = URL(string: uri), candidate.isFileURL {
            url = candidate
        } else {
            url = nil
        }
        guard let url else {
            return .unavailable("Preview is unavailable for this audio source.")
        }
        guard FileManager.default.isReadableFile(atPath: url.path) else {
            return .unavailable("The original audio file is missing or unreadable.")
        }
        return .localFile(url)
    }
}

struct TrackAudioScheduleGeneration: Equatable, Sendable {
    private(set) var current: UInt64 = 0

    @discardableResult
    mutating func invalidate() -> UInt64 {
        current &+= 1
        return current
    }

    func isCurrent(_ generation: UInt64) -> Bool {
        generation == current
    }
}

enum TrackAudioLoopTransition {
    static func position(
        current: UInt64,
        loop: Range<UInt64>,
        preservingPosition: Bool
    ) -> UInt64 {
        preservingPosition && loop.contains(current) ? current : loop.lowerBound
    }
}

@MainActor
public final class TrackAudioPreviewController: ObservableObject {
    @Published public private(set) var isPlaying = false
    @Published public private(set) var positionMillis: UInt64 = 0
    @Published public private(set) var unavailableReason: String?
    @Published public var volume: Double = 0.75 {
        didSet { player?.volume = Float(min(max(0, volume), 1)) }
    }

    private let analysis: TrackEditorAnalysis
    private var engine: AVAudioEngine?
    private var player: AVAudioPlayerNode?
    private var buffer: AVAudioPCMBuffer?
    private var playbackTask: Task<Void, Never>?
    private var scheduledStartFrame: AVAudioFramePosition = 0
    private var scheduledFrameCount: AVAudioFrameCount = 0
    private var scheduledLoops = false
    private var loopMillis: Range<UInt64>?
    private var scheduleGeneration = TrackAudioScheduleGeneration()

    public init(analysis: TrackEditorAnalysis) {
        self.analysis = analysis
        do {
            buffer = try Self.loadBuffer(for: analysis)
        } catch {
            unavailableReason = "Preview unavailable: \(error.localizedDescription)"
        }
    }

    public func togglePlayback() {
        isPlaying ? pause() : play()
    }

    public func play() {
        guard let buffer, unavailableReason == nil else { return }
        do {
            let (engine, player) = prepareEngine(for: buffer)
            if !engine.isRunning { try engine.start() }
            invalidateSchedule()
            player.stop()
            if positionMillis >= analysis.track.durationMillis {
                positionMillis = loopMillis?.lowerBound ?? 0
            }
            schedule(
                buffer: buffer,
                player: player,
                fromMillis: positionMillis,
                generation: scheduleGeneration.current
            )
            player.play()
            isPlaying = true
            startPositionUpdates()
        } catch {
            unavailableReason = "Preview unavailable: \(error.localizedDescription)"
            isPlaying = false
        }
    }

    public func pause() {
        refreshPosition()
        invalidateSchedule()
        player?.pause()
        isPlaying = false
        playbackTask?.cancel()
        playbackTask = nil
    }

    public func stop() {
        invalidateSchedule()
        player?.stop()
        isPlaying = false
        positionMillis = 0
        playbackTask?.cancel()
        playbackTask = nil
    }

    public func seek(toMillis value: UInt64) {
        let wasPlaying = isPlaying
        invalidateSchedule()
        player?.stop()
        positionMillis = min(value, analysis.track.durationMillis)
        if wasPlaying { play() }
    }

    public func moveByBar(_ delta: Int) {
        let currentBeat = TrackEditorCoordinateMapper.beat(
            atTimeMillis: positionMillis,
            beats: analysis.beats
        )
        let currentBar = Int(currentBeat) / Int(analysis.beatsPerBar)
        let targetBar = max(0, min(Int(analysis.totalBars) - 1, currentBar + delta))
        let targetBeat = Double(targetBar * Int(analysis.beatsPerBar))
        seek(toMillis: TrackEditorCoordinateMapper.timeMillis(atBeat: targetBeat, analysis: analysis))
    }

    public func moveByBeat(_ delta: Int) {
        let currentBeat = TrackEditorCoordinateMapper.beat(
            atTimeMillis: positionMillis,
            beats: analysis.beats
        )
        let targetBeat = min(
            max(0, currentBeat.rounded(.down) + Double(delta)),
            Double(analysis.totalBeats)
        )
        seek(toMillis: TrackEditorCoordinateMapper.timeMillis(atBeat: targetBeat, analysis: analysis))
    }

    @discardableResult
    public func setLoop(_ phrase: TrackEditorPhrase?) -> Bool {
        updateLoop(phrase, preservingPosition: false)
    }

    @discardableResult
    public func adoptEditedLoop(_ phrase: TrackEditorPhrase?) -> Bool {
        updateLoop(phrase, preservingPosition: true)
    }

    private func updateLoop(
        _ phrase: TrackEditorPhrase?,
        preservingPosition: Bool
    ) -> Bool {
        if let phrase {
            let beatsPerBar = UInt32(max(1, analysis.beatsPerBar))
            guard phrase.startBeat < phrase.endBeat,
                  phrase.endBeat <= UInt32(analysis.beats.count),
                  phrase.startBeat.isMultiple(of: beatsPerBar),
                  phrase.endBeat.isMultiple(of: beatsPerBar) else {
                return false
            }
        }
        let wasPlaying = isPlaying
        if wasPlaying { refreshPosition() }
        let previousPosition = positionMillis
        invalidateSchedule()
        player?.stop()
        loopMillis = phrase.map(analysis.phraseTimeRange)
        if let loopMillis {
            positionMillis = TrackAudioLoopTransition.position(
                current: previousPosition,
                loop: loopMillis,
                preservingPosition: preservingPosition
            )
        }
        if wasPlaying { play() }
        return true
    }

    public func shutdown() {
        invalidateSchedule()
        player?.stop()
        engine?.stop()
        if let player { engine?.detach(player) }
        player = nil
        engine = nil
        playbackTask?.cancel()
        playbackTask = nil
        isPlaying = false
    }

    private func prepareEngine(
        for buffer: AVAudioPCMBuffer
    ) -> (AVAudioEngine, AVAudioPlayerNode) {
        if let engine, let player { return (engine, player) }
        let engine = AVAudioEngine()
        let player = AVAudioPlayerNode()
        engine.attach(player)
        engine.connect(player, to: engine.mainMixerNode, format: buffer.format)
        player.volume = Float(volume)
        self.engine = engine
        self.player = player
        return (engine, player)
    }

    private func schedule(
        buffer: AVAudioPCMBuffer,
        player: AVAudioPlayerNode,
        fromMillis: UInt64,
        generation: UInt64
    ) {
        let sampleRate = buffer.format.sampleRate
        let totalFrames = AVAudioFramePosition(buffer.frameLength)
        let requestedFrame = Self.frame(forMillis: fromMillis, sampleRate: sampleRate)
        let loopFrames = loopMillis.map { range in
            Self.frame(forMillis: range.lowerBound, sampleRate: sampleRate)..<min(
                totalFrames,
                Self.frame(forMillis: range.upperBound, sampleRate: sampleRate)
            )
        }
        let startFrame: AVAudioFramePosition
        let endFrame: AVAudioFramePosition
        if let loopFrames {
            startFrame = loopFrames.lowerBound
            endFrame = loopFrames.upperBound
            scheduledLoops = true
        } else {
            startFrame = min(requestedFrame, max(0, totalFrames - 1))
            endFrame = totalFrames
            scheduledLoops = false
        }
        guard endFrame > startFrame,
              let slice = Self.slice(buffer, start: startFrame, end: endFrame) else {
            stop()
            return
        }
        scheduledStartFrame = startFrame
        scheduledFrameCount = slice.frameLength
        let options: AVAudioPlayerNodeBufferOptions = scheduledLoops ? .loops : []
        player.scheduleBuffer(slice, at: nil, options: options) { [weak self] in
            Task { @MainActor [weak self] in
                guard let self,
                      self.scheduleGeneration.isCurrent(generation),
                      !self.scheduledLoops else { return }
                self.positionMillis = self.analysis.track.durationMillis
                self.isPlaying = false
                self.playbackTask?.cancel()
                self.playbackTask = nil
            }
        }
    }

    private func startPositionUpdates() {
        playbackTask?.cancel()
        playbackTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(33))
                guard let self, self.isPlaying else { return }
                self.refreshPosition()
            }
        }
    }

    private func invalidateSchedule() {
        scheduleGeneration.invalidate()
    }

    private func refreshPosition() {
        guard let buffer, let player,
              let renderTime = player.lastRenderTime,
              let playerTime = player.playerTime(forNodeTime: renderTime) else { return }
        let elapsed = max(0, playerTime.sampleTime)
        let relative = scheduledLoops && scheduledFrameCount > 0
            ? elapsed % AVAudioFramePosition(scheduledFrameCount)
            : min(elapsed, AVAudioFramePosition(scheduledFrameCount))
        let frame = scheduledStartFrame + relative
        positionMillis = min(
            analysis.track.durationMillis,
            UInt64(Double(frame) / buffer.format.sampleRate * 1_000)
        )
    }

    private static func loadBuffer(for analysis: TrackEditorAnalysis) throws -> AVAudioPCMBuffer {
        switch TrackAudioPreviewResolver.resolve(analysis.audioURI) {
        case let .syntheticDemo(seed):
            return try syntheticBuffer(seed: seed, durationMillis: analysis.track.durationMillis)
        case let .localFile(url):
            let file = try AVAudioFile(forReading: url)
            guard file.length > 0,
                  file.length <= AVAudioFramePosition(UInt32.max),
                  let buffer = AVAudioPCMBuffer(
                    pcmFormat: file.processingFormat,
                    frameCapacity: AVAudioFrameCount(file.length)
                  ) else {
                throw TrackAudioPreviewError.invalidAudio
            }
            try file.read(into: buffer)
            return buffer
        case let .unavailable(reason):
            throw TrackAudioPreviewError.unavailable(reason)
        }
    }

    private static func syntheticBuffer(
        seed: String,
        durationMillis: UInt64
    ) throws -> AVAudioPCMBuffer {
        let sampleRate = 44_100.0
        let frameCount64 = durationMillis * 44_100 / 1_000
        guard frameCount64 > 0,
              frameCount64 <= UInt64(UInt32.max),
              let format = AVAudioFormat(standardFormatWithSampleRate: sampleRate, channels: 1),
              let buffer = AVAudioPCMBuffer(
                pcmFormat: format,
                frameCapacity: AVAudioFrameCount(frameCount64)
              ),
              let samples = buffer.floatChannelData?[0] else {
            throw TrackAudioPreviewError.invalidAudio
        }
        buffer.frameLength = AVAudioFrameCount(frameCount64)
        let hash = seed.utf8.reduce(UInt32(2_166_136_261)) { ($0 ^ UInt32($1)) &* 16_777_619 }
        let frequency = 110.0 + Double(hash % 220)
        for frame in 0..<Int(frameCount64) {
            let time = Double(frame) / sampleRate
            let saw = Float((time * frequency).truncatingRemainder(dividingBy: 1) * 2 - 1)
            let pulse = Float(exp(-18 * (time.truncatingRemainder(dividingBy: 0.5))))
            samples[frame] = saw * 0.12 + pulse * 0.08
        }
        return buffer
    }

    private static func slice(
        _ source: AVAudioPCMBuffer,
        start: AVAudioFramePosition,
        end: AVAudioFramePosition
    ) -> AVAudioPCMBuffer? {
        let count = AVAudioFrameCount(end - start)
        guard count > 0,
              let result = AVAudioPCMBuffer(pcmFormat: source.format, frameCapacity: count) else {
            return nil
        }
        result.frameLength = count
        let channelCount = Int(source.format.channelCount)
        guard let sourceChannels = source.floatChannelData,
              let resultChannels = result.floatChannelData else { return nil }
        for channel in 0..<channelCount {
            resultChannels[channel].update(
                from: sourceChannels[channel].advanced(by: Int(start)),
                count: Int(count)
            )
        }
        return result
    }

    private static func frame(forMillis millis: UInt64, sampleRate: Double) -> AVAudioFramePosition {
        AVAudioFramePosition(Double(millis) / 1_000 * sampleRate)
    }
}

private enum TrackAudioPreviewError: LocalizedError {
    case unavailable(String)
    case invalidAudio

    var errorDescription: String? {
        switch self {
        case let .unavailable(reason): reason
        case .invalidAudio: "The original audio file is empty, unsupported, or corrupt."
        }
    }
}
