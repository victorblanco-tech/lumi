import Foundation
import LumiDesignSystem
import SwiftUI

public struct DataBackupsSettingsView: View {
    private let data: DataManagementState
    private let operation: DataManagementOperationState
    private let backups: [LibraryBackupRecord]
    private let canManageData: Bool
    private let onCreateBackup: @Sendable () -> Void
    private let onPrepareReset: @Sendable ([UInt64]) -> Void
    private let onApplyReset: @Sendable () -> Void
    private let onRestoreBackup: @Sendable (String) -> Void

    @State private var preserveTrackIDs: Set<UInt64>
    @State private var knownCandidateTrackIDs: Set<UInt64>
    @State private var backupPendingRestore: LibraryBackupRecord?
    @State private var showsResetConfirmation = false

    public init(
        data: DataManagementState,
        operation: DataManagementOperationState,
        backups: [LibraryBackupRecord],
        canManageData: Bool,
        onCreateBackup: @escaping @Sendable () -> Void,
        onPrepareReset: @escaping @Sendable ([UInt64]) -> Void,
        onApplyReset: @escaping @Sendable () -> Void,
        onRestoreBackup: @escaping @Sendable (String) -> Void
    ) {
        self.data = data
        self.operation = operation
        self.backups = backups
        self.canManageData = canManageData
        self.onCreateBackup = onCreateBackup
        self.onPrepareReset = onPrepareReset
        self.onApplyReset = onApplyReset
        self.onRestoreBackup = onRestoreBackup
        let candidateTrackIDs = Set(data.resetCandidates.map(\.trackID))
        _preserveTrackIDs = State(initialValue: candidateTrackIDs)
        _knownCandidateTrackIDs = State(initialValue: candidateTrackIDs)
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: LumiSpacing.xLarge) {
                heading
                if operation.phase != .idle {
                    operationStatus
                }
                backupPanel
                resetPanel
                creativeArchivePanel
            }
            .padding(LumiSpacing.xLarge)
            .frame(maxWidth: 920, alignment: .leading)
        }
        .confirmationDialog(
            "Restore this complete Lumi backup?",
            isPresented: Binding(
                get: { backupPendingRestore != nil },
                set: { if !$0 { backupPendingRestore = nil } }
            ),
            titleVisibility: .visible
        ) {
            if let backupPendingRestore {
                Button("Restore \(backupPendingRestore.name)", role: .destructive) {
                    onRestoreBackup(backupPendingRestore.path)
                    self.backupPendingRestore = nil
                }
            }
            Button("Cancel", role: .cancel) { backupPendingRestore = nil }
        } message: {
            Text("Lumi first creates a safety backup of the current state. The selected backup then replaces Library & Phrases, Lumi Configuration, Lighting Output, and saved App Preferences.")
        }
        .confirmationDialog(
            "Reset the reviewed library content?",
            isPresented: $showsResetConfirmation,
            titleVisibility: .visible
        ) {
            Button("Reset Library Content", role: .destructive) {
                onApplyReset()
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("The mandatory backup already exists. Lumi Creative Archive keeps user-authored phrase work independently so it can be linked to tracks imported by a future USB workflow.")
        }
        .onChange(of: data.resetCandidates.map(\.trackID)) { _, currentIDs in
            let current = Set(currentIDs)
            let newlyAvailable = current.subtracting(knownCandidateTrackIDs)
            preserveTrackIDs.formIntersection(current)
            preserveTrackIDs.formUnion(newlyAvailable)
            knownCandidateTrackIDs = current
        }
        .accessibilityIdentifier("lumi.settings.dataBackups")
    }

    private var heading: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
            Text("Data & Backups")
                .font(LumiTypography.screenTitle)
            Text("Protect your complete Lumi work, rebuild USB-driven libraries cleanly, and carry authored phrase timelines into a new playlist workflow.")
                .font(LumiTypography.body)
                .foregroundStyle(LumiColor.textSecondary)
        }
    }

    private var operationStatus: some View {
        LumiPanel {
            HStack(spacing: LumiSpacing.medium) {
                if operation.isBusy {
                    ProgressView()
                        .controlSize(.small)
                } else {
                    Image(systemName: operation.phase == .failed ? "xmark.octagon.fill" : "checkmark.circle.fill")
                        .foregroundStyle(operation.phase == .failed ? LumiColor.destructive : LumiColor.success)
                }
                VStack(alignment: .leading, spacing: 3) {
                    Text(operation.title)
                        .font(LumiTypography.body.weight(.semibold))
                    Text(operation.detail)
                        .font(LumiTypography.metadata)
                        .foregroundStyle(LumiColor.textSecondary)
                }
            }
        }
    }

    private var backupPanel: some View {
        LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.large) {
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                        Label("Complete Lumi Backup", systemImage: "externaldrive.badge.timemachine")
                            .font(LumiTypography.sectionTitle)
                        Text("One atomic backup contains Library & Phrases, Lumi Configuration, Lighting Output, and App Preferences.")
                            .font(LumiTypography.metadata)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                    Spacer()
                    Button {
                        onCreateBackup()
                    } label: {
                        Label("Create Backup", systemImage: "plus.circle.fill")
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(!canManageData || operation.isBusy)
                    .accessibilityIdentifier("lumi.settings.backup.create")
                }
                if backups.isEmpty {
                    Text("No complete backups yet.")
                        .font(LumiTypography.metadata)
                        .foregroundStyle(LumiColor.textSecondary)
                } else {
                    Divider()
                    ForEach(backups.prefix(6)) { backup in
                        HStack(spacing: LumiSpacing.medium) {
                            Image(systemName: "shippingbox.fill")
                                .foregroundStyle(LumiColor.accent)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(backup.name)
                                    .font(LumiTypography.body.weight(.semibold))
                                Text("\(backup.createdAt.formatted(date: .abbreviated, time: .shortened)) · \(formattedBytes(backup.sizeBytes))")
                                    .font(LumiTypography.technical)
                                    .foregroundStyle(LumiColor.textSecondary)
                            }
                            Spacer()
                            Button("Restore…") {
                                backupPendingRestore = backup
                            }
                            .buttonStyle(.bordered)
                            .disabled(!canManageData || operation.isBusy)
                        }
                    }
                }
            }
        }
    }

    private var resetPanel: some View {
        LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.large) {
                VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                    Label("Rebuild Library Content", systemImage: "arrow.triangle.2.circlepath")
                        .font(LumiTypography.sectionTitle)
                    Text("Remove old tracks, playlists, source mirrors, and sync history while keeping global Lumi and lighting configuration. User-authored phrase work is archived before deletion.")
                        .font(LumiTypography.metadata)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                HStack(spacing: LumiSpacing.xLarge) {
                    metric("TRACKS", data.trackCount)
                    metric("PLAYLISTS", data.playlistCount)
                    metric("USER-EDITED", data.userEditedTrackCount)
                    metric("ARCHIVED", data.creativeArchiveCount)
                }
                if !data.resetCandidates.isEmpty {
                    Divider()
                    VStack(alignment: .leading, spacing: LumiSpacing.small) {
                        Text("Keep immediately available after reset")
                            .font(LumiTypography.body.weight(.semibold))
                        Text("Unchecked authored tracks are still protected in Creative Archive and can return when a matching USB track is synchronized.")
                            .font(LumiTypography.metadata)
                            .foregroundStyle(LumiColor.textSecondary)
                        ForEach(data.resetCandidates) { track in
                            Toggle(isOn: Binding(
                                get: { preserveTrackIDs.contains(track.trackID) },
                                set: { selected in
                                    if selected {
                                        preserveTrackIDs.insert(track.trackID)
                                    } else {
                                        preserveTrackIDs.remove(track.trackID)
                                    }
                                }
                            )) {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(track.title)
                                        .font(LumiTypography.body.weight(.semibold))
                                    Text("\(track.artist) · phrase revision R\(track.timelineRevision)")
                                        .font(LumiTypography.technical)
                                        .foregroundStyle(LumiColor.textSecondary)
                                }
                            }
                            .toggleStyle(.checkbox)
                        }
                    }
                }
                if let preview = data.resetPreview {
                    Divider()
                    resetPreview(preview)
                } else {
                    HStack {
                        Spacer()
                        Button {
                            onPrepareReset(preserveTrackIDs.sorted())
                        } label: {
                            Label("Create Backup & Review Reset", systemImage: "checkmark.shield")
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(!canManageData || operation.isBusy || data.trackCount == 0)
                        .accessibilityIdentifier("lumi.settings.reset.preview")
                    }
                }
            }
        }
    }

    private func resetPreview(_ preview: LibraryResetPreview) -> some View {
        VStack(alignment: .leading, spacing: LumiSpacing.medium) {
            Label("REVIEWED · BACKUP COMPLETE", systemImage: "checkmark.shield.fill")
                .font(LumiTypography.technical.weight(.semibold))
                .foregroundStyle(LumiColor.success)
            HStack(spacing: LumiSpacing.xLarge) {
                metric("REMOVE", preview.removedTrackCount)
                metric("KEEP", preview.preservedTrackCount)
                metric("PLAYLISTS", preview.playlistCount)
                metric("CREATIVE ARCHIVES", preview.archivedCreativeTrackCount)
            }
            Text("Phrase roles, source mappings, Soundswitch banks, AutoLoop names and mappings, MIDI/output configuration, timing, and app preferences remain intact.")
                .font(LumiTypography.metadata)
                .foregroundStyle(LumiColor.textSecondary)
            HStack {
                Spacer()
                Button("Reset Library Content…", role: .destructive) {
                    showsResetConfirmation = true
                }
                .buttonStyle(.borderedProminent)
                .tint(LumiColor.destructive)
                .disabled(!canManageData || operation.isBusy)
                .accessibilityIdentifier("lumi.settings.reset.apply")
            }
        }
    }

    private var creativeArchivePanel: some View {
        LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                HStack {
                    Label("Creative Phrase Archive", systemImage: "waveform.badge.plus")
                        .font(LumiTypography.sectionTitle)
                    Spacer()
                    if data.pendingArchiveCount > 0 {
                        Text("\(data.pendingArchiveCount) WAITING")
                            .font(LumiTypography.technical.weight(.semibold))
                            .foregroundStyle(LumiColor.warning)
                    }
                }
                Text("The latest authored phrase timeline and track-level AutoLoop choices live independently from USB playlist organization. Exact audio identity restores automatically; changed beat structures remain for review.")
                    .font(LumiTypography.metadata)
                    .foregroundStyle(LumiColor.textSecondary)
                if data.creativeArchives.isEmpty {
                    Text("No detached creative timelines. They are created automatically during a library rebuild.")
                        .font(LumiTypography.metadata)
                        .foregroundStyle(LumiColor.textSecondary)
                } else {
                    Divider()
                    ForEach(data.creativeArchives.prefix(12)) { archive in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(archive.title)
                                    .font(LumiTypography.body.weight(.semibold))
                                Text("\(archive.artist) · \(archive.phraseCount) phrases · \(archive.totalBeats) beats")
                                    .font(LumiTypography.technical)
                                    .foregroundStyle(LumiColor.textSecondary)
                            }
                            Spacer()
                            Text(archive.state.uppercased())
                                .font(LumiTypography.technical.weight(.semibold))
                                .foregroundStyle(archiveColor(archive.state))
                        }
                    }
                }
            }
        }
    }

    private func metric(_ label: String, _ value: UInt64) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label)
                .font(LumiTypography.technical)
                .foregroundStyle(LumiColor.textSecondary)
            Text("\(value)")
                .font(LumiTypography.sectionTitle.monospacedDigit())
        }
    }

    private func formattedBytes(_ bytes: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(clamping: bytes), countStyle: .file)
    }

    private func archiveColor(_ state: String) -> Color {
        switch state {
        case "restored", "preserved": LumiColor.success
        case "review": LumiColor.warning
        default: LumiColor.textSecondary
        }
    }
}
