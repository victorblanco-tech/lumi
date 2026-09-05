import Foundation
import LumiProtocol

public let lumiRemoteProtocolVersion = 1
public let lumiRemoteMaximumFrameBytes = 512 * 1_024

public enum RemoteFrameKind: String, Codable, Sendable {
    case hello
    case snapshot
    case projection
    case transportAnchor
    case command
    case commandResult
    case error
}

public struct RemoteFrame: Codable, Equatable, Sendable {
    public let protocolVersion: Int
    public let frameKind: RemoteFrameKind
    public let sequence: UInt64
    public let correlationID: String?
    public let payload: JSONValue

    public init(
        protocolVersion: Int = lumiRemoteProtocolVersion,
        frameKind: RemoteFrameKind,
        sequence: UInt64,
        correlationID: String? = nil,
        payload: JSONValue
    ) {
        self.protocolVersion = protocolVersion
        self.frameKind = frameKind
        self.sequence = sequence
        self.correlationID = correlationID
        self.payload = payload
    }

    enum CodingKeys: String, CodingKey {
        case protocolVersion
        case frameKind
        case sequence
        case correlationID = "correlationId"
        case payload
    }
}

public enum RemoteOperationState: String, Codable, CaseIterable, Sendable {
    case off
    case armed
    case live
    case paused
}

public enum RemoteIntegrationHealth: String, Codable, Sendable {
    case unavailable
    case starting
    case ready
    case degraded
}

public struct RemoteIntegrationStatus: Codable, Equatable, Sendable {
    public let proDJLink: RemoteIntegrationHealth
    public let lightOutput: RemoteIntegrationHealth
    public let abletonLink: RemoteIntegrationHealth
    public let abletonLinkEnabled: Bool
    public let abletonLinkBPMMilli: UInt64?
    public let timingOffsetMillis: Int
    public let pendingTimingOffsetMillis: Int?

    public init(
        proDJLink: RemoteIntegrationHealth,
        lightOutput: RemoteIntegrationHealth,
        abletonLink: RemoteIntegrationHealth,
        abletonLinkEnabled: Bool,
        abletonLinkBPMMilli: UInt64?,
        timingOffsetMillis: Int,
        pendingTimingOffsetMillis: Int?
    ) {
        self.proDJLink = proDJLink
        self.lightOutput = lightOutput
        self.abletonLink = abletonLink
        self.abletonLinkEnabled = abletonLinkEnabled
        self.abletonLinkBPMMilli = abletonLinkBPMMilli
        self.timingOffsetMillis = timingOffsetMillis
        self.pendingTimingOffsetMillis = pendingTimingOffsetMillis
    }

    enum CodingKeys: String, CodingKey {
        case proDJLink = "proDjLink"
        case lightOutput
        case abletonLink
        case abletonLinkEnabled
        case abletonLinkBPMMilli = "abletonLinkBpmMilli"
        case timingOffsetMillis
        case pendingTimingOffsetMillis
    }
}

public struct RemoteLiveProjection: Codable, Equatable, Sendable {
    public let projectionRevision: UInt64
    public let stateRevision: UInt64
    public let engineVersion: String
    public let operationState: RemoteOperationState
    public let leaderPlayerNumber: UInt8?
    public let integrations: RemoteIntegrationStatus
    public let players: [RemotePlayer]
    public let livePlan: RemoteLightPlan?
    public let nextPlan: RemoteLightPlan?
    public let themeOptions: [RemoteThemeOption]
    public let phraseRoleOptions: [RemotePhraseRoleOption]

    public init(
        projectionRevision: UInt64,
        stateRevision: UInt64,
        engineVersion: String,
        operationState: RemoteOperationState,
        leaderPlayerNumber: UInt8?,
        integrations: RemoteIntegrationStatus,
        players: [RemotePlayer],
        livePlan: RemoteLightPlan?,
        nextPlan: RemoteLightPlan?,
        themeOptions: [RemoteThemeOption],
        phraseRoleOptions: [RemotePhraseRoleOption] = []
    ) {
        self.projectionRevision = projectionRevision
        self.stateRevision = stateRevision
        self.engineVersion = engineVersion
        self.operationState = operationState
        self.leaderPlayerNumber = leaderPlayerNumber
        self.integrations = integrations
        self.players = players
        self.livePlan = livePlan
        self.nextPlan = nextPlan
        self.themeOptions = themeOptions
        self.phraseRoleOptions = phraseRoleOptions
    }

