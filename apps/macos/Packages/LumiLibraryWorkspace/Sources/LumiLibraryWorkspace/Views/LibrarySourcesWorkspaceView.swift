import AppKit
import Foundation
import LumiDesignSystem
import SwiftUI

public struct LibrarySourcesWorkspaceView: View {
    private let library: LibraryWorkspaceState
    private let settings: PhraseRoleSettingsState?
    private let feedback: String?
    private let syncFeedback: String?
    private let syncFeedbackIsError: Bool
    private let rendersInteractiveControls: Bool
    private let onMutation: @Sendable (PhraseRoleMutationRequest) -> Void
    private let onSyncPreview: @Sendable (RekordboxXMLSyncPreviewRequest) -> Void
    private let onSyncApply: @Sendable (RekordboxXMLSyncPreviewRequest, String) -> Void

    @AppStorage("nl.blancoservices.lumi.rekordboxXML.folder")
    private var rekordboxFolderPath = ""
    @AppStorage("nl.blancoservices.lumi.rekordboxXML.includeFutureChildren")
    private var includeFutureChildPlaylists = true
    @AppStorage("nl.blancoservices.lumi.rekordboxXML.followedPaths")
    private var followedPathsJSON = "[]"

    @State private var selectedProviderKind: String?
    @State private var followedPaths: Set<String> = []
    @State private var discovery: RekordboxXMLDiscoveryState?
    @State private var availableExportCount = 0
    @State private var sourceError: String?
    @State private var isScanning = false
    @State private var didInitializeSource = false
    @State private var isPlaylistSelectionExpanded = false
    @State private var isPhraseMappingExpanded = false

