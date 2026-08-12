import LumiDesignSystem
import LumiLibraryWorkspace
import LumiLiveWorkspace
import SwiftUI

private enum AppDestination: String, CaseIterable, Identifiable {
    case live
    case library
    case integrations
    case settings

    var id: String { rawValue }
}

struct FoundationView: View {
    @ObservedObject var engineStatus: EngineStatusModel
    @Bindable var preferences: LumiPreferences
    @State private var destination: AppDestination = .live
    @State private var librarySection: LibraryHubSection = .tracks
    @State private var navigationHovered = false
    @AppStorage(LumiPreferenceKey.navigationAutoHide)
    private var navigationAutoHides = false

    private var productVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "LumiProductVersion") as? String
            ?? "unknown"
    }

    private var navigationIsCollapsed: Bool {
        navigationAutoHides && !navigationHovered
    }

    var body: some View {
        HStack(spacing: 0) {
            navigationShell
            Divider()
            Group {
                switch destination {
                case .live:
                    LiveWorkspaceView(
                        state: engineStatus.workspaceState,
                        productVersion: productVersion,
                        appearance: $preferences.appearance,
                        keyNotation: $preferences.keyNotation,
                        lightingTimingOffsetMillis: $preferences.lightingTimingOffsetMillis,
                        allowsScrolling: false,
                        showsNavigation: false,
                        deckVisualClocks: engineStatus.deckVisualClocks,
                        localPlaybackWaveforms: engineStatus.localPlaybackWaveforms,
                        localPlaybackFeedback: engineStatus.localPlaybackFeedback,
                        localPlaybackFeedbackIsError: engineStatus.localPlaybackFeedbackIsError,
                        onPlanMutation: { request in
                            Task { await engineStatus.mutatePlan(request) }
                        },
                        onSessionCommand: { request in
                            Task { await engineStatus.runSessionCommand(request) }
                        },
                        onLocalPlayback: { request in
                            engineStatus.runLocalPlayback(request)
                        },
                        localPlaybackBrowser: AnyView(
                            LocalPlaybackLibraryBrowserView(
                                state: engineStatus.libraryState,
                                keyNotation: $preferences.keyNotation,
                                onQuery: { request in
                                    Task { await engineStatus.queryLibrary(request) }
                                },
                                onLoadOnLocalDeck: { request in
                                    Task { await engineStatus.loadLibraryTrackOnLocalDeck(request) }
                                }
                            )
                            .equatable()
                        )
                    )
                case .library:
                    LibraryHubView(
                        state: engineStatus.libraryState,
                        keyNotation: $preferences.keyNotation,
                        section: $librarySection,
                        phraseRoleFeedback: engineStatus.phraseRoleFeedback,
                        timelineFeedback: engineStatus.timelineEditFeedback,
                        localPlaybackFeedback: engineStatus.localPlaybackFeedback,
                        localPlaybackFeedbackIsError: engineStatus.localPlaybackFeedbackIsError,
                        sourceImportFeedback: engineStatus.sourceImportFeedback,
                        sourceImportFeedbackIsError: engineStatus.sourceImportFeedbackIsError,
                        usbSourceOperation: engineStatus.usbSourceOperation,
                        onQuery: { request in
                            Task { await engineStatus.queryLibrary(request) }
                        },
                        onOpenEditor: { trackID in
                            Task { await engineStatus.openLibraryTrackEditor(trackID: trackID) }
                        },
                        onTimelineEdit: { request in
                            Task { await engineStatus.editLibraryTimeline(request) }
                        },
                        onTimelineHistory: { request in
                            Task { await engineStatus.mutateLibraryTimelineHistory(request) }
                        },
                        onSourceReconcile: { request in
                            Task { await engineStatus.reconcileLibrarySource(request) }
                        },
                        onLoadOnLocalDeck: { request in
                            Task { await engineStatus.loadLibraryTrackOnLocalDeck(request) }
                        },
                        onPhraseRoleMutation: { request in
                            Task { await engineStatus.mutatePhraseRoles(request) }
                        },
                        onRekordboxSyncPreview: { request in
                            Task { await engineStatus.previewRekordboxXMLSync(request) }
                        },
                        onRekordboxSyncApply: { request, expectedContentSHA256 in
                            Task {
                                await engineStatus.applyRekordboxXMLSync(
                                    request,
                                    expectedContentSHA256: expectedContentSHA256
                                )
                            }
                        },
                        onRekordboxAnalysisImport: { request, expectedContentSHA256 in
                            Task {
                                await engineStatus.importRekordboxAnalysis(
                                    request,
                                    expectedContentSHA256: expectedContentSHA256
                                )
                            }
                        },
                        onRekordboxDeviceInspect: { root in
                            Task { await engineStatus.inspectRekordboxDevice(root: root) }
                        },
                        onRekordboxDeviceSync: { root, playlistIDs in
                            Task {
                                await engineStatus.syncRekordboxDevice(
                                    root: root,
                                    playlistIDs: playlistIDs
                                )
                            }
                        }
                    )
                case .integrations:
                    IntegrationsWorkspaceView(
                        library: engineStatus.libraryState,
                        autoloopFeedback: engineStatus.autoloopCatalogFeedback,
                        midiIntegrationFeedback: engineStatus.midiIntegrationFeedback,
                        onOpenLibrarySources: {
                            librarySection = .sources
                            destination = .library
                        },
                        onAutoloopMutation: { request in
                            Task { await engineStatus.mutateAutoloopCatalog(request) }
                        },
                        onPublishMidi: {
                            Task { await engineStatus.publishMidiSource() }
                        },
                        onStopMidi: {
                            Task { await engineStatus.stopMidiSource() }
                        },
                        onTestAbletonLinkHelper: {
                            Task { await engineStatus.testAbletonLinkHelper() }
                        },
                        onSendMidiAddressLearnPulse: { targetKind, targetNumber in
                            Task {
                                await engineStatus.sendMidiAddressLearnPulse(
                                    targetKind: targetKind,
                                    targetNumber: targetNumber
                                )
                            }
                        },
                        onTriggerMidiAutoloop: { bankNumber, autoloopNumber in
                            Task {
                                await engineStatus.triggerMidiAutoloop(
                                    bankNumber: bankNumber,
                                    autoloopNumber: autoloopNumber
                                )
                            }
                        }
                    )
                case .settings:
                    PhraseRoleSettingsView(
                        settings: engineStatus.libraryState.phraseRoleSettings,
                        appearance: $preferences.appearance,
                        keyNotation: $preferences.keyNotation,
                        lightingTimingOffsetMillis: $preferences.lightingTimingOffsetMillis,
                        feedback: engineStatus.phraseRoleFeedback,
                        dataManagement: engineStatus.libraryState.dataManagement,
                        dataOperation: engineStatus.dataManagementOperation,
                        backups: engineStatus.backupRecords,
                        canManageData: engineStatus.canManageData,
                        onMutation: { request in
                            Task { await engineStatus.mutatePhraseRoles(request) }
                        },
                        onCreateBackup: {
                            Task { await engineStatus.createFullBackup() }
                        },
                        onPrepareReset: { trackIDs in
                            Task {
                                await engineStatus.prepareLibraryReset(
                                    preserveTrackIDs: trackIDs
                                )
                            }
                        },
                        onApplyReset: {
                            Task { await engineStatus.applyPreparedLibraryReset() }
                        },
                        onRestoreBackup: { path in
                            Task { await engineStatus.restoreBackup(path: path) }
                        }
                    )
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .background(LumiColor.canvas)
        .tint(LumiColor.accent)
        .frame(minWidth: 1_180, minHeight: 620)
        .accessibilityIdentifier("lumi.app.shell")
    }

    private var navigationShell: some View {
        ZStack(alignment: .leading) {
            appNavigation
                .opacity(navigationIsCollapsed ? 0 : 1)
                .offset(x: navigationIsCollapsed ? -8 : 0)
                .allowsHitTesting(!navigationIsCollapsed)
                .accessibilityHidden(navigationIsCollapsed)
            collapsedNavigation
                .opacity(navigationIsCollapsed ? 1 : 0)
                .offset(x: navigationIsCollapsed ? 0 : 6)
                .allowsHitTesting(navigationIsCollapsed)
                .accessibilityHidden(!navigationIsCollapsed)
        }
        .frame(width: navigationIsCollapsed ? 52 : 196, alignment: .leading)
        .clipped()
        .onHover { navigationHovered = $0 }
        .animation(.easeInOut(duration: 0.24), value: navigationIsCollapsed)
    }

    private var appNavigation: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.xLarge) {
            VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                HStack(alignment: .center, spacing: LumiSpacing.xSmall) {
                    Image("LumiWordmark")
                        .resizable()
                        .interpolation(.high)
                        .scaledToFit()
                        .frame(width: 126, height: 36, alignment: .leading)
                        .accessibilityHidden(true)
                        .accessibilityIdentifier("lumi.navigation.brandWordmark")
                    Spacer(minLength: 0)
                    Button {
                        navigationAutoHides.toggle()
                    } label: {
                        Image(systemName: "sidebar.left")
                            .frame(width: 28, height: 28)
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(navigationAutoHides ? LumiColor.accent : LumiColor.textSecondary)
                    .help(navigationAutoHides ? "Keep navigation visible" : "Auto-hide navigation")
                    .accessibilityLabel(
                        navigationAutoHides ? "Keep navigation visible" : "Auto-hide navigation"
                    )
                    .accessibilityIdentifier("lumi.navigation.autoHide")
                }
                Text(productVersion)
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
                    .accessibilityIdentifier("lumi.navigation.version")
            }
            .accessibilityElement(children: .contain)
            .accessibilityLabel("Lumi \(productVersion)")
            VStack(spacing: LumiSpacing.xSmall) {
                destinationButton(.live, title: "Live", systemImage: "waveform")
                destinationButton(.library, title: "Library", systemImage: "music.note.list")
                unavailableNavigation("Plans", systemImage: "list.bullet.rectangle")
                destinationButton(.integrations, title: "Integrations", systemImage: "cable.connector")
            }
            Spacer()
            destinationButton(.settings, title: "Settings", systemImage: "gearshape")
        }
        .padding(LumiSpacing.large)
        .frame(width: 196)
        .background(LumiColor.surface)
        .accessibilityIdentifier("lumi.navigation")
    }

    private var collapsedNavigation: some View {
        VStack(spacing: LumiSpacing.medium) {
            Button {
                navigationAutoHides = false
            } label: {
                Image("LumiMark")
                    .resizable()
                    .interpolation(.high)
                    .frame(width: 36, height: 36)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Lumi · Show navigation")
            .accessibilityLabel("Lumi, show navigation")
            .accessibilityIdentifier("lumi.navigation.brandMark")
            compactDestinationButton(.live, systemImage: "waveform", title: "Live")
            compactDestinationButton(.library, systemImage: "music.note.list", title: "Library")
            Image(systemName: "list.bullet.rectangle")
                .foregroundStyle(LumiColor.textSecondary)
                .frame(width: 32, height: 32)
                .accessibilityLabel("Plans, coming soon")
            compactDestinationButton(
                .integrations,
                systemImage: "cable.connector",
                title: "Integrations"
            )
            Spacer()
            compactDestinationButton(.settings, systemImage: "gearshape", title: "Settings")
        }
        .padding(.vertical, LumiSpacing.large)
        .frame(width: 52)
        .background(LumiColor.surface)
        .accessibilityIdentifier("lumi.navigation.collapsed")
    }

    private func compactDestinationButton(
        _ value: AppDestination,
        systemImage: String,
        title: String
    ) -> some View {
        Button {
            destination = value
        } label: {
            Image(systemName: systemImage)
                .frame(width: 32, height: 32)
                .foregroundStyle(destination == value ? LumiColor.accent : LumiColor.textPrimary)
                .background(destination == value ? LumiColor.accent.opacity(0.14) : Color.clear)
                .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        }
        .buttonStyle(.plain)
        .help(title)
        .accessibilityLabel(title)
        .accessibilityIdentifier("lumi.navigation.compact.\(value.rawValue)")
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
                .contentShape(Rectangle())
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
