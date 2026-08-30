import Foundation

/// The single display mapping for Lumi's Rekordbox-compatible RGB waveforms.
///
/// Stored waveform points retain the engine's frequency-band field names. The
/// approved CDJ/Rekordbox-style presentation uses the packed PWV5 channel order
/// red, green, blue, which maps to high, low, mid in those stored fields.
public struct LumiRGBWaveformSample: Equatable, Sendable {
    public let amplitude: Double
    public let red: Double
    public let green: Double
    public let blue: Double

    public init?(low: Double, mid: Double, high: Double) {
        let peak = max(low, max(mid, high))
        guard peak > 0.000_1 else { return nil }

        amplitude = pow(peak, 0.58)
        red = pow(high / peak, 0.72)
        green = pow(low / peak, 0.72)
        blue = pow(mid / peak, 0.72)
    }
}