    public init(
        library: LibraryWorkspaceState,
        settings: PhraseRoleSettingsState?,
        feedback: String? = nil,
        syncFeedback: String? = nil,
        syncFeedbackIsError: Bool = false,
        rendersInteractiveControls: Bool = true,
        onMutation: @escaping @Sendable (PhraseRoleMutationRequest) -> Void = { _ in },
        onSyncPreview: @escaping @Sendable (RekordboxXMLSyncPreviewRequest) -> Void = { _ in },
        onSyncApply: @escaping @Sendable (RekordboxXMLSyncPreviewRequest, String) -> Void = { _, _ in }
    ) {
        self.library = library
        self.settings = settings
        self.feedback = feedback
        self.syncFeedback = syncFeedback
        self.syncFeedbackIsError = syncFeedbackIsError
        self.rendersInteractiveControls = rendersInteractiveControls
        self.onMutation = onMutation
        self.onSyncPreview = onSyncPreview
        self.onSyncApply = onSyncApply
        _selectedProviderKind = State(initialValue: settings?.mappingProfiles.first?.providerKind)
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: LumiSpacing.xLarge) {
                header
                rekordboxSource
                rekordboxSourceSettings
                rekordboxPlaylistSelection
                rekordboxSyncPreview
                appliedRekordboxMirror
                activeSource
                sourceMappings
            }
            .padding(LumiSpacing.xLarge)
            .frame(maxWidth: 980, alignment: .leading)
        }
        .background(LumiColor.canvas)
        .accessibilityIdentifier("lumi.library.sources")
        .onChange(of: settings?.revision) { _, _ in synchronizeProvider() }
        .onAppear {
            guard !didInitializeSource else { return }
            didInitializeSource = true
            restoreFollowedPaths()
            if !rekordboxFolderPath.isEmpty { scanImportFolder() }
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
            Text("Sources & Import")
                .font(LumiTypography.screenTitle)
            Text("Connect local music-library sources, inspect import state and configure source-specific initial phrase mapping.")
                .font(LumiTypography.body)
                .foregroundStyle(LumiColor.textSecondary)
        }
    }

    private var rekordboxSource: some View {
        LumiPanel {
            HStack(alignment: .top, spacing: LumiSpacing.large) {
                sourceIcon("r.square.fill", state: .empty)
                VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                    HStack {
                        Text("Rekordbox XML")
                            .font(LumiTypography.cardTitle)
                        Text("LOCAL · READ ONLY")
                            .font(LumiTypography.technical)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(LumiColor.surfaceElevated)
                            .clipShape(Capsule())
                    }
                    Text(rekordboxSourceStatus)
                        .font(LumiTypography.technical)
                        .foregroundStyle(sourceError == nil ? LumiColor.textSecondary : LumiColor.warning)
                    Text("Lumi watches a folder you choose and reads its newest XML export. The source files are never modified or deleted.")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                    if let sourceError {
                        Label(sourceError, systemImage: "exclamationmark.triangle.fill")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.warning)
                    }
                }
                Spacer()
                VStack(alignment: .trailing, spacing: LumiSpacing.small) {
                    Button(rekordboxFolderPath.isEmpty ? "Choose Import Folder…" : "Change Folder…") {
                        chooseImportFolder()
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(!rendersInteractiveControls)
                    .accessibilityIdentifier("lumi.library.sources.rekordbox.chooseFolder")
                    Button(isScanning ? "Scanning…" : "Scan Now") {
                        scanImportFolder()
                    }
                    .buttonStyle(.bordered)
                    .disabled(rekordboxFolderPath.isEmpty || isScanning || !rendersInteractiveControls)
                    .accessibilityIdentifier("lumi.library.sources.rekordbox.scan")
                }
            }
        }
    }

    @ViewBuilder
    private var rekordboxSourceSettings: some View {
        if !rekordboxFolderPath.isEmpty {
            VStack(alignment: .leading, spacing: LumiSpacing.large) {
                VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                    Text("Rekordbox Source Settings")
                        .font(LumiTypography.cardTitle)
                    Text("These settings belong to this source and define how followed playlist folders behave during future syncs.")
                        .font(LumiTypography.body)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                LumiPanel {
                    VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                        sourceSettingRow(
                            title: "XML import folder",
                            detail: rekordboxFolderPath,
                            systemImage: "folder.fill"
                        )
                        Divider()
                        Toggle(isOn: $includeFutureChildPlaylists) {
                            VStack(alignment: .leading, spacing: 3) {
                                Text("Include future playlists inside followed folders")
                                    .font(LumiTypography.body.weight(.semibold))
                                Text("When you add a playlist to a followed Rekordbox folder, Lumi will include it on the next sync preview.")
                                    .font(LumiTypography.caption)
                                    .foregroundStyle(LumiColor.textSecondary)
                            }
                        }
                        .toggleStyle(.switch)
                        .disabled(!rendersInteractiveControls)
                        Divider()
                        sourceSettingRow(
                            title: "Mirror playlist membership",
                            detail: "Enabled · additions and removals are included in the sync preview",
                            systemImage: "arrow.triangle.2.circlepath"
                        )
                        Divider()
                        sourceSettingRow(
                            title: "Archive removed tracks",
                            detail: "Required · Lumi-owned phrases and plans remain recoverable",
                            systemImage: "archivebox.fill"
                        )
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var rekordboxPlaylistSelection: some View {
        if let discovery {
            DisclosureGroup(isExpanded: $isPlaylistSelectionExpanded) {
                LumiPanel {
                    VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                        HStack {
                            VStack(alignment: .leading, spacing: 3) {
                                Text(discovery.export.fileName)
                                    .font(LumiTypography.body.weight(.semibold))
                                Text(exportDetails(discovery))
                                    .font(LumiTypography.technical)
                                    .foregroundStyle(LumiColor.textSecondary)
                            }
                            Spacer()
                            StatusBadge("READ ONLY", state: .ready)
                        }
                        Divider()
                        VStack(alignment: .leading, spacing: 4) {
                            ForEach(discovery.roots) { node in
                                RekordboxPlaylistTreeRow(
                                    node: node,
                                    followedPaths: $followedPaths,
                                    isInteractive: rendersInteractiveControls,
                                    onChange: persistFollowedPaths
                                )
                            }
                        }
                        Divider()
                        HStack {
                            Label(selectionSummary(discovery), systemImage: "checkmark.circle.fill")
                                .font(LumiTypography.caption)
                                .foregroundStyle(followedPaths.isEmpty ? LumiColor.textSecondary : LumiColor.accent)
                            Spacer()
                            Button("Preview Sync") {
                                onSyncPreview(
                                    RekordboxXMLSyncPreviewRequest(
                                        folderPath: rekordboxFolderPath,
                                        followedPaths: followedPaths.sorted(),
                                        includeFutureChildPlaylists: includeFutureChildPlaylists
                                    )
                                )
                            }
                                .buttonStyle(.borderedProminent)
                                .disabled(
                                    followedPaths.isEmpty
                                        || isScanning
                                        || !rendersInteractiveControls
                                )
                                .help("Read the selected playlists and calculate a mirror preview without changing the Lumi library")
                                .accessibilityIdentifier("lumi.library.sources.rekordbox.previewSync")
                        }
                    }
                }
                .padding(.top, LumiSpacing.medium)
            } label: {
                HStack(alignment: .center, spacing: LumiSpacing.medium) {
                    VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                        Text("Follow Playlists")
                            .font(LumiTypography.cardTitle)
                        Text(selectionSummary(discovery))
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                    Spacer()
                    Text("\(discovery.folderCount) folders · \(discovery.playlistCount) playlists")
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                }
            }
            .tint(LumiColor.accent)
            .accessibilityIdentifier("lumi.library.sources.rekordbox.playlistsDisclosure")
        }
    }

    @ViewBuilder
    private var rekordboxSyncPreview: some View {
        if !syncFeedbackIsError,
           let preview = library.rekordboxSyncPreview,
           previewMatchesCurrentConfiguration(preview) {
            VStack(alignment: .leading, spacing: LumiSpacing.large) {
                HStack(alignment: .bottom) {
                    VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                        Text("Sync Preview")
                            .font(LumiTypography.cardTitle)
                        Text("Engine-validated mirror scope. This preview has not written tracks, playlists, phrases or analysis to Lumi.")
                            .font(LumiTypography.body)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                    Spacer()
                    StatusBadge(preview.applyState == "applied" ? "APPLIED" : "READY", state: .ready)
                }
                LumiPanel {
                    VStack(alignment: .leading, spacing: LumiSpacing.large) {
                        HStack(spacing: LumiSpacing.medium) {
                            previewMetric(
                                "Playlists",
                                value: preview.followedPlaylistCount,
                                systemImage: "music.note.list"
                            )
                            previewMetric(
                                "Unique tracks",
                                value: preview.uniqueTrackCount,
                                systemImage: "music.note"
                            )
                            previewMetric(
                                "Collection tracks",
                                value: preview.collectionTrackCount,
                                systemImage: "shippingbox.fill"
                            )
                        }
                        Divider()
                        HStack(spacing: LumiSpacing.medium) {
                            previewMetric("Add", value: preview.diff.inserted, systemImage: "plus.circle.fill")
                            previewMetric("Update", value: preview.diff.updated, systemImage: "arrow.clockwise.circle.fill")
                            previewMetric("Archive", value: preview.diff.archived, systemImage: "archivebox.fill")
                            previewMetric("Restore", value: preview.diff.restored, systemImage: "arrow.uturn.backward.circle.fill")
                        }
                        Divider()
                        VStack(alignment: .leading, spacing: LumiSpacing.small) {
                            Text("Selected mirror")
                                .font(LumiTypography.caption.weight(.semibold))
                                .foregroundStyle(LumiColor.textSecondary)
                            ForEach(preview.playlists.prefix(8)) { playlist in
                                HStack {
                                    Label(playlist.path, systemImage: "music.note.list")
                                        .lineLimit(1)
                                    Spacer()
                                    Text("\(playlist.trackCount) tracks")
                                        .font(LumiTypography.technical)
                                        .foregroundStyle(LumiColor.textSecondary)
                                }
                                .font(LumiTypography.body)
                            }
                            if preview.playlists.count > 8 {
                                Text("+ \(preview.playlists.count - 8) more playlists")
                                    .font(LumiTypography.caption)
                                    .foregroundStyle(LumiColor.textSecondary)
                            }
                        }
                        Divider()
                        VStack(alignment: .leading, spacing: LumiSpacing.small) {
                            Text("Source analysis")
                                .font(LumiTypography.caption.weight(.semibold))
                                .foregroundStyle(LumiColor.textSecondary)
                            HStack(spacing: LumiSpacing.large) {
                                diagnosticLabel(
                                    "Duplicate references merged",
                                    preview.diagnostics.duplicatePlaylistReferences
                                )
                                diagnosticLabel("No beat grid", preview.diagnostics.missingBeatGrid)
                                diagnosticLabel("No key", preview.diagnostics.missingKey)
                                diagnosticLabel("No duration", preview.diagnostics.missingDuration)
                                diagnosticLabel("No colour", preview.diagnostics.missingColour)
                            }
                            Label(
                                "Rekordbox XML does not contain RGB waveform or phrase data. Lumi will keep those capabilities explicitly missing until a later analysis source provides them.",
                                systemImage: "info.circle.fill"
                            )
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.textSecondary)
                        }
                        Divider()
                        HStack {
                            VStack(alignment: .leading, spacing: 3) {
                                Text(preview.exportFileName)
                                    .font(LumiTypography.body.weight(.semibold))
                                Text("SHA-256 \(preview.contentSHA256.prefix(12))… · Rekordbox \(preview.productVersion)")
                                    .font(LumiTypography.technical)
                                    .foregroundStyle(LumiColor.textSecondary)
                            }
                            Spacer()
                            Button(preview.applyState == "applied" ? "Applied" : "Apply Sync") {
                                onSyncApply(
                                    RekordboxXMLSyncPreviewRequest(
                                        folderPath: rekordboxFolderPath,
                                        followedPaths: followedPaths.sorted(),
                                        includeFutureChildPlaylists: includeFutureChildPlaylists
                                    ),
                                    preview.contentSHA256
                                )
                            }
                                .buttonStyle(.borderedProminent)
                                .disabled(
                                    preview.applyState == "applied"
                                        || !rendersInteractiveControls
                                )
                                .help("Atomically mirror this exact SHA-256-bound preview; missing tracks are archived, never deleted")
                                .accessibilityLabel(
                                    preview.applyState == "applied" ? "Applied" : "Apply Sync"
                                )
                                .accessibilityIdentifier("lumi.library.sources.rekordbox.applySync")
                        }
                    }
                }
                if let syncFeedback {
                    Label(syncFeedback, systemImage: "checkmark.shield.fill")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.success)
                }
            }
            .accessibilityIdentifier("lumi.library.sources.rekordbox.syncPreview")
        } else if let syncFeedback {
            Label(syncFeedback, systemImage: "exclamationmark.triangle.fill")
                .font(LumiTypography.caption)
                .foregroundStyle(LumiColor.warning)
        }
    }

    @ViewBuilder
    private var appliedRekordboxMirror: some View {
        if let mirror = library.rekordboxMirror {
            LumiPanel {
                HStack(alignment: .top, spacing: LumiSpacing.large) {
                    sourceIcon("archivebox.fill", state: .ready)
                    VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                        HStack {
                            Text("Rekordbox Mirror")
                                .font(LumiTypography.cardTitle)
                            StatusBadge("PERSISTED", state: .ready)
                        }
                        Text("\(mirror.activeTracks) active tracks · \(mirror.archivedTracks) archived · \(mirror.playlists) playlists")
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.textSecondary)
                        Text("Metadata and playlist membership are stored safely. Beatgrid, waveform and phrases remain analysis pending; no placeholder analysis was created.")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                    Spacer()
                    StatusBadge("ANALYSIS PENDING", state: .degraded)
                }
            }
        }
    }

    private func previewMetric(
        _ title: String,
        value: UInt64,
        systemImage: String
    ) -> some View {
        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
            Label(title, systemImage: systemImage)
                .font(LumiTypography.caption)
                .foregroundStyle(LumiColor.textSecondary)
            Text(value.formatted())
                .font(LumiTypography.cardTitle.monospacedDigit())
                .foregroundStyle(LumiColor.textPrimary)
        }
        .padding(LumiSpacing.medium)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(LumiColor.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
    }

    private func diagnosticLabel(_ title: String, _ count: UInt64) -> some View {
        Label("\(title): \(count)", systemImage: count == 0 ? "checkmark.circle.fill" : "exclamationmark.circle.fill")
            .font(LumiTypography.technical)
            .foregroundStyle(count == 0 ? LumiColor.success : LumiColor.warning)
    }

    private func previewMatchesCurrentConfiguration(_ preview: RekordboxXMLSyncPreview) -> Bool {
        preview.exportFileName == discovery?.export.fileName
            && Set(preview.selectionPaths) == followedPaths
            && preview.includeFutureChildPlaylists == includeFutureChildPlaylists
    }

    @ViewBuilder
    private var activeSource: some View {
        if let source = library.source {
            LumiPanel {
                HStack(alignment: .top, spacing: LumiSpacing.large) {
                    sourceIcon("shippingbox.fill", state: library.condition.componentState)
                    VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                        HStack {
                            Text(source.name).font(LumiTypography.cardTitle)
                            StatusBadge("ACTIVE", state: library.condition.componentState)
                        }
                        Text("\(library.collectionTotal) tracks · revision \(source.revision)")
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.textSecondary)
                        Text("The local demo source remains available for dry-running Library, Local Play and planning while Rekordbox import is being built.")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                    Spacer()
                }
            }
        }
    }

    @ViewBuilder
    private var sourceMappings: some View {
        if let settings, !settings.mappingProfiles.isEmpty {
            DisclosureGroup(isExpanded: $isPhraseMappingExpanded) {
                VStack(alignment: .leading, spacing: LumiSpacing.large) {
                    providerTabs(settings)
                    mappingTable(settings)
                    Label(settings.mappingPolicy, systemImage: "lock.shield")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                    if let feedback {
                        Label(feedback, systemImage: "checkmark.circle")
                            .font(LumiTypography.caption)
                            .foregroundStyle(
                                feedback.lowercased().contains("could not")
                                    ? LumiColor.warning
                                    : LumiColor.success
                            )
                    }
                }
                .padding(.top, LumiSpacing.medium)
            } label: {
                VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                    Text("Initial Phrase Mapping")
                        .font(LumiTypography.cardTitle)
                    Text("Map imported source phrases once; later edits remain Lumi-owned.")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                }
            }
            .tint(LumiColor.accent)
            .accessibilityIdentifier("lumi.library.sources.phraseMappingDisclosure")
        }
    }

    private func providerTabs(_ settings: PhraseRoleSettingsState) -> some View {
        HStack(spacing: LumiSpacing.small) {
            ForEach(settings.mappingProfiles) { profile in
                Button(profile.providerName) {
                    selectedProviderKind = profile.providerKind
                }
                .buttonStyle(.bordered)
                .tint(selectedProviderKind == profile.providerKind ? LumiColor.accent : LumiColor.textSecondary)
                .accessibilityIdentifier("lumi.library.sources.mapping.\(profile.providerKind)")
            }
        }
    }

    @ViewBuilder
    private func mappingTable(_ settings: PhraseRoleSettingsState) -> some View {
        let profile = settings.mappingProfiles.first { $0.providerKind == selectedProviderKind }
            ?? settings.mappingProfiles.first
        if let profile {
            LumiPanel {
                VStack(alignment: .leading, spacing: 0) {
                    HStack {
                        Text("Raw source phrase")
                        Spacer()
                        Text("Lumi phrase type")
                            .frame(width: 250, alignment: .leading)
                    }
                    .font(LumiTypography.caption.weight(.semibold))
                    .foregroundStyle(LumiColor.textSecondary)
                    .padding(.bottom, LumiSpacing.medium)
                    Divider()
                    ForEach(profile.mappings) { mapping in
                        mappingRow(mapping, profile: profile, roles: settings.roles)
                    }
                }
            }
        }
    }

    private func mappingRow(
        _ mapping: SourcePhraseMapping,
        profile: SourcePhraseMappingProfile,
        roles: [PhraseRoleDefinition]
    ) -> some View {
        VStack(spacing: 0) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(mapping.rawLabel == "*" ? "Other source phrases" : mapping.rawLabel)
                        .font(LumiTypography.body.weight(.semibold))
                    Text(mapping.rawLabel)
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                Spacer()
                if rendersInteractiveControls {
                    Picker("", selection: mappingBinding(mapping, profile: profile)) {
                        ForEach(roles.filter { !$0.archived || $0.id == mapping.roleID }) { role in
                            Text(role.archived ? "\(role.name) · Archived" : role.name)
                                .tag(role.id)
                        }
                    }
                    .labelsHidden()
                    .frame(width: 250)
                } else {
                    Text(roles.first { $0.id == mapping.roleID }?.name ?? mapping.roleID)
                        .frame(width: 250, alignment: .leading)
                }
            }
            .padding(.vertical, LumiSpacing.small)
            Divider()
        }
    }

    private func mappingBinding(
        _ mapping: SourcePhraseMapping,
        profile: SourcePhraseMappingProfile
    ) -> Binding<String> {
        Binding(
            get: { mapping.roleID },
            set: { roleID in
                guard roleID != mapping.roleID else { return }
                onMutation(
                    .setSourceMapping(
                        providerKind: profile.providerKind,
                        rawLabel: mapping.rawLabel,
                        roleID: roleID
                    )
                )
            }
        )
    }

    private func sourceIcon(_ systemName: String, state: LumiComponentState) -> some View {
        Image(systemName: systemName)
            .font(.system(size: 22, weight: .semibold))
            .foregroundStyle(state.color)
            .frame(width: 46, height: 46)
            .background(state.color.opacity(0.14))
            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
    }

    private func synchronizeProvider() {
        guard let settings else { return }
        if !settings.mappingProfiles.contains(where: { $0.providerKind == selectedProviderKind }) {
            selectedProviderKind = settings.mappingProfiles.first?.providerKind
        }
    }

    private var rekordboxSourceStatus: String {
        if isScanning { return "Scanning import folder read-only…" }
        if let sourceError { return sourceError }
        guard let discovery else {
            return rekordboxFolderPath.isEmpty ? "Not configured" : "Configured · no valid export loaded"
        }
        return "Ready · \(discovery.export.fileName) · \(availableExportCount) XML export\(availableExportCount == 1 ? "" : "s") found"
    }

    private func sourceSettingRow(title: String, detail: String, systemImage: String) -> some View {
        HStack(alignment: .top, spacing: LumiSpacing.medium) {
            Image(systemName: systemImage)
                .foregroundStyle(LumiColor.accent)
                .frame(width: 22)
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(LumiTypography.body.weight(.semibold))
                Text(detail)
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.textSecondary)
                    .lineLimit(2)
                    .truncationMode(.middle)
            }
            Spacer()
        }
    }

    private func chooseImportFolder() {
        let panel = NSOpenPanel()
        panel.title = "Choose Rekordbox XML Import Folder"
        panel.message = "Lumi will only read XML files from this folder."
        panel.prompt = "Use Import Folder"
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.canCreateDirectories = true
        if !rekordboxFolderPath.isEmpty {
            panel.directoryURL = URL(fileURLWithPath: rekordboxFolderPath, isDirectory: true)
        }
        guard panel.runModal() == .OK, let url = panel.url else { return }
        rekordboxFolderPath = url.path
        discovery = nil
        sourceError = nil
        scanImportFolder()
    }

    private func scanImportFolder() {
        guard !rekordboxFolderPath.isEmpty, !isScanning else { return }
        let path = rekordboxFolderPath
        isScanning = true
        sourceError = nil
        Task {
            do {
                let scan = try await Task.detached {
                    let service = RekordboxXMLDiscoveryService()
                    let exports = try service.exports(
                        in: URL(fileURLWithPath: path, isDirectory: true)
                    )
                    return (exports.count, try exports.first.map(service.scan))
                }.value
                availableExportCount = scan.0
                discovery = scan.1
                if scan.1 == nil {
                    sourceError = "No XML exports found in this folder"
                }
            } catch {
                availableExportCount = 0
                discovery = nil
                sourceError = error.localizedDescription
            }
            isScanning = false
        }
    }

    private func restoreFollowedPaths() {
        guard let data = followedPathsJSON.data(using: .utf8),
              let paths = try? JSONDecoder().decode([String].self, from: data) else {
            followedPaths = []
            return
        }
        followedPaths = Set(paths)
    }

    private func persistFollowedPaths() {
        guard let data = try? JSONEncoder().encode(followedPaths.sorted()),
              let value = String(data: data, encoding: .utf8) else { return }
        followedPathsJSON = value
    }

    private func exportDetails(_ state: RekordboxXMLDiscoveryState) -> String {
        let size = ByteCountFormatter.string(fromByteCount: Int64(state.export.sizeBytes), countStyle: .file)
        return "Rekordbox \(state.productVersion) · \(state.collectionEntries) collection tracks · \(size) · modified \(state.export.modifiedAt.formatted(date: .abbreviated, time: .shortened))"
    }

    private func selectionSummary(_ state: RekordboxXMLDiscoveryState) -> String {
        let nodes = flattened(state.roots)
        let followed = nodes.filter { followedPaths.contains($0.path) }
        let folders = followed.filter { $0.kind == .folder }.count
        let playlists = followed.filter { $0.kind == .playlist }.count
        let references = followed.reduce(UInt64(0)) { $0 + $1.descendantTrackCount }
        if followed.isEmpty { return "No playlists followed yet" }
        return "\(folders) folder\(folders == 1 ? "" : "s") · \(playlists) playlist\(playlists == 1 ? "" : "s") · \(references) track references"
    }

    private func flattened(_ nodes: [RekordboxPlaylistNode]) -> [RekordboxPlaylistNode] {
        nodes.flatMap { [$0] + flattened($0.children) }
    }
}

