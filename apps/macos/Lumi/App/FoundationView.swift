import LumiDesignSystem
import LumiLibraryWorkspace
import LumiLiveWorkspace
import SwiftUI

private enum AppDestination: String, CaseIterable, Identifiable {
    case live
    case library
    case plans
    case integrations
    case settings

    var id: String { rawValue }

    var keyboardKey: KeyEquivalent {
        switch self {
        case .live: return "1"
        case .library: return "2"
        case .plans: return "3"
        case .integrations: return "4"
        case .settings: return ","
        }
    }
}

struct FoundationView: View {
    @ObservedObject var engineStatus: EngineStatusModel
    @Bindable var preferences: LumiPreferences
    @State private var destination: AppDestination = .live
    @State private var librarySection: LibraryHubSection = .tracks
    @AppStorage(LumiPreferenceKey.navigationHidden)
    private var navigationIsHidden = false

    private var productVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "LumiProductVersion") as? String
            ?? "unknown"
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
                        lightingTimingOffsetMillis: lightingTimingBinding,
                        allowsScrolling: false,
                        showsNavigation: false,
                        deckVisualClocks: engineStatus.deckVisualClocks,
                        localPlaybackWaveforms: engineStatus.localPlaybackWaveforms,
                        localPlaybackFeedback: engineStatus.localPlaybackFeedback,
                        localPlaybackFeedbackIsError: engineStatus.localPlaybackFeedbackIsError,
                        phraseColorPalette: engineStatus.libraryState.phraseRoleSettings?.colorPalette ?? .defaults,
                        onPlanMutation: { request in
                            Task { await engineStatus.mutatePlan(request) }
                        },
                        onSessionCommand: { request in
                            Task { await engineStatus.runSessionCommand(request) }
                        },
                        onLocalPlayback: { request in
                            engineStatus.runLocalPlayback(request)
                        },
                        onSetAbletonLinkEnabled: { enabled in
                            Task { await engineStatus.setAbletonLinkEnabled(enabled) }
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
                        trackWorkflowFeedback: engineStatus.trackWorkflowFeedback,
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
                        onReuseTimeline: { request in
                            Task { await engineStatus.reuseLibraryTimeline(request) }
                        },
                        onTrackWorkflowMutation: { request in
                            Task { await engineStatus.mutateTrackWorkflow(request) }
                        },
                        onLoadOnLocalDeck: { request in
                            Task { await engineStatus.loadLibraryTrackOnLocalDeck(request) }
                        },
                        onPhraseRoleMutation: { request in
                            Task { await engineStatus.mutatePhraseRoles(request) }
                        },
                        onRekordboxDeviceInspect: { root, sourceID in
                            Task {
                                await engineStatus.inspectRekordboxDevice(
                                    root: root,
                                    sourceID: sourceID
                                )
                            }
                        },
                        onRekordboxDeviceSync: { root, sourceID, playlistIDs in
                            Task {
                                await engineStatus.syncRekordboxDevice(
                                    root: root,
                                    sourceID: sourceID,
                                    playlistIDs: playlistIDs
                                )
                            }
                        },
                        onRekordboxDeviceConflictResolution: { request in
                            Task { await engineStatus.resolveUSBConflict(request) }
                        }
                    )
                case .plans:
                    LightPlansWorkspaceView(
                        state: engineStatus.lightPlanningState,
                        library: engineStatus.libraryState,
                        feedback: engineStatus.lightPlanningFeedback,
                        onSave: { policy in
                            Task { await engineStatus.replaceLightPlanningPolicy(policy) }
                        },
                        onOpenTrack: { trackID in
                            Task { await engineStatus.openLibraryTrackEditor(trackID: trackID) }
                        },
                        onPreview: { trackID, timelineRevision, themeID, seed, policy in
                            Task {
                                await engineStatus.previewLightPlan(
                                    trackID: trackID,
                                    expectedTimelineRevision: timelineRevision,
                                    themeID: themeID,
                                    variationSeed: seed,
                                    policy: policy
                                )
                            }
                        },
                        onOpenLightingOutputs: {
                            destination = .integrations
                        },
                        onSendModifierLearnPulse: { channel, note in
                            Task {
                                await engineStatus.sendCustomMidiLearnPulse(
                                    channel: channel,
                                    note: note
                                )
                            }
                        }
                    )
                case .integrations:
                    IntegrationsWorkspaceView(
                        library: engineStatus.libraryState,
                        autoloopFeedback: engineStatus.autoloopCatalogFeedback,
                        midiIntegrationFeedback: engineStatus.midiIntegrationFeedback,
                        abletonLinkFeedback: engineStatus.abletonLinkFeedback,
                        remoteGateway: engineStatus.remoteGatewayState,
                        lightPlanningPolicy: engineStatus.lightPlanningState.policy,
                        lightPlanningFeedback: engineStatus.lightPlanningFeedback,
                        abletonLinkAutoStart: $preferences.abletonLinkAutoStart,
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
                        onSetAbletonLinkEnabled: { enabled in
                            Task { await engineStatus.setAbletonLinkEnabled(enabled) }
                        },
                        onTestAbletonLinkHelper: {
                            Task { await engineStatus.testAbletonLinkHelper() }
                        },
                        onSetRemoteGatewayEnabled: { enabled in
                            Task { await engineStatus.setRemoteGatewayEnabled(enabled) }
                        },
                        onRefreshRemoteGateway: {
                            Task { await engineStatus.refreshRemoteGateway() }
                        },
                        onCreateRemoteInvitation: {
                            Task { await engineStatus.createRemotePairingInvitation() }
                        },
                        onApproveRemoteInvitation: { invitationID, shortCode in
                            Task {
                                await engineStatus.approveRemotePairing(
                                    invitationID: invitationID,
                                    shortCode: shortCode
                                )
                            }
                        },
                        onRevokeRemoteDevice: { deviceID in
                            Task { await engineStatus.revokeRemoteDevice(deviceID: deviceID) }
                        },
                        onTransferRemoteControl: { deviceID in
                            Task { await engineStatus.transferRemoteControl(to: deviceID) }
                        },
                        onSendMidiAddressLearnPulse: { targetKind, targetNumber, bankNumber in
                            Task {
                                await engineStatus.sendMidiAddressLearnPulse(
                                    targetKind: targetKind,
                                    targetNumber: targetNumber,
                                    bankNumber: bankNumber
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
                        },
                        onSaveLightPlanningPolicy: { policy in
                            Task { await engineStatus.replaceLightPlanningPolicy(policy) }
                        },
                        onToggleMidiStaticLook: { slotNumber in
                            Task { await engineStatus.toggleMidiStaticLook(slotNumber: slotNumber) }
                        }
                    )
                case .settings:
                    PhraseRoleSettingsView(
                        settings: engineStatus.libraryState.phraseRoleSettings,
                        appearance: $preferences.appearance,
                        keyNotation: $preferences.keyNotation,
                        lightingTimingOffsetMillis: lightingTimingBinding,
                        feedback: engineStatus.phraseRoleFeedback,
                        workflowCatalog: engineStatus.libraryState.workflowCatalog,
                        workflowFeedback: engineStatus.trackWorkflowFeedback,
                        dataManagement: engineStatus.libraryState.dataManagement,
                        dataOperation: engineStatus.dataManagementOperation,
                        backups: engineStatus.backupRecords,
                        canManageData: engineStatus.canManageData,
                        onMutation: { request in
                            Task { await engineStatus.mutatePhraseRoles(request) }
                        },
                        onWorkflowMutation: { request in
                            Task { await engineStatus.mutateTrackWorkflow(request) }
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
        .accessibilityIdentifier("lumi.app.shell")
    }

    private var lightingTimingBinding: Binding<Int> {
        Binding(
            get: {
                let timing = engineStatus.lightingTimingSettings
                return timing?.pendingTimingOffsetMillis ?? timing?.timingOffsetMillis
                    ?? preferences.lightingTimingOffsetMillis
            },
            set: { millis in
                Task { await engineStatus.setLightingTimingOffset(millis) }
            }
        )
    }

    private var navigationShell: some View {
        ZStack(alignment: .leading) {
            appNavigation
                .opacity(navigationIsHidden ? 0 : 1)
                .offset(x: navigationIsHidden ? -8 : 0)
                .allowsHitTesting(!navigationIsHidden)
                .accessibilityHidden(navigationIsHidden)
            collapsedNavigation
                .opacity(navigationIsHidden ? 1 : 0)
                .offset(x: navigationIsHidden ? 0 : 6)
                .allowsHitTesting(navigationIsHidden)
                .accessibilityHidden(!navigationIsHidden)
        }
        .frame(width: navigationIsHidden ? 52 : 196, alignment: .leading)
        .clipped()
        .animation(.easeInOut(duration: 0.24), value: navigationIsHidden)
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
                        navigationIsHidden = true
                    } label: {
                        Image(systemName: "sidebar.left")
                            .frame(width: 28, height: 28)
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(LumiColor.textSecondary)
                    .help("Hide navigation")
                    .accessibilityLabel("Hide navigation")
                    .accessibilityIdentifier("lumi.navigation.hide")
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
                destinationButton(.plans, title: "Light Plans", systemImage: "list.bullet.rectangle")
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
                navigationIsHidden = false
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
            compactDestinationButton(
                .plans,
                systemImage: "list.bullet.rectangle",
                title: "Light Plans"
            )
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
        .keyboardShortcut(value.keyboardKey, modifiers: .command)
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
        .keyboardShortcut(value.keyboardKey, modifiers: .command)
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
