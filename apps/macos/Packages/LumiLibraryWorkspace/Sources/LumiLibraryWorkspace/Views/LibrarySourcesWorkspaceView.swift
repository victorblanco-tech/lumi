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
    private let onAnalysisImport: @Sendable (RekordboxXMLSyncPreviewRequest, String) -> Void

    @AppStorage(LumiPreferenceKey.rekordboxXMLFolder)
    private var rekordboxFolderPath = ""
    @AppStorage(LumiPreferenceKey.rekordboxXMLIncludeFutureChildren)
    private var includeFutureChildPlaylists = true
    @AppStorage(LumiPreferenceKey.rekordboxXMLFollowedPaths)
    private var followedPathsJSON = "[]"

    @State private var selectedProviderKind: String?
    @State private var followedPaths: Set<String> = []
    @State private var discovery: RekordboxXMLDiscoveryState?
    @State private var availableExportCount = 0
    @State private var sourceError: String?
    @State private var isScanning = false
    @State private var didInitializeSource = false
    @State private var isRekordboxExportExpanded = false
    @State private var isSyncPreviewDetailsExpanded = false
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
        onSyncApply: @escaping @Sendable (RekordboxXMLSyncPreviewRequest, String) -> Void = { _, _ in },
        onAnalysisImport: @escaping @Sendable (RekordboxXMLSyncPreviewRequest, String) -> Void = { _, _ in }
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
        self.onAnalysisImport = onAnalysisImport
        _selectedProviderKind = State(initialValue: settings?.mappingProfiles.first?.providerKind)
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: LumiSpacing.xLarge) {
                header
                rekordboxSource
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
            VStack(alignment: .leading, spacing: LumiSpacing.large) {
                HStack(alignment: .top, spacing: LumiSpacing.large) {
                    sourceIcon("r.square.fill", state: discovery == nil ? .empty : .ready)
                    VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                        HStack {
                            Text("Rekordbox")
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
                        Text("Lumi reads the newest XML export for playlist scope. Rekordbox source files are never modified or deleted.")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.textSecondary)
                        if let sourceError {
                            Label(sourceError, systemImage: "exclamationmark.triangle.fill")
                                .font(LumiTypography.caption)
                                .foregroundStyle(LumiColor.warning)
                        }
                    }
                    Spacer()
                    HStack(spacing: LumiSpacing.small) {
                        Button(rekordboxFolderPath.isEmpty ? "Choose Folder…" : "Change Folder…") {
                            chooseImportFolder()
                        }
                        .buttonStyle(.bordered)
                        .disabled(!rendersInteractiveControls)
                        .accessibilityIdentifier("lumi.library.sources.rekordbox.chooseFolder")
                        if discovery != nil {
                            Button(isScanning ? "Reading…" : primarySyncActionTitle) {
                                scanImportFolder(previewAfterScan: true)
                            }
                            .buttonStyle(.borderedProminent)
                            .disabled(!canPreviewSync)
                            .help("Reload the newest XML export and calculate changes without modifying the Lumi library")
                            .accessibilityIdentifier("lumi.library.sources.rekordbox.previewSync")
                        }
                    }
                }
                if let discovery {
                    Divider()
                    DisclosureGroup(isExpanded: $isRekordboxExportExpanded) {
                        VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                            HStack {
                                Text("Follow playlists")
                                    .font(LumiTypography.body.weight(.semibold))
                                Spacer()
                                Text("\(discovery.folderCount) folders · \(discovery.playlistCount) playlists")
                                    .font(LumiTypography.technical)
                                    .foregroundStyle(LumiColor.textSecondary)
                            }
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
                            Toggle(isOn: $includeFutureChildPlaylists) {
                                VStack(alignment: .leading, spacing: 3) {
                                    Text("Include future playlists inside followed folders")
                                        .font(LumiTypography.body.weight(.semibold))
                                    Text("New playlists inside a followed folder are included automatically on the next check.")
                                        .font(LumiTypography.caption)
                                        .foregroundStyle(LumiColor.textSecondary)
                                }
                            }
                            .toggleStyle(.switch)
                            .disabled(!rendersInteractiveControls)
                            Divider()
                            HStack(spacing: LumiSpacing.large) {
                                sourceSettingRow(
                                    title: "Mirror membership",
                                    detail: "Additions and removals are reviewed before apply",
                                    systemImage: "arrow.triangle.2.circlepath"
                                )
                                sourceSettingRow(
                                    title: "Removed tracks",
                                    detail: "Archived so Lumi-owned edits remain recoverable",
                                    systemImage: "archivebox.fill"
                                )
                            }
                        }
                        .padding(.top, LumiSpacing.medium)
                    } label: {
                        HStack(alignment: .center, spacing: LumiSpacing.medium) {
                            Image(systemName: "doc.text.fill")
                                .foregroundStyle(LumiColor.accent)
                            VStack(alignment: .leading, spacing: 3) {
                                Text(discovery.export.fileName)
                                    .font(LumiTypography.body.weight(.semibold))
                                Text(exportDetails(discovery))
                                    .font(LumiTypography.technical)
                                    .foregroundStyle(LumiColor.textSecondary)
                            }
                            Spacer()
                            VStack(alignment: .trailing, spacing: 3) {
                                Text(selectionSummary(discovery))
                                    .font(LumiTypography.caption)
                                    .foregroundStyle(followedPaths.isEmpty ? LumiColor.textSecondary : LumiColor.accent)
                                Text("Expand to choose playlists")
                                    .font(LumiTypography.technical)
                                    .foregroundStyle(LumiColor.textSecondary)
                            }
                        }
                    }
                    .tint(LumiColor.accent)
                    .accessibilityIdentifier("lumi.library.sources.rekordbox.exportDisclosure")
                } else if !rekordboxFolderPath.isEmpty {
                    Divider()
                    HStack {
                        sourceSettingRow(
                            title: "XML import folder",
                            detail: rekordboxFolderPath,
                            systemImage: "folder.fill"
                        )
                        Spacer()
                        Button(isScanning ? "Reading…" : "Read Folder") {
                            scanImportFolder()
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(isScanning || !rendersInteractiveControls)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var rekordboxSyncPreview: some View {
        if !syncFeedbackIsError,
           let preview = library.rekordboxSyncPreview,
           previewMatchesCurrentConfiguration(preview) {
            LumiPanel {
                VStack(alignment: .leading, spacing: LumiSpacing.large) {
                    HStack(alignment: .center, spacing: LumiSpacing.large) {
                        sourceIcon(
                            preview.applyState == "applied" || !previewHasChanges(preview)
                                ? "checkmark.shield.fill"
                                : "arrow.triangle.2.circlepath",
                            state: .ready
                        )
                        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                            HStack {
                                Text(syncPreviewTitle(preview))
                                    .font(LumiTypography.cardTitle)
                                StatusBadge(syncPreviewBadge(preview), state: .ready)
                            }
                            Text(syncPreviewSummary(preview))
                                .font(LumiTypography.caption)
                                .foregroundStyle(LumiColor.textSecondary)
                        }
                        Spacer()
                        Button(syncApplyButtonTitle(preview)) {
                            onSyncApply(
                                currentSyncRequest,
                                preview.contentSHA256
                            )
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(
                            preview.applyState == "applied"
                                || !previewHasChanges(preview)
                                || !rendersInteractiveControls
                        )
                        .help("Atomically stage this exact SHA-256-bound preview; missing tracks are archived, never deleted")
                        .accessibilityLabel(syncApplyButtonTitle(preview))
                        .accessibilityIdentifier("lumi.library.sources.rekordbox.applySync")
                    }
                    HStack(spacing: LumiSpacing.medium) {
                        previewMetric("Playlists", value: preview.followedPlaylistCount, systemImage: "music.note.list")
                        previewMetric("Tracks", value: preview.uniqueTrackCount, systemImage: "music.note")
                        previewMetric("Add", value: preview.diff.inserted, systemImage: "plus.circle.fill")
                        previewMetric("Update", value: preview.diff.updated, systemImage: "arrow.clockwise.circle.fill")
                        previewMetric("Archive", value: preview.diff.archived, systemImage: "archivebox.fill")
                    }
                    DisclosureGroup(isExpanded: $isSyncPreviewDetailsExpanded) {
                        VStack(alignment: .leading, spacing: LumiSpacing.large) {
                            Divider()
                            VStack(alignment: .leading, spacing: LumiSpacing.small) {
                                Text("Selected playlists")
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
                                Text("XML diagnostics")
                                    .font(LumiTypography.caption.weight(.semibold))
                                    .foregroundStyle(LumiColor.textSecondary)
                                HStack(spacing: LumiSpacing.large) {
                                    diagnosticLabel("Duplicate references", preview.diagnostics.duplicatePlaylistReferences)
                                    diagnosticLabel("No beat grid", preview.diagnostics.missingBeatGrid)
                                    diagnosticLabel("No key", preview.diagnostics.missingKey)
                                    diagnosticLabel("No duration", preview.diagnostics.missingDuration)
                                    diagnosticLabel("No colour", preview.diagnostics.missingColour)
                                }
                            }
                            Divider()
                            HStack {
                                Text(preview.exportFileName)
                                    .font(LumiTypography.body.weight(.semibold))
                                Spacer()
                                Text("SHA-256 \(preview.contentSHA256.prefix(12))… · Rekordbox \(preview.productVersion)")
                                    .font(LumiTypography.technical)
                                    .foregroundStyle(LumiColor.textSecondary)
                            }
                        }
                        .padding(.top, LumiSpacing.medium)
                    } label: {
                        Text("Review playlists, diagnostics and source fingerprint")
                            .font(LumiTypography.caption.weight(.semibold))
                    }
                    .tint(LumiColor.accent)
                    .accessibilityIdentifier("lumi.library.sources.rekordbox.previewDetailsDisclosure")
                    if let syncFeedback {
                        Label(syncFeedback, systemImage: "checkmark.shield.fill")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.success)
                    }
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
                            StatusBadge(
                                canonicalRekordboxIsActive ? "LIBRARY READY" : "METADATA STAGED",
                                state: .ready
                            )
                        }
                        Text("\(mirror.activeTracks) active tracks · \(mirror.archivedTracks) archived · \(mirror.playlists) playlists")
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.textSecondary)
                        Text(canonicalRekordboxIsActive
                            ? "Beatgrids, RGB waveforms and Rekordbox phrase observations are stored in Lumi. Phrase edits now evolve independently in Lumi."
                            : "Playlist scope and metadata are stored safely. Import analysis from the closed Rekordbox library to publish these tracks atomically.")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                    Spacer()
                    if let preview = library.rekordboxSyncPreview {
                        Button(canonicalRekordboxIsActive ? "Refresh Analysis" : "Import Analysis") {
                            onAnalysisImport(currentSyncRequest, preview.contentSHA256)
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(!rendersInteractiveControls)
                        .help("Rekordbox must be closed. Lumi reads verified snapshots and publishes or refreshes only after the complete import succeeds.")
                        .accessibilityIdentifier("lumi.library.sources.rekordbox.importAnalysis")
                    } else if canonicalRekordboxIsActive {
                        StatusBadge("PUBLISHED", state: .ready)
                    } else {
                        StatusBadge("CHECK FOR CHANGES FIRST", state: .degraded)
                    }
                }
            }
        }
    }

    private var canonicalRekordboxIsActive: Bool {
        library.providerKind == "rekordbox7"
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
                        Text(canonicalRekordboxIsActive
                            ? "Tracks now uses the canonical Rekordbox library. Future phrase edits and lighting choices remain Lumi-owned."
                            : library.rekordboxMirror == nil
                                ? "The local demo source remains available for dry-running Library, Local Play and planning."
                                : "Tracks currently shows this source. The staged Rekordbox mirror will replace it after its analysis has been imported successfully.")
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

    private var currentSyncRequest: RekordboxXMLSyncPreviewRequest {
        RekordboxXMLSyncPreviewRequest(
            folderPath: rekordboxFolderPath,
            followedPaths: followedPaths.sorted(),
            includeFutureChildPlaylists: includeFutureChildPlaylists
        )
    }

    private var canPreviewSync: Bool {
        discovery != nil
            && !followedPaths.isEmpty
            && !isScanning
            && rendersInteractiveControls
    }

    private var primarySyncActionTitle: String {
        library.rekordboxMirror == nil ? "Preview Import" : "Check for Changes"
    }

    private func syncPreviewSummary(_ preview: RekordboxXMLSyncPreview) -> String {
        if preview.applyState == "applied" {
            return "Metadata for \(preview.uniqueTrackCount) tracks is safely staged. Analysis import is the remaining step before the tracks appear in Library."
        }
        let changed = syncChangeCount(preview)
        if changed == 0 {
            return "The staged metadata matches the newest XML export for all \(preview.uniqueTrackCount) selected tracks."
        }
        return "\(changed) change\(changed == 1 ? "" : "s") across \(preview.uniqueTrackCount) tracks. Applying stages metadata; it does not publish incomplete tracks."
    }

    private func syncChangeCount(_ preview: RekordboxXMLSyncPreview) -> UInt64 {
        preview.diff.inserted
            + preview.diff.updated
            + preview.diff.archived
            + preview.diff.restored
    }

    private func previewHasChanges(_ preview: RekordboxXMLSyncPreview) -> Bool {
        syncChangeCount(preview) > 0
    }

    private func syncPreviewTitle(_ preview: RekordboxXMLSyncPreview) -> String {
        if preview.applyState == "applied" { return "Sync Applied" }
        return previewHasChanges(preview) ? "Changes Ready to Apply" : "Rekordbox Metadata Is Up to Date"
    }

    private func syncPreviewBadge(_ preview: RekordboxXMLSyncPreview) -> LocalizedStringKey {
        if preview.applyState == "applied" { return "STAGED" }
        return previewHasChanges(preview) ? "REVIEWED" : "UP TO DATE"
    }

    private func syncApplyButtonTitle(_ preview: RekordboxXMLSyncPreview) -> String {
        if preview.applyState == "applied" { return "Applied" }
        return previewHasChanges(preview) ? "Apply Changes" : "Up to Date"
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

    private func scanImportFolder(previewAfterScan: Bool = false) {
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
                } else if previewAfterScan, !followedPaths.isEmpty {
                    onSyncPreview(currentSyncRequest)
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

    @State private var isExpanded = false

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
