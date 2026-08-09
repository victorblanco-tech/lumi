import Foundation

public enum LibraryWorkspaceLocalization {
    public static func value(_ key: String) -> String {
        String(localized: String.LocalizationValue(key), bundle: .module)
    }
}
