import LumiDesignSystem

public extension PhraseRoleSettingsState {
    var colorPalette: LumiPhraseColorPalette {
        LumiPhraseColorPalette(
            roleColors: Dictionary(uniqueKeysWithValues: roles.map { ($0.id, $0.colorRGB) })
        )
    }
}