    enum CodingKeys: String, CodingKey {
        case projectionRevision
        case stateRevision
        case engineVersion
        case operationState
        case leaderPlayerNumber
        case integrations
        case players
        case livePlan
        case nextPlan
        case themeOptions
        case phraseRoleOptions
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        projectionRevision = try container.decode(UInt64.self, forKey: .projectionRevision)
        stateRevision = try container.decode(UInt64.self, forKey: .stateRevision)
        engineVersion = try container.decode(String.self, forKey: .engineVersion)
        operationState = try container.decode(RemoteOperationState.self, forKey: .operationState)
        leaderPlayerNumber = try container.decodeIfPresent(UInt8.self, forKey: .leaderPlayerNumber)
        integrations = try container.decode(RemoteIntegrationStatus.self, forKey: .integrations)
        players = try container.decode([RemotePlayer].self, forKey: .players)
        livePlan = try container.decodeIfPresent(RemoteLightPlan.self, forKey: .livePlan)
        nextPlan = try container.decodeIfPresent(RemoteLightPlan.self, forKey: .nextPlan)
        themeOptions = try container.decode([RemoteThemeOption].self, forKey: .themeOptions)
        phraseRoleOptions = try container.decodeIfPresent(
            [RemotePhraseRoleOption].self,
            forKey: .phraseRoleOptions
        ) ?? []
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(projectionRevision, forKey: .projectionRevision)
        try container.encode(stateRevision, forKey: .stateRevision)
        try container.encode(engineVersion, forKey: .engineVersion)
        try container.encode(operationState, forKey: .operationState)
        try container.encodeIfPresent(leaderPlayerNumber, forKey: .leaderPlayerNumber)
        try container.encode(integrations, forKey: .integrations)
        try container.encode(players, forKey: .players)
        try container.encodeIfPresent(livePlan, forKey: .livePlan)
        try container.encodeIfPresent(nextPlan, forKey: .nextPlan)
        try container.encode(themeOptions, forKey: .themeOptions)
        try container.encode(phraseRoleOptions, forKey: .phraseRoleOptions)
    }
}

public struct RemotePlayer: Codable, Equatable, Identifiable, Sendable {
    public var id: UInt8 { playerNumber }
    public let playerNumber: UInt8
    public let hardwareModel: String?
    public let trackLoadID: UInt64
    public let transport: RemoteTransportAnchor
    public let track: RemoteTrack

    enum CodingKeys: String, CodingKey {
        case playerNumber
        case hardwareModel
        case trackLoadID = "trackLoadId"
        case transport
        case track
    }
}

public struct RemoteTransportAnchor: Codable, Equatable, Sendable {
    public let trackLoadID: UInt64
    public let beat: UInt64
    public let positionMillis: UInt64?
    public let effectiveBPMMilli: UInt64
    public let playing: Bool
    public let discontinuityRevision: UInt64
    public let observedAtUnixMillis: UInt64
    public let publishedAtUnixMillis: UInt64?

    public init(
        trackLoadID: UInt64,
        beat: UInt64,
        positionMillis: UInt64?,
        effectiveBPMMilli: UInt64,
        playing: Bool,
        discontinuityRevision: UInt64,
        observedAtUnixMillis: UInt64,
        publishedAtUnixMillis: UInt64? = nil
    ) {
        self.trackLoadID = trackLoadID
        self.beat = beat
        self.positionMillis = positionMillis
        self.effectiveBPMMilli = effectiveBPMMilli
        self.playing = playing
        self.discontinuityRevision = discontinuityRevision
        self.observedAtUnixMillis = observedAtUnixMillis
        self.publishedAtUnixMillis = publishedAtUnixMillis
    }

    enum CodingKeys: String, CodingKey {
        case trackLoadID = "trackLoadId"
        case beat
        case positionMillis
        case effectiveBPMMilli = "effectiveBpmMilli"
        case playing
        case discontinuityRevision
        case observedAtUnixMillis
        case publishedAtUnixMillis
    }
}

