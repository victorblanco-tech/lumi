import Foundation
import Testing
@testable import LumiDesignSystem

struct LumiRGBWaveformSampleTests {
    @Test
    func mapsStoredPWV5FieldsToApprovedRGBOrder() throws {
        let sample = try #require(
            LumiRGBWaveformSample(low: 0.25, mid: 0.5, high: 1)
        )

        #expect(sample.red == 1)
        #expect(sample.green < sample.blue)
        #expect(abs(sample.green - pow(0.25, 0.72)) < 0.000_001)
        #expect(abs(sample.blue - pow(0.5, 0.72)) < 0.000_001)
    }

    @Test
    func preservesApprovedAmplitudeCurve() throws {
        let sample = try #require(
            LumiRGBWaveformSample(low: 0.2, mid: 0.4, high: 0.8)
        )

        #expect(abs(sample.amplitude - pow(0.8, 0.58)) < 0.000_001)
    }

    @Test
    func omitsSilentSamples() {
        #expect(LumiRGBWaveformSample(low: 0, mid: 0, high: 0) == nil)
    }
}
