import SwiftUI

public struct LumiPanel<Content: View>: View {
    private let content: Content

    public init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    public var body: some View {
        content
            .padding(LumiSpacing.large)
            .background(LumiColor.surface)
            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.panel))
            .overlay {
                RoundedRectangle(cornerRadius: LumiRadius.panel)
                    .stroke(LumiColor.border, lineWidth: 1)
            }
    }
}