public struct RemoteTrack: Codable, Equatable, Sendable {
    public let trackID: UInt64?
    public let title: String
    public let artist: String
    public let originalBPMMilli: UInt64
    public let colorRGB: UInt32?
    public let key: String
    public let durationBeats: UInt64
    public let beatGrid: RemoteBeatGrid?
    public let waveform: [RemoteWaveformPoint]
    public let hotCues: [RemoteHotCue]
    public let phrases: [RemotePhrase]

    enum CodingKeys: String, CodingKey {
        case trackID = "trackId"
        case title
        case artist
        case originalBPMMilli = "originalBpmMilli"
        case colorRGB = "colorRgb"
        case key
        case durationBeats
        case beatGrid
        case waveform
        case hotCues
        case phrases
    }
}

public struct RemoteBeatGrid: Codable, Equatable, Sendable {
    public let beatsPerBar: UInt8
    public let durationMillis: UInt64
    public let timesMillis: [UInt64]
}

public struct RemoteWaveformPoint: Codable, Equatable, Sendable {
    public let low: UInt8
    public let mid: UInt8
    public let high: UInt8
}

extension RemoteWaveformPoint {
    private enum CodingKeys: String, CodingKey {
        case low
        case mid
        case high
    }

    public init(from decoder: Decoder) throws {
        let single = try decoder.singleValueContainer()
        if let packed = try? single.decode(String.self) {
            guard packed.utf8.count == 6,
                  let low = UInt8(packed.prefix(2), radix: 16),
                  let mid = UInt8(packed.dropFirst(2).prefix(2), radix: 16),
                  let high = UInt8(packed.suffix(2), radix: 16) else {
                throw DecodingError.dataCorruptedError(
                    in: single,
                    debugDescription: "Packed waveform points require exactly six hexadecimal characters."
                )
            }
            self.low = low
            self.mid = mid
            self.high = high
            return
        }

        let keyed = try decoder.container(keyedBy: CodingKeys.self)
        low = try keyed.decode(UInt8.self, forKey: .low)
        mid = try keyed.decode(UInt8.self, forKey: .mid)
        high = try keyed.decode(UInt8.self, forKey: .high)
    }

    public func encode(to encoder: Encoder) throws {
        var single = encoder.singleValueContainer()
        try single.encode(String(format: "%02x%02x%02x", low, mid, high))
    }
}

public struct RemoteHotCue: Codable, Equatable, Identifiable, Sendable {
    public var id: UInt8 { index }
    public let index: UInt8
    public let timeMillis: UInt64
    public let loopEndMillis: UInt64?
    public let colorRGB: UInt32

    enum CodingKeys: String, CodingKey {
        case index
        case timeMillis
        case loopEndMillis
        case colorRGB = "colorRgb"
    }
}

public struct RemotePhrase: Codable, Equatable, Identifiable, Sendable {
    public var id: UInt16 { index }
    public let index: UInt16
    public let startBeat: UInt64
    public let endBeat: UInt64
    public let kind: String
    public let roleID: String?
    public let roleName: String?
    public let colorRGB: UInt32?

    enum CodingKeys: String, CodingKey {
        case index
        case startBeat
        case endBeat
        case kind
        case roleID = "roleId"
        case roleName
        case colorRGB = "colorRgb"
    }
}

public struct RemoteLightPlan: Codable, Equatable, Sendable {
    public let planID: String
    public let playerNumber: UInt8
    public let trackLoadID: UInt64
    public let revision: UInt64
    public let themeID: UInt64?
    public let themeName: String?
    public let cues: [RemotePlanCue]

    enum CodingKeys: String, CodingKey {
        case planID = "planId"
        case playerNumber
        case trackLoadID = "trackLoadId"
        case revision
        case themeID = "themeId"
        case themeName
        case cues
    }
}

public struct RemotePlanCue: Codable, Equatable, Identifiable, Sendable {
    public var id: UInt16 { phraseIndex }
    public let phraseIndex: UInt16
    public let startBeat: UInt64
    public let endBeat: UInt64
    public let locked: Bool
    public let themeID: UInt64?
    public let themeName: String?
    public let autoloopNumber: UInt8?
    public let autoloopName: String?
    public let staticLookName: String?
    public let availableAutoloops: [RemoteAutoloopChoice]