private struct RekordboxPlaylistTreeRow: View {
    let node: RekordboxPlaylistNode
    @Binding var followedPaths: Set<String>
    let isInteractive: Bool
    let onChange: () -> Void

    @State private var isExpanded = true

    var body: some View {
        if node.kind == .folder {
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 4) {
                    Button { isExpanded.toggle() } label: {
                        Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                            .font(.system(size: 11, weight: .semibold))
                            .frame(width: 16, height: 28)
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(LumiColor.textSecondary)
                    .accessibilityLabel(isExpanded ? "Collapse \(node.name)" : "Expand \(node.name)")
                    .accessibilityIdentifier("lumi.library.sources.rekordbox.disclosure.\(node.path)")
                    selectionButton
                }
                if isExpanded {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(node.children) { child in
                            RekordboxPlaylistTreeRow(
                                node: child,
                                followedPaths: $followedPaths,
                                isInteractive: isInteractive,
                                onChange: onChange
                            )
                        }
                    }
                    .padding(.leading, LumiSpacing.large)
                }
            }
        } else {
            selectionButton
        }
    }

    private var selectionButton: some View {
        Button {
            guard isInteractive else { return }
            if followedPaths.contains(node.path) {
                followedPaths.remove(node.path)
            } else {
                followedPaths.insert(node.path)
            }
            onChange()
        } label: {
            HStack(spacing: LumiSpacing.small) {
                Image(systemName: followedPaths.contains(node.path) ? "checkmark.square.fill" : "square")
                    .foregroundStyle(followedPaths.contains(node.path) ? LumiColor.accent : LumiColor.textSecondary)
                Image(systemName: node.kind == .folder ? "folder.fill" : "music.note.list")
                    .foregroundStyle(node.kind == .folder ? LumiColor.accent : LumiColor.textSecondary)
                Text(node.name)
                    .font(LumiTypography.body)
                Spacer()
                Text(node.kind == .folder ? "\(node.descendantTrackCount) references" : "\(node.trackCount) tracks")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            .contentShape(Rectangle())
            .padding(.vertical, 5)
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("lumi.library.sources.rekordbox.node.\(node.path)")
    }
}
