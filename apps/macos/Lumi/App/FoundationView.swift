import LumiDesignSystem
import LumiLibraryWorkspace
import LumiLiveWorkspace
import SwiftUI

private enum AppDestination: String, CaseIterable, Identifiable {
    case live
    case library

    var id: String { rawValue }
}

struct FoundationView: View {
    @ObservedObject var engineStatus: EngineStatusModel
    @Bindable var preferences: LumiPreferences
    @State private var destination: AppDestination = .live

    private var productVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "LumiProductVersion") as? String
            ?? "unknown"
    }

    var body: some View {
        HStack(spacing: 0) {
            appNavigation
            Divider()
            Group {
                switch destination {
                case .live:
                    LiveWorkspaceView(
                        state: engineStatus.workspaceState,
                        productVersion: productVersion,
                        appearance: $preferences.appearance,
                        keyNotation: $preferences.keyNotation,
                        showsNavigation: false,
                        onPlanMutation: { request in
                            Task { await engineStatus.mutatePlan(request) }
                        },
                        onSessionCommand: { request in
                            Task { await engineStatus.runSessionCommand(request) }
                        }
                    )
                case .library:
                    LibraryWorkspaceView(
                        state: engineStatus.libraryState,
                        keyNotation: $preferences.keyNotation,
                        onQuery: { request in
                            Task { await engineStatus.queryLibrary(request) }
                        },
                        onOpenEditor: { trackID in
                            Task { await engineStatus.openLibraryTrackEditor(trackID: trackID) }
                        },
                        onCloseEditor: {
                            Task { await engineStatus.closeLibraryTrackEditor() }
                        },
                        onTimelineEdit: { request in
                            Task { await engineStatus.editLibraryTimeline(request) }
                        },
                        onTimelineHistory: { request in
                            Task { await engineStatus.mutateLibraryTimelineHistory(request) }
                        },
                        timelineFeedback: engineStatus.timelineEditFeedback
                    )
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .background(LumiColor.canvas)
        .frame(minWidth: 1_180, minHeight: 620)
        .accessibilityIdentifier("lumi.app.shell")
    }

    private var appNavigation: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.xLarge) {
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                Text(verbatim: "Lumi")
                    .font(LumiTypography.screenTitle)
                    .foregroundStyle(LumiColor.textPrimary)
                Text(productVersion)
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            VStack(spacing: LumiSpacing.xSmall) {
                destinationButton(.live, title: "Live", systemImage: "waveform")
                destinationButton(.library, title: "Library", systemImage: "music.note.list")
                unavailableNavigation("Plans", systemImage: "list.bullet.rectangle")
                unavailableNavigation("Integrations", systemImage: "cable.connector")
            }
            Spacer()
            Menu {
                Picker("Appearance", selection: $preferences.appearance) {
                    ForEach(AppearancePreference.allCases) { preference in
                        Text(preference.titleKey).tag(preference)
                    }
                }
                Picker("Key notation", selection: $preferences.keyNotation) {
                    ForEach(KeyNotationPreference.allCases) { preference in
                        Text(preference.titleKey).tag(preference)
                    }
                }
            } label: {
                Label("Settings", systemImage: "gearshape")
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .frame(height: LumiControlMetric.standardHeight)
            }
            .menuStyle(.borderlessButton)
            .accessibilityIdentifier("lumi.navigation.settings")
        }
        .padding(LumiSpacing.large)
        .frame(width: 196)
        .background(LumiColor.surface)
        .accessibilityIdentifier("lumi.navigation")
    }

    private func destinationButton(
        _ value: AppDestination,
        title: String,
        systemImage: String
    ) -> some View {
        Button {
            destination = value
        } label: {
            Label(title, systemImage: systemImage)
                .frame(maxWidth: .infinity, alignment: .leading)
                .frame(height: LumiControlMetric.standardHeight)
                .padding(.horizontal, LumiSpacing.small)
                .foregroundStyle(destination == value ? LumiColor.accent : LumiColor.textPrimary)
                .background(destination == value ? LumiColor.accent.opacity(0.14) : Color.clear)
                .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("lumi.navigation.\(value.rawValue)")
    }

    private func unavailableNavigation(_ title: String, systemImage: String) -> some View {
        Label(title, systemImage: systemImage)
            .frame(maxWidth: .infinity, alignment: .leading)
            .frame(height: LumiControlMetric.standardHeight)
            .padding(.horizontal, LumiSpacing.small)
            .foregroundStyle(LumiColor.textSecondary)
            .accessibilityLabel("\(title), coming soon")
    }
}

#Preview("Library") {
    FoundationView(
        engineStatus: EngineStatusModel(),
        preferences: LumiPreferences()
    )
    .preferredColorScheme(.dark)
    .frame(width: 1_280, height: 820)
}