    enum CodingKeys: String, CodingKey {
        case phraseIndex
        case startBeat
        case endBeat
        case locked
        case themeID = "themeId"
        case themeName
        case autoloopNumber
        case autoloopName
        case staticLookName
        case availableAutoloops
    }
}

public struct RemoteThemeOption: Codable, Equatable, Identifiable, Sendable {
    public let id: UInt64
    public let name: String
}

public struct RemotePhraseRoleOption: Codable, Equatable, Identifiable, Sendable {
    public let id: String
    public let name: String
    public let colorRGB: UInt32

    public init(id: String, name: String, colorRGB: UInt32) {
        self.id = id
        self.name = name
        self.colorRGB = colorRGB
    }

    enum CodingKeys: String, CodingKey {
        case id
        case name
        case colorRGB = "colorRgb"
    }
}

public struct RemoteAutoloopChoice: Codable, Equatable, Identifiable, Sendable {
    public var id: UInt8 { number }
    public let number: UInt8
    public let name: String
    public let bankNumber: UInt8
}

public enum RemoteContractError: Error, Equatable {
    case oversizedFrame
    case unsupportedProtocol(Int)
    case wrongFrameKind
    case invalidFrame
    case nonIncreasingRevision
    case invalidTransportAnchor
}

public struct RemoteFrameDecoder: Sendable {
    private let decoder = JSONDecoder()

    public init() {}

    public func decodeFrame(_ data: Data) throws -> RemoteFrame {
        guard data.count <= lumiRemoteMaximumFrameBytes else {
            throw RemoteContractError.oversizedFrame
        }
        let frame: RemoteFrame
        do {
            frame = try decoder.decode(RemoteFrame.self, from: data)
        } catch {
            throw RemoteContractError.invalidFrame
        }
        guard frame.protocolVersion == lumiRemoteProtocolVersion else {
            throw RemoteContractError.unsupportedProtocol(frame.protocolVersion)
        }
        guard frame.sequence > 0 else {
            throw RemoteContractError.invalidFrame
        }
        return frame
    }

    public func decodeProjection(_ frame: RemoteFrame) throws -> RemoteLiveProjection {
        guard [.snapshot, .projection].contains(frame.frameKind) else {
            throw RemoteContractError.wrongFrameKind
        }
        let data: Data
        do {
            data = try JSONEncoder().encode(frame.payload)
            return try decoder.decode(RemoteLiveProjection.self, from: data)
        } catch {
            throw RemoteContractError.invalidFrame
        }
    }

    public func decodeTransportAnchor(
        _ frame: RemoteFrame
    ) throws -> (playerNumber: UInt8, anchor: RemoteTransportAnchor) {
        guard frame.frameKind == .transportAnchor,
              case let .object(payload) = frame.payload,
              case let .number(playerNumberValue)? = payload["playerNumber"],
              let playerNumber = UInt8(exactly: playerNumberValue) else {
            throw RemoteContractError.invalidTransportAnchor
        }
        let anchorValue = payload["anchor"] ?? frame.payload
        do {
            let data = try JSONEncoder().encode(anchorValue)
            let anchor = try decoder.decode(RemoteTransportAnchor.self, from: data)
            return (playerNumber, anchor)
        } catch {
            throw RemoteContractError.invalidTransportAnchor
        }
    }

    public func decodeCommandResult(_ frame: RemoteFrame) throws -> RemoteCommandResult {
        guard frame.frameKind == .commandResult else {
            throw RemoteContractError.wrongFrameKind
        }
        do {
            let data = try JSONEncoder().encode(frame.payload)
            let result = try decoder.decode(RemoteCommandResult.self, from: data)
            guard result.commandID == frame.correlationID,
                  !result.commandID.isEmpty,
                  result.commandID.count <= 128 else {
                throw RemoteContractError.invalidFrame
            }
            return result
        } catch let error as RemoteContractError {
            throw error
        } catch {
            throw RemoteContractError.invalidFrame
        }
    }
}
