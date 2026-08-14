import SwiftUI

public enum LumiTypography {
    public static let screenTitle = Font.title2.weight(.semibold)
    public static let sectionTitle = Font.headline
    public static let cardTitle = Font.title3.weight(.semibold)
    public static let body = Font.body
    public static let metadata = Font.callout
    public static let caption = Font.caption
    public static let technical = Font.caption.monospacedDigit()
    public static let hotCueLetter = Font.system(size: 10, weight: .heavy, design: .rounded)
}
