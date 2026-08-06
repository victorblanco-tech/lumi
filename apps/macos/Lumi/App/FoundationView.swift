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
                        allowsScrolling: false,
                        showsNavigation: false,
                        onPlanMutation: { request in
                            Task { await engineStatus.mutatePlan(request) }
                        },
                        onSessionCommand: { request in
                            Task { await engineStatus.runSessionCommand(request) }
                        },
                        onLocalPlayback: { request in
                            engineStatus.runLocalPlayback(request)
                        },
                        onLibraryTrackDrop: { transfer, deckID in
                            Task {
                                await engineStatus.loadLibraryTrackOnLocalDeck(
                                    LibraryDeckLoadRequest(
                                        trackID: transfer.trackID,
                                        deckID: deckID,
                                        expectedTimelineRevision: transfer.timelineRevision
                                    )
                                )
                            }
                        },
                        localPlaybackBrowser: AnyView(
                            LocalPlaybackLibraryBrowserView(
                                state: engineStatus.libraryState,
                                keyNotation: $preferences.keyNotation,
                                feedback: engineStatus.localPlaybackFeedback,
                                feedbackIsError: engineStatus.localPlaybackFeedbackIsError,
                                onQuery: { request in
                                    Task { await engineStatus.queryLibrary(request) }
                                },
                                onLoadOnLocalDeck: { request in
                                    Task { await engineStatus.loadLibraryTrackOnLocalDeck(request) }
                                }
                            )
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
                        feedback: engineStatus.phraseRoleFeedback,
                        onMutation: { request in
                            Task { await engineStatus.mutatePhraseRoles(request) }
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
