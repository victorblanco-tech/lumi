import AppKit
import Foundation
import LumiDesignSystem
import SwiftUI

public struct USBPlaylistSelectionImpact: Equatable, Sendable {
    public let playlistCount: Int
    public let uniqueTrackCount: Int
    public let currentCount: Int
    public let usbNewerCount: Int
    public let usbOutdatedCount: Int
    public let notInLumiCount: Int
    public let conflictCount: Int

    public init(
        inspection: RekordboxDeviceInspectionState,
        selectedPlaylistIDs: Set<UInt32>
    ) {
        let selected = inspection.playlists.filter { selectedPlaylistIDs.contains($0.id) }
        var tracksByID: [UInt32: RekordboxDeviceTrackState] = [:]
        for playlist in selected {
            for track in playlist.tracks {
                tracksByID[track.id] = track
            }
        }

        playlistCount = selected.count
        uniqueTrackCount = tracksByID.count
        currentCount = tracksByID.values.count { $0.status == "current" }
        usbNewerCount = tracksByID.values.count { $0.status == "usb-newer" }
        usbOutdatedCount = tracksByID.values.count { $0.status == "usb-outdated" }
        notInLumiCount = tracksByID.values.count { $0.status == "not-in-lumi" }
        conflictCount = tracksByID.values.count {
            !["current", "usb-newer", "usb-outdated", "not-in-lumi"].contains($0.status)
        }
    }

    public var changedCount: Int { usbNewerCount + notInLumiCount }
    public var heldCount: Int { usbOutdatedCount + conflictCount }
}

public struct LibrarySourcesWorkspaceView: View {
    private let library: LibraryWorkspaceState
    private let settings: PhraseRoleSettingsState?
    private let feedback: String?
    private let syncFeedback: String?
    private let syncFeedbackIsError: Bool
    private let usbOperation: USBSourceOperationState
    private let rendersInteractiveControls: Bool
    private let onMutation: @Sendable (PhraseRoleMutationRequest) -> Void
    private let onDeviceInspect: @Sendable (String, String?) -> Void
    private let onDeviceSync: @Sendable (String, String?, [UInt32]) -> Void
    private let onDeviceConflictResolution: @Sendable (USBConflictResolutionRequest) -> Void

    @AppStorage(LumiPreferenceKey.rekordboxDeviceRoot)
    private var rekordboxDeviceRoot = ""
    @AppStorage(LumiPreferenceKey.rekordboxDevicePlaylistSelections)
    private var devicePlaylistSelectionsJSON = "{}"
    @AppStorage(LumiPreferenceKey.rekordboxDeviceBookmarks)
    private var deviceBookmarksJSON = "{}"

    @State private var selectedProviderKind: String?
    @State private var didInitializeSource = false
    @State private var isPhraseMappingExpanded = false
    @State private var selectedUSBSourceID: String?
    @State private var selectedUSBPlaylistIDs: Set<UInt32> = []
    @State private var expandedUSBPlaylistIDs: Set<UInt32> = []
    @State private var expandedUSBPlaylistFolderPaths: Set<String> = []
    @State private var usbPlaylistSearch = ""
    @State private var mountRevision = 0
    @State private var mountedUSBChoices: [URL] = []
    @State private var isUSBSourceChoicePresented = false
    @State private var usbSelectionFeedback: String?
    @State private var ignoredUSBReviews: Set<String> = []
    @State private var pendingUSBVersionRequest: USBConflictResolutionRequest?
    @State private var resolvingUSBReviewKey: String?
    @State private var failedUSBReviewKey: String?

    public init(
        library: LibraryWorkspaceState,
        settings: PhraseRoleSettingsState?,
        feedback: String? = nil,
        syncFeedback: String? = nil,
        syncFeedbackIsError: Bool = false,
        usbOperation: USBSourceOperationState = .idle,
        rendersInteractiveControls: Bool = true,
        onMutation: @escaping @Sendable (PhraseRoleMutationRequest) -> Void = { _ in },
        onDeviceInspect: @escaping @Sendable (String, String?) -> Void = { _, _ in },
        onDeviceSync: @escaping @Sendable (String, String?, [UInt32]) -> Void = { _, _, _ in },
        onDeviceConflictResolution: @escaping @Sendable (USBConflictResolutionRequest) -> Void = { _ in }
    ) {
        self.library = library
        self.settings = settings
        self.feedback = feedback
        self.syncFeedback = syncFeedback
        self.syncFeedbackIsError = syncFeedbackIsError
        self.usbOperation = usbOperation
        self.rendersInteractiveControls = rendersInteractiveControls
        self.onMutation = onMutation
        self.onDeviceInspect = onDeviceInspect
        self.onDeviceSync = onDeviceSync
        self.onDeviceConflictResolution = onDeviceConflictResolution
        _selectedProviderKind = State(initialValue: settings?.mappingProfiles.first?.providerKind)
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: LumiSpacing.xLarge) {
                header
                usbMediaSummary
                trustedUSBSources
                sourceMappings
            }
            .padding(LumiSpacing.xLarge)
            .frame(maxWidth: 980, alignment: .leading)
        }
        .background(LumiColor.canvas)
        .accessibilityIdentifier("lumi.library.sources")
        .confirmationDialog(
            "Choose USB Source",
            isPresented: $isUSBSourceChoicePresented,
            titleVisibility: .visible
        ) {
            ForEach(mountedUSBChoices, id: \.path) { url in
                Button(volumeDisplayName(url)) { inspectRekordboxDevice(at: url) }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Only connected Rekordbox OneLibrary USB sources are shown.")
        }
        .confirmationDialog(
            "Use the USB version?",
            isPresented: Binding(
                get: { pendingUSBVersionRequest != nil },
                set: { if !$0 { pendingUSBVersionRequest = nil } }
            ),
            titleVisibility: .visible
        ) {
            Button("Sync to Lumi & Overwrite", role: .destructive) {
                if let request = pendingUSBVersionRequest {
                    onDeviceConflictResolution(request)
                }
                pendingUSBVersionRequest = nil
            }
            Button("Cancel", role: .cancel) { pendingUSBVersionRequest = nil }
        } message: {
            Text("This replaces the imported Rekordbox beatgrid, waveform, cue points and raw Rekordbox phrases. Lumi-authored phrases and AutoLoop choices are preserved.")
        }
        .onChange(of: settings?.revision) { _, _ in synchronizeProvider() }
        .onChange(of: library.rekordboxDevices) { _, devices in
            if let selectedUSBSourceID,
               !devices.contains(where: { $0.sourceID == selectedUSBSourceID }),
               library.rekordboxDeviceInspection?.sourceID != selectedUSBSourceID {
                self.selectedUSBSourceID = nil
            }
        }
        .onChange(of: selectedUSBSourceID) { _, _ in
            expandedUSBPlaylistIDs.removeAll()
            expandedUSBPlaylistFolderPaths.removeAll()
            restoreDevicePlaylistSelection()
        }
        .onChange(of: library.rekordboxDeviceInspection) { _, inspection in
            if let inspection, selectedUSBSourceID != inspection.sourceID {
                // The isolated worker may recover an authoritative marker ID
                // after a volume rename. Follow that completed result without
                // reading any USB file on the SwiftUI main thread.
                selectedUSBSourceID = inspection.sourceID
            }
            expandedUSBPlaylistIDs.removeAll()
            expandedUSBPlaylistFolderPaths.removeAll()
            restoreDevicePlaylistSelection()
        }
        .onChange(of: usbOperation.phase) { _, phase in
            if phase == .failed {
                failedUSBReviewKey = resolvingUSBReviewKey
            } else if phase == .completed {
                failedUSBReviewKey = nil
            }
            if phase == .completed || phase == .failed || phase == .idle {
                resolvingUSBReviewKey = nil
            }
        }
        .onAppear {
            guard !didInitializeSource else { return }
            didInitializeSource = true
            restoreDevicePlaylistSelection()
        }
        .onReceive(
            NSWorkspace.shared.notificationCenter.publisher(
                for: NSWorkspace.didMountNotification
            )
        ) { notification in
            mountRevision &+= 1
            guard let url = notification.userInfo?[NSWorkspace.volumeURLUserInfoKey] as? URL else {
                return
            }
            inspectMountedTrustedSource(url)
        }
        .onReceive(
            NSWorkspace.shared.notificationCenter.publisher(
                for: NSWorkspace.didUnmountNotification
            )
        ) { _ in
            mountRevision &+= 1
        }
    }

    private var usbMediaSummary: some View {
        LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.small) {
                HStack(spacing: LumiSpacing.xLarge) {
                    sourceIcon("externaldrive.fill.badge.checkmark", state: overallUSBState)
                    VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                        Text("USB Sources").font(LumiTypography.cardTitle)
                        Text("\(visibleUSBDevices.count) trusted · \(mountedTrustedSources.count) connected · Rekordbox data read only")
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.textSecondary)
                        Text("Trusted media identifies live Pro DJ Link tracks. Source identity stays in Lumi; Rekordbox and USB files are never changed.")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                    Spacer()
                    Button("Add USB Source…") { chooseRekordboxDevice() }
                        .buttonStyle(.borderedProminent)
                        .disabled(!rendersInteractiveControls)
                        .accessibilityIdentifier("lumi.library.sources.usb.add")
                }
                Label(
                    usbSelectionFeedback ?? "USB identities and synchronization are isolated per trusted source.",
                    systemImage: usbSelectionFeedback == nil
                        ? "checkmark.shield"
                        : "exclamationmark.triangle.fill"
                )
                .font(LumiTypography.caption.weight(.semibold))
                .foregroundStyle(usbSelectionFeedback == nil ? LumiColor.textSecondary : LumiColor.warning)
                .lineLimit(1)
                .frame(height: 16, alignment: .leading)
                .accessibilityIdentifier("lumi.library.sources.usb.selectionFeedback")
            }
        }
    }

    private var trustedUSBSources: some View {
        return VStack(alignment: .leading, spacing: LumiSpacing.medium) {
            HStack {
                Text("Trusted USB Sources").font(LumiTypography.sectionTitle)
                Spacer()
                VStack(alignment: .trailing, spacing: LumiSpacing.xSmall) {
                    compactUSBOperationStatus
                    Text("SOURCE  ·  CONNECTION  ·  SYNC HEALTH")
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                }
            }
            if visibleUSBDevices.isEmpty, provisionalUSBInspection == nil {
                LumiPanel {
                    VStack(alignment: .leading, spacing: LumiSpacing.small) {
                        Text("No trusted USB sources").font(LumiTypography.cardTitle)
                        Text("Connect a current Rekordbox OneLibrary USB and choose Add USB Source. Lumi only reads its library and analysis files.")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                }
            } else {
                VStack(spacing: LumiSpacing.small) {
                    ForEach(visibleUSBDevices) { device in
                        VStack(spacing: LumiSpacing.small) {
                            usbSourceRow(device)
                            if selectedUSBSourceID == device.sourceID {
                                selectedUSBInspector(device: device)
                            }
                        }
                    }
                    if let inspection = provisionalUSBInspection {
                        VStack(spacing: LumiSpacing.small) {
                            provisionalUSBSourceRow(inspection)
                            if selectedUSBSourceID == inspection.sourceID {
                                selectedUSBInspector(device: nil)
                            }
                        }
                    }
                }
                .animation(.easeInOut(duration: 0.18), value: selectedUSBSourceID)
            }
        }
    }

    private func usbSourceRow(_ device: RekordboxDeviceState) -> some View {
        let mounted = mountedURL(for: device) != nil
        let selected = selectedUSBSourceID == device.sourceID
        let displayName = USBSourceIdentityResolver.displayName(
            for: device,
            inspection: selected ? library.rekordboxDeviceInspection : nil
        )
        return Button {
            if selected {
                selectedUSBSourceID = nil
            } else {
                selectedUSBSourceID = device.sourceID
            }
        } label: {
            HStack(spacing: LumiSpacing.large) {
                Image(systemName: mounted ? "externaldrive.fill.badge.checkmark" : "externaldrive")
                    .foregroundStyle(mounted ? LumiColor.success : LumiColor.textSecondary)
                    .frame(width: 28)
                VStack(alignment: .leading, spacing: 3) {
                    Text(displayName).font(LumiTypography.cardTitle)
                    Text("TRUSTED · \(shortRevision(device.databaseRevision))")
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                Spacer()
                compactStatus(mounted ? "CONNECTED" : "OFFLINE", mounted ? .ready : .empty)
                compactStatus(deviceSyncLabel(device), deviceSyncState(device))
                Text("\(device.matchedTracks)/\(device.activeTracks) matched")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
                    .frame(width: 130, alignment: .trailing)
                Image(systemName: selected ? "chevron.down" : "chevron.right")
                    .foregroundStyle(LumiColor.textSecondary)
            }
            .padding(.horizontal, LumiSpacing.large)
            .frame(minHeight: 66)
            .background(selected ? LumiColor.accent.opacity(0.13) : LumiColor.surface)
            .overlay {
                RoundedRectangle(cornerRadius: LumiRadius.panel)
                    .stroke(selected ? LumiColor.accent : LumiColor.border, lineWidth: selected ? 1.5 : 1)
            }
            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.panel))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("lumi.library.sources.usb.\(device.sourceID)")
    }

    private func provisionalUSBSourceRow(
        _ inspection: RekordboxDeviceInspectionState
    ) -> some View {
        let selected = selectedUSBSourceID == inspection.sourceID
        return Button {
            selectedUSBSourceID = selected ? nil : inspection.sourceID
        } label: {
            HStack(spacing: LumiSpacing.large) {
                Image(systemName: "externaldrive.fill.badge.checkmark")
                    .foregroundStyle(LumiColor.success)
                    .frame(width: 28)
                VStack(alignment: .leading, spacing: 3) {
                    Text(inspection.displayName).font(LumiTypography.cardTitle)
                    Text("ANALYZED · NOT YET SYNCHRONIZED")
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                Spacer()
                compactStatus("CONNECTED", .ready)
                compactStatus("READY TO REVIEW", .empty)
                Text("\(inspection.trackCount) on USB")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
                    .frame(width: 130, alignment: .trailing)
                Image(systemName: selected ? "chevron.down" : "chevron.right")
                    .foregroundStyle(LumiColor.textSecondary)
            }
            .padding(.horizontal, LumiSpacing.large)
            .frame(minHeight: 66)
            .background(selected ? LumiColor.accent.opacity(0.13) : LumiColor.surface)
            .overlay {
                RoundedRectangle(cornerRadius: LumiRadius.panel)
                    .stroke(selected ? LumiColor.accent : LumiColor.border, lineWidth: selected ? 1.5 : 1)
            }
            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.panel))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("lumi.library.sources.usb.\(inspection.sourceID)")
    }

    @ViewBuilder
    private func selectedUSBInspector(device: RekordboxDeviceState?) -> some View {
        if let device {
            let displayName = USBSourceIdentityResolver.displayName(
                for: device,
                inspection: activeDeviceInspection
            )
            LumiPanel {
                VStack(alignment: .leading, spacing: LumiSpacing.large) {
                    HStack(alignment: .top) {
                        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                            Text(displayName).font(LumiTypography.sectionTitle)
                            Text("Trusted USB source · last synchronized \(formattedSyncDate(device.syncedAt))")
                                .font(LumiTypography.caption)
                                .foregroundStyle(LumiColor.textSecondary)
                        }
                        Spacer()
                        if let root = mountedURL(for: device)?.path {
                            Button(activeDeviceInspection == nil ? "Choose Playlists…" : "Refresh Playlists") {
                                inspectTrustedDevice(device, root: root)
                            }
                            .buttonStyle(.bordered)
                            .disabled(usbOperation.isActive || !rendersInteractiveControls)
                            Button(deviceSyncButtonTitle) {
                                syncSelectedDevicePlaylists(root: root)
                            }
                                .buttonStyle(.borderedProminent)
                                .disabled(selectedUSBPlaylistIDs.isEmpty || usbOperation.isActive || !rendersInteractiveControls)
                        } else {
                            compactStatus("CONNECT USB TO SYNC", .empty)
                        }
                    }
                    Divider()
                    HStack(spacing: LumiSpacing.xLarge) {
                        metric("Synced", device.activeTracks)
                        metric("Matched", device.matchedTracks)
                        metric("Unmatched · held", device.unmatchedTracks)
                        metric("Current", device.currentTracks)
                        metric("Updated", device.promotedTracks)
                    }
                    if !device.reviewTracks.isEmpty {
                        usbReviewTracks(device)
                    } else if device.conflictTracks > 0 {
                        Label(
                            "\(device.conflictTracks) track\(device.conflictTracks == 1 ? "" : "s") need review. Reconnect and refresh this USB source to load their details.",
                            systemImage: "exclamationmark.triangle.fill"
                        )
                        .font(LumiTypography.caption.weight(.semibold))
                        .foregroundStyle(LumiColor.warning)
                    } else if device.protectedTracks > 0 {
                        Label(
                            "\(device.protectedTracks) older USB track version\(device.protectedTracks == 1 ? " was" : "s were") safely held; no review is required.",
                            systemImage: "checkmark.shield.fill"
                        )
                        .font(LumiTypography.caption.weight(.semibold))
                        .foregroundStyle(LumiColor.success)
                    } else {
                        Label("No downgrade risk detected. Active Lumi analysis and all Lumi-owned phrases and AutoLoop choices are protected.", systemImage: "checkmark.shield.fill")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.success)
                    }
                    HStack(spacing: LumiSpacing.large) {
                        sourceSettingRow(title: "Database revision", detail: shortRevision(device.databaseRevision), systemImage: "cylinder")
                        sourceSettingRow(title: "Version policy", detail: "Newer promotes · older/unknown holds", systemImage: "arrow.up.arrow.down")
                    }
                    Divider()
                    if let inspection = activeDeviceInspection {
                        devicePlaylistSelection(inspection)
                    } else {
                        storedDevicePlaylists(device)
                    }
                }
            }
        } else if let inspection = activeDeviceInspection {
            LumiPanel {
                VStack(alignment: .leading, spacing: LumiSpacing.large) {
                    HStack {
                        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                            Text(inspection.displayName).font(LumiTypography.sectionTitle)
                            Text("USB indexed read-only · choose playlists before the first sync")
                                .font(LumiTypography.caption)
                                .foregroundStyle(LumiColor.textSecondary)
                        }
                        Spacer()
                        if !rekordboxDeviceRoot.isEmpty {
                            Button(deviceSyncButtonTitle) {
                                syncSelectedDevicePlaylists(root: rekordboxDeviceRoot)
                            }
                            .buttonStyle(.borderedProminent)
                            .disabled(selectedUSBPlaylistIDs.isEmpty || usbOperation.isActive || !rendersInteractiveControls)
                        }
                    }
                    Divider()
                    devicePlaylistSelection(inspection)
                }
            }
        }
    }

    @ViewBuilder
    private func storedDevicePlaylists(_ device: RekordboxDeviceState) -> some View {
        VStack(alignment: .leading, spacing: LumiSpacing.medium) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Synchronized playlists")
                        .font(LumiTypography.cardTitle)
                    Text("Stored in Lumi and available while this USB is disconnected")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                Spacer()
                compactStatus("\(device.playlists.count) STORED", device.playlists.isEmpty ? .empty : .ready)
            }
            if device.playlists.isEmpty {
                Label(
                    "Legacy full-device sync · \(device.activeTracks) track identities are stored, but this older sync did not record playlist names. Reconnect the USB and synchronize selected playlists once to make them available offline.",
                    systemImage: "externaldrive.badge.questionmark"
                )
                .font(LumiTypography.caption)
                .foregroundStyle(LumiColor.textSecondary)
                .padding(LumiSpacing.medium)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(LumiColor.surfaceElevated)
                .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
            } else {
                VStack(spacing: LumiSpacing.xSmall) {
                    ForEach(device.playlists) { playlist in
                        HStack(spacing: LumiSpacing.medium) {
                            Image(systemName: "music.note.list")
                                .foregroundStyle(LumiColor.success)
                            Text(playlist.name)
                                .font(LumiTypography.body.weight(.semibold))
                                .lineLimit(1)
                                .truncationMode(.middle)
                                .layoutPriority(1)
                                .help(playlist.name)
                            Spacer()
                            compactStatus("SYNCED", .ready)
                            Text("\(playlist.trackCount) tracks")
                                .font(LumiTypography.technical)
                                .foregroundStyle(LumiColor.textSecondary)
                        }
                        .padding(.horizontal, LumiSpacing.medium)
                        .frame(minHeight: 42)
                        .background(LumiColor.surfaceElevated)
                        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
                    }
                }
            }
        }
    }

    private var compactUSBOperationStatus: some View {
        HStack(spacing: LumiSpacing.xSmall) {
            if usbOperation.isActive {
                ProgressView().controlSize(.small)
            } else {
                Image(systemName: compactUSBOperationIcon)
                    .foregroundStyle(compactUSBOperationState.color)
            }
            Text(compactUSBOperationLabel)
                .font(LumiTypography.technical)
                .foregroundStyle(compactUSBOperationState.color)
                .lineLimit(1)
        }
        .padding(.horizontal, LumiSpacing.small)
        .frame(width: 158, alignment: .center)
        .frame(minHeight: 26)
        .background(compactUSBOperationState.color.opacity(0.10))
        .overlay {
            RoundedRectangle(cornerRadius: LumiRadius.control)
                .stroke(compactUSBOperationState.color.opacity(0.45), lineWidth: 1)
        }
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        .help("\(usbOperation.title)\n\(usbOperation.detail)")
        .accessibilityIdentifier("lumi.library.sources.usb.operation")
    }

    private var compactUSBOperationLabel: String {
        switch usbOperation.phase {
        case .idle: "USB READY"
        case .reading: "SCANNING USB"
        case .synchronizing: "SYNCING USB"
        case .completed:
            usbOperation.title.localizedCaseInsensitiveContains("sync")
                ? "SYNC COMPLETE"
                : "SCAN COMPLETE"
        case .failed: "USB ACTION FAILED"
        }
    }

    private var compactUSBOperationIcon: String {
        switch usbOperation.phase {
        case .idle: "externaldrive"
        case .reading, .synchronizing: "arrow.triangle.2.circlepath"
        case .completed: "checkmark.circle.fill"
        case .failed: "exclamationmark.triangle.fill"
        }
    }

    private var compactUSBOperationState: LumiComponentState {
        switch usbOperation.phase {
        case .idle: .empty
        case .reading, .synchronizing: .stale
        case .completed: .ready
        case .failed: .degraded
        }
    }

    private func usbReviewTracks(_ device: RekordboxDeviceState) -> some View {
        let visibleTracks = device.reviewTracks.filter {
            !ignoredUSBReviews.contains(reviewKey(device, $0))
        }
        return VStack(alignment: .leading, spacing: LumiSpacing.medium) {
            HStack {
                Label("Tracks to review", systemImage: "exclamationmark.triangle.fill")
                    .font(LumiTypography.cardTitle)
                    .foregroundStyle(LumiColor.warning)
                Spacer()
                compactStatus("\(visibleTracks.count) REVIEW", visibleTracks.isEmpty ? .ready : .degraded)
            }
            Text("Lumi did not overwrite these tracks. Compare each imported Rekordbox component, then choose what should happen with this exact USB revision.")
                .font(LumiTypography.caption)
                .foregroundStyle(LumiColor.textSecondary)
            VStack(spacing: LumiSpacing.xSmall) {
                ForEach(visibleTracks) { track in
                    VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                        HStack(alignment: .top, spacing: LumiSpacing.medium) {
                            VStack(alignment: .leading, spacing: 3) {
                            Text(track.title)
                                .font(LumiTypography.body.weight(.semibold))
                                .foregroundStyle(LumiColor.textPrimary)
                            Text(track.artist.isEmpty ? "Unknown artist" : track.artist)
                                .font(LumiTypography.caption)
                                .foregroundStyle(LumiColor.textSecondary)
                            Text(track.reason)
                                .font(LumiTypography.caption)
                                .foregroundStyle(LumiColor.textSecondary)
                                .fixedSize(horizontal: false, vertical: true)
                            }
                            Spacer(minLength: LumiSpacing.medium)
                            Text(String(format: "%.2f BPM", Double(track.bpmMilli) / 1_000))
                                .font(LumiTypography.technical)
                                .foregroundStyle(LumiColor.textSecondary)
                            compactStatus("REVIEW", .degraded)
                        }
                        HStack(spacing: LumiSpacing.large) {
                            reviewFact("USB source date", formattedReviewDate(track.incomingAnalyzedAt))
                            reviewFact("Active Lumi source", track.activeSourceName ?? "Unknown source")
                            reviewFact("Active source date", formattedReviewDate(track.activeAnalyzedAt))
                        }
                        HStack(spacing: LumiSpacing.large) {
                            reviewFact("USB analysis revision", compactRevision(track.incomingAnalysisRevision))
                            reviewFact("Lumi analysis revision", compactRevision(track.activeAnalysisRevision))
                            reviewFact("USB metadata revision", compactRevision(track.incomingMetadataRevision))
                        }
                        if let components = track.components {
                            LazyVGrid(
                                columns: [GridItem(.adaptive(minimum: 150), spacing: LumiSpacing.small)],
                                spacing: LumiSpacing.small
                            ) {
                                reviewComponent("Beatgrid", components.beatGrid, "metronome")
                                reviewComponent("Cue Points", components.cuePoints, "mappin")
                                reviewComponent("File Data", components.fileData, "doc.fill")
                                reviewComponent("RB Phrases", components.rekordboxPhrases, "rectangle.split.3x1")
                                reviewComponent("Waveform", components.waveform, "waveform")
                            }
                        } else {
                            Label("Reconnect or refresh this USB to calculate the component-level differences.", systemImage: "arrow.clockwise")
                                .font(LumiTypography.caption)
                                .foregroundStyle(LumiColor.textSecondary)
                        }
                        reviewActions(device: device, track: track)
                    }
                    .padding(LumiSpacing.medium)
                    .background(LumiColor.surfaceElevated)
                    .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
                }
            }
        }
        .padding(LumiSpacing.medium)
        .background(LumiColor.warning.opacity(0.08))
        .overlay {
            RoundedRectangle(cornerRadius: LumiRadius.control)
                .stroke(LumiColor.warning.opacity(0.45), lineWidth: 1)
        }
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        .accessibilityIdentifier("lumi.library.sources.usb.reviewTracks")
    }

    @ViewBuilder
    private func reviewActions(
        device: RekordboxDeviceState,
        track: RekordboxDeviceReviewTrackState
    ) -> some View {
        let key = reviewKey(device, track)
        if resolvingUSBReviewKey == key {
            HStack(spacing: LumiSpacing.small) {
                ProgressView().controlSize(.small)
                Text("Saving this review choice…")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.accent)
            }
            .frame(minHeight: 30, alignment: .leading)
            .accessibilityIdentifier("lumi.library.sources.usb.reviewSaving")
        } else {
            VStack(alignment: .leading, spacing: LumiSpacing.small) {
                HStack(spacing: LumiSpacing.small) {
                    Button("Ignore This Time") {
                        failedUSBReviewKey = nil
                        ignoredUSBReviews.insert(key)
                    }
                    .buttonStyle(.bordered)
                    .help("Hide this item until you reopen this screen. Nothing is saved or synchronized.")
                    if let root = mountedURL(for: device)?.path,
                       let activeRevision = track.activeAnalysisRevision {
                        Button("Do Not Sync to Lumi") {
                            failedUSBReviewKey = nil
                            resolvingUSBReviewKey = key
                            onDeviceConflictResolution(
                                conflictRequest(
                                    root: root,
                                    device: device,
                                    track: track,
                                    activeRevision: activeRevision,
                                    choice: .keepLumi
                                )
                            )
                        }
                        .buttonStyle(.bordered)
                        .disabled(usbOperation.isActive || !rendersInteractiveControls)
                        .help("Permanently keep Lumi for this exact USB analysis revision. A later USB change will be reviewed again.")
                        Button("Sync to Lumi & Overwrite") {
                            failedUSBReviewKey = nil
                            pendingUSBVersionRequest = conflictRequest(
                                root: root,
                                device: device,
                                track: track,
                                activeRevision: activeRevision,
                                choice: .useUSB
                            )
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(usbOperation.isActive || !rendersInteractiveControls)
                        .help("Replace the imported Rekordbox projection after a final revision check. Lumi phrases and AutoLoops stay intact.")
                    } else {
                        Text("Connect and refresh this USB to apply a saved choice.")
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                }
                if failedUSBReviewKey == key {
                    Label(usbOperation.detail, systemImage: "exclamationmark.triangle.fill")
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.warning)
                        .accessibilityIdentifier("lumi.library.sources.usb.reviewFailure")
                }
            }
        }
    }

    private func reviewKey(
        _ device: RekordboxDeviceState,
        _ track: RekordboxDeviceReviewTrackState
    ) -> String {
        "\(device.sourceID):\(track.deviceTrackID):\(track.incomingAnalysisRevision)"
    }

    private func conflictRequest(
        root: String,
        device: RekordboxDeviceState,
        track: RekordboxDeviceReviewTrackState,
        activeRevision: String,
        choice: USBConflictResolutionChoice
    ) -> USBConflictResolutionRequest {
        USBConflictResolutionRequest(
            root: root,
            sourceID: device.sourceID,
            deviceTrackID: track.deviceTrackID,
            expectedIncomingRevision: track.incomingAnalysisRevision,
            expectedActiveRevision: activeRevision,
            choice: choice
        )
    }

    private func formattedReviewDate(_ value: String?) -> String {
        guard let value, !value.isEmpty else { return "Unknown" }
        return value
    }

    private func compactRevision(_ value: String?) -> String {
        guard let value, !value.isEmpty else { return "Unknown" }
        guard value.count > 18 else { return value }
        return "\(value.prefix(8))…\(value.suffix(8))"
    }

    private func reviewFact(_ title: String, _ detail: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title.uppercased())
                .font(LumiTypography.technical)
                .foregroundStyle(LumiColor.textSecondary)
            Text(detail)
                .font(LumiTypography.caption.weight(.semibold))
                .foregroundStyle(LumiColor.textPrimary)
                .lineLimit(2)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func reviewComponent(
        _ title: String,
        _ component: RekordboxDeviceReviewComponentState,
        _ icon: String
    ) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Label(title, systemImage: icon)
                .font(LumiTypography.caption.weight(.semibold))
                .foregroundStyle(component.changed ? LumiColor.warning : LumiColor.success)
            Text(component.changed ? "CHANGED" : "UNCHANGED")
                .font(LumiTypography.technical)
                .foregroundStyle(component.changed ? LumiColor.warning : LumiColor.success)
            Text(component.detail)
                .font(LumiTypography.technical)
                .foregroundStyle(LumiColor.textSecondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(LumiSpacing.small)
        .frame(maxWidth: .infinity, minHeight: 92, alignment: .topLeading)
        .background(LumiColor.surface)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
    }

    private func devicePlaylistSelection(
        _ inspection: RekordboxDeviceInspectionState
    ) -> some View {
        VStack(alignment: .leading, spacing: LumiSpacing.medium) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Playlists to load into Lumi").font(LumiTypography.cardTitle)
                    Text("\(inspection.playlistCount) available · \(inspection.trackCount) tracks on USB · \(selectedUSBPlaylistIDs.count) selected")
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                Spacer()
                Button("Clear") {
                    selectedUSBPlaylistIDs.removeAll()
                    persistDevicePlaylistSelection()
                }
                .buttonStyle(.borderless)
                .disabled(selectedUSBPlaylistIDs.isEmpty)
                Button("Select All") {
                    selectedUSBPlaylistIDs = Set(inspection.playlists.map(\.id))
                    persistDevicePlaylistSelection()
                }
                .buttonStyle(.borderless)
            }
            TextField("Search playlists", text: $usbPlaylistSearch)
                .textFieldStyle(.roundedBorder)
                .accessibilityIdentifier("lumi.library.sources.usb.playlistSearch")
            selectionImpact(inspection)
            ScrollView {
                LazyVStack(spacing: LumiSpacing.xSmall) {
                    ForEach(
                        usbPlaylistOutlineRows(
                            playlists: inspection.playlists,
                            expandedFolderPaths: expandedUSBPlaylistFolderPaths,
                            search: usbPlaylistSearch
                        )
                    ) { row in
                        switch row.kind {
                        case let .folder(path, name, playlistCount, trackCount):
                            usbPlaylistFolderRow(
                                path: path,
                                name: name,
                                playlistCount: playlistCount,
                                trackCount: trackCount,
                                depth: row.depth,
                                forceExpanded: !usbPlaylistSearch
                                    .trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                            )
                        case let .playlist(playlist):
                            usbPlaylistRow(
                                playlist,
                                previouslySynchronized: inspection.selectedPlaylistIDs.contains(playlist.id),
                                depth: row.depth
                            )
                        }
                    }
                }
            }
            .frame(minHeight: 160, maxHeight: 360)
            if usbPlaylistOutlineRows(
                playlists: inspection.playlists,
                expandedFolderPaths: expandedUSBPlaylistFolderPaths,
                search: usbPlaylistSearch
            ).isEmpty {
                Text("No playlists match this search.")
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            Text("Browse playlists and track status before sync. Only selected playlists are synchronized; duplicate tracks are processed once.")
                .font(LumiTypography.caption)
                .foregroundStyle(LumiColor.textSecondary)
        }
    }

    private func usbPlaylistFolderRow(
        path: String,
        name: String,
        playlistCount: Int,
        trackCount: UInt64,
        depth: Int,
        forceExpanded: Bool
    ) -> some View {
        let expanded = forceExpanded || expandedUSBPlaylistFolderPaths.contains(path)
        return Button {
            if expandedUSBPlaylistFolderPaths.contains(path) {
                expandedUSBPlaylistFolderPaths.remove(path)
            } else {
                expandedUSBPlaylistFolderPaths.insert(path)
            }
        } label: {
            HStack(spacing: LumiSpacing.small) {
                Image(systemName: expanded ? "chevron.down" : "chevron.right")
                    .frame(width: 16)
                Image(systemName: expanded ? "folder.fill" : "folder")
                    .foregroundStyle(LumiColor.accent)
                Text(name)
                    .font(LumiTypography.body.weight(.semibold))
                    .foregroundStyle(LumiColor.textPrimary)
                    .lineLimit(1)
                Spacer()
                Text("\(playlistCount) playlist\(playlistCount == 1 ? "" : "s") · \(trackCount) tracks")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            .padding(.leading, CGFloat(depth) * 20 + LumiSpacing.medium)
            .padding(.trailing, LumiSpacing.medium)
            .frame(minHeight: 38)
            .background(LumiColor.surface)
            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(path)
        .accessibilityIdentifier("lumi.library.sources.usb.folder.\(path)")
    }

    private func usbPlaylistRow(
        _ playlist: RekordboxDevicePlaylistState,
        previouslySynchronized: Bool,
        depth: Int
    ) -> some View {
        VStack(spacing: 0) {
            HStack(spacing: LumiSpacing.small) {
                Button {
                    if selectedUSBPlaylistIDs.contains(playlist.id) {
                        selectedUSBPlaylistIDs.remove(playlist.id)
                    } else {
                        selectedUSBPlaylistIDs.insert(playlist.id)
                    }
                    persistDevicePlaylistSelection()
                } label: {
                    Image(systemName: selectedUSBPlaylistIDs.contains(playlist.id) ? "checkmark.square.fill" : "square")
                        .foregroundStyle(selectedUSBPlaylistIDs.contains(playlist.id) ? LumiColor.accent : LumiColor.textSecondary)
                        .frame(width: 28, height: 36)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                Button {
                    if expandedUSBPlaylistIDs.contains(playlist.id) {
                        expandedUSBPlaylistIDs.remove(playlist.id)
                    } else {
                        expandedUSBPlaylistIDs.insert(playlist.id)
                    }
                } label: {
                    HStack(spacing: LumiSpacing.small) {
                        Image(systemName: "music.note.list")
                            .foregroundStyle(LumiColor.textSecondary)
                        Text(playlist.name)
                            .font(LumiTypography.body.weight(.semibold))
                            .foregroundStyle(LumiColor.textPrimary)
                            .lineLimit(1)
                            .truncationMode(.middle)
                        if previouslySynchronized {
                            Text("SYNCED")
                                .font(LumiTypography.technical)
                                .foregroundStyle(LumiColor.accent)
                        }
                        Spacer()
                        playlistStatusSummary(playlist.statusCounts)
                        Text("\(playlist.trackCount)")
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.textSecondary)
                        Image(systemName: expandedUSBPlaylistIDs.contains(playlist.id) ? "chevron.down" : "chevron.right")
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
            .padding(.leading, CGFloat(depth) * 20 + LumiSpacing.medium)
            .padding(.trailing, LumiSpacing.medium)
            .frame(minHeight: 40)
            if expandedUSBPlaylistIDs.contains(playlist.id) {
                Divider()
                LazyVStack(spacing: 0) {
                    ForEach(playlist.tracks) { track in
                        deviceTrackRow(track)
                    }
                }
                .padding(.leading, CGFloat(depth) * 20 + 44)
            }
        }
        .background(
            selectedUSBPlaylistIDs.contains(playlist.id)
                ? LumiColor.accent.opacity(0.10)
                : LumiColor.surfaceElevated
        )
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        .help(playlist.path)
        .accessibilityIdentifier("lumi.library.sources.usb.playlist.\(playlist.id)")
    }

    @ViewBuilder
    private func selectionImpact(_ inspection: RekordboxDeviceInspectionState) -> some View {
        if selectedUSBPlaylistIDs.isEmpty {
            Label(
                "Select one or more playlists to calculate their impact before synchronization.",
                systemImage: "checklist"
            )
            .font(LumiTypography.caption)
            .foregroundStyle(LumiColor.textSecondary)
            .padding(LumiSpacing.medium)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(LumiColor.surfaceElevated)
            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
        } else {
            let impact = USBPlaylistSelectionImpact(
                inspection: inspection,
                selectedPlaylistIDs: selectedUSBPlaylistIDs
            )
            VStack(alignment: .leading, spacing: LumiSpacing.medium) {
                HStack {
                    Label("Selection impact", systemImage: "checkmark.shield.fill")
                        .font(LumiTypography.cardTitle)
                    Spacer()
                    Text("ANALYSIS COMPLETE · NO CHANGES APPLIED")
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.success)
                }
                HStack(spacing: LumiSpacing.small) {
                    impactMetric("Selected", impact.uniqueTrackCount, .empty)
                    impactMetric("New", impact.notInLumiCount, .ready)
                    impactMetric("Update", impact.usbNewerCount, .ready)
                    impactMetric("Current", impact.currentCount, .ready)
                    impactMetric("Protected", impact.usbOutdatedCount, .stale)
                    impactMetric("Review", impact.conflictCount, .degraded)
                }
                Text(
                    "Sync will add or update \(impact.changedCount) unique track\(impact.changedCount == 1 ? "" : "s"). "
                        + "\(impact.heldCount) older or incomparable version\(impact.heldCount == 1 ? "" : "s") will be held; Lumi-owned phrases and AutoLoop choices remain unchanged."
                )
                .font(LumiTypography.caption)
                .foregroundStyle(LumiColor.textSecondary)
            }
            .padding(LumiSpacing.medium)
            .background(LumiColor.accent.opacity(0.08))
            .overlay {
                RoundedRectangle(cornerRadius: LumiRadius.control)
                    .stroke(LumiColor.accent.opacity(0.55), lineWidth: 1)
            }
            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
            .accessibilityIdentifier("lumi.library.sources.usb.selectionImpact")
        }
    }

    private func impactMetric(
        _ title: String,
        _ value: Int,
        _ state: LumiComponentState
    ) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title.uppercased())
                .font(LumiTypography.technical)
                .foregroundStyle(LumiColor.textSecondary)
            Text(value.formatted())
                .font(LumiTypography.body.monospacedDigit().weight(.semibold))
                .foregroundStyle(value == 0 ? LumiColor.textSecondary : state.color)
        }
        .padding(.horizontal, LumiSpacing.medium)
        .padding(.vertical, LumiSpacing.small)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(LumiColor.surfaceElevated)
        .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
    }

    private func playlistStatusSummary(
        _ counts: RekordboxDeviceStatusCounts
    ) -> some View {
        HStack(spacing: LumiSpacing.small) {
            if counts.current > 0 { compactStatus("\(counts.current) CURRENT", .ready) }
            if counts.usbNewer > 0 { compactStatus("\(counts.usbNewer) USB NEWER", .ready) }
            if counts.usbOutdated > 0 { compactStatus("\(counts.usbOutdated) USB OUTDATED", .stale) }
            if counts.conflict > 0 { compactStatus("\(counts.conflict) REVIEW", .degraded) }
            if counts.notInLumi > 0 { compactStatus("\(counts.notInLumi) NEW", .empty) }
        }
    }

    private func deviceTrackRow(_ track: RekordboxDeviceTrackState) -> some View {
        HStack(spacing: LumiSpacing.medium) {
            VStack(alignment: .leading, spacing: 2) {
                Text(track.title)
                    .font(LumiTypography.body)
                    .foregroundStyle(LumiColor.textPrimary)
                Text(track.artist.isEmpty ? "Unknown artist" : track.artist)
                    .font(LumiTypography.caption)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            Spacer()
            Text(String(format: "%.2f BPM", Double(track.bpmMilli) / 1_000))
                .font(LumiTypography.technical)
                .foregroundStyle(LumiColor.textSecondary)
                .frame(width: 82, alignment: .trailing)
            deviceTrackStatus(track.status)
        }
        .padding(.horizontal, LumiSpacing.medium)
        .frame(minHeight: 42)
        .background(LumiColor.canvas.opacity(0.55))
        .help(track.detail)
    }

    @ViewBuilder
    private func deviceTrackStatus(_ status: String) -> some View {
        switch status {
        case "current": compactStatus("CURRENT", .ready)
        case "usb-newer": compactStatus("USB NEWER", .ready)
        case "usb-outdated": compactStatus("USB OUTDATED", .stale)
        case "not-in-lumi": compactStatus("NEW", .empty)
        default: compactStatus("REVIEW", .degraded)
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
            Text("Import & Sources")
                .font(LumiTypography.screenTitle)
            Text("Manage trusted USB sources, safe synchronization and source-specific initial phrase mapping.")
                .font(LumiTypography.body)
                .foregroundStyle(LumiColor.textSecondary)
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

    private var selectedUSBSource: RekordboxDeviceState? {
        visibleUSBDevices.first { $0.sourceID == selectedUSBSourceID }
    }

    private var provisionalUSBInspection: RekordboxDeviceInspectionState? {
        guard let inspection = library.rekordboxDeviceInspection else { return nil }
        let alreadyTrusted = visibleUSBDevices.contains {
            USBSourceIdentityResolver.inspection(inspection, matches: $0)
        }
        return alreadyTrusted ? nil : inspection
    }

    private var deviceSyncButtonTitle: String {
        if usbOperation.phase == .synchronizing { return "Synchronizing…" }
        return "Sync \(selectedUSBPlaylistIDs.count) Playlist\(selectedUSBPlaylistIDs.count == 1 ? "" : "s")"
    }

    private var visibleUSBDevices: [RekordboxDeviceState] {
        let stableDevices = library.rekordboxDevices.filter {
            !$0.sourceID.hasPrefix("usb-volume:")
        }
        let stableKeys = Set(stableDevices.map(deviceIdentityKey))
        var newestLegacyByKey: [String: RekordboxDeviceState] = [:]
        for device in library.rekordboxDevices where device.sourceID.hasPrefix("usb-volume:") {
            let key = deviceIdentityKey(device)
            guard !stableKeys.contains(key) else { continue }
            if let current = newestLegacyByKey[key], current.syncedAt >= device.syncedAt {
                continue
            }
            newestLegacyByKey[key] = device
        }
        return (stableDevices + newestLegacyByKey.values).sorted {
            if $0.displayName == $1.displayName { return $0.sourceID < $1.sourceID }
            return $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending
        }
    }

    private func deviceIdentityKey(_ device: RekordboxDeviceState) -> String {
        "\(device.displayName.lowercased())|\(device.databaseRevision)"
    }

    private var activeDeviceInspection: RekordboxDeviceInspectionState? {
        guard let inspection = library.rekordboxDeviceInspection else { return nil }
        if inspection.sourceID == selectedUSBSourceID { return inspection }
        guard let selectedUSBSource else { return nil }
        guard USBSourceIdentityResolver.inspection(inspection, matches: selectedUSBSource) else {
            return nil
        }
        return inspection
    }

    private func restoreDevicePlaylistSelection() {
        guard let sourceID = selectedUSBSourceID else {
            selectedUSBPlaylistIDs = []
            return
        }
        let storedSelections = decodedDevicePlaylistSelections()[sourceID]
        let stored = storedSelections ?? activeDeviceInspection?.selectedPlaylistIDs ?? []
        if let inspection = activeDeviceInspection {
            let available = Set(inspection.playlists.map(\.id))
            selectedUSBPlaylistIDs = Set(stored).intersection(available)
        } else {
            selectedUSBPlaylistIDs = Set(stored)
        }
    }

    private func persistDevicePlaylistSelection() {
        guard let sourceID = selectedUSBSourceID else { return }
        var selections = decodedDevicePlaylistSelections()
        selections[sourceID] = selectedUSBPlaylistIDs.sorted()
        guard let data = try? JSONEncoder().encode(selections),
              let encoded = String(data: data, encoding: .utf8) else { return }
        devicePlaylistSelectionsJSON = encoded
    }

    private func decodedDevicePlaylistSelections() -> [String: [UInt32]] {
        guard let data = devicePlaylistSelectionsJSON.data(using: .utf8),
              let decoded = try? JSONDecoder().decode([String: [UInt32]].self, from: data) else {
            return [:]
        }
        return decoded
    }

    private func syncSelectedDevicePlaylists(root: String) {
        guard !selectedUSBPlaylistIDs.isEmpty else { return }
        persistDevicePlaylistSelection()
        let url = URL(fileURLWithPath: root, isDirectory: true)
        let sourceID = stableSourceID(
            for: url,
            preferredSourceID: selectedUSBSourceID
        )
        selectedUSBSourceID = sourceID
        onDeviceSync(root, sourceID, selectedUSBPlaylistIDs.sorted())
    }

    private var mountedTrustedSources: [RekordboxDeviceState] {
        _ = mountRevision
        return visibleUSBDevices.filter { mountedURL(for: $0) != nil }
    }

    private func inspectMountedTrustedSource(_ url: URL) {
        guard FileManager.default.fileExists(
            atPath: url.appendingPathComponent("PIONEER/rekordbox/exportLibrary.db").path
        ) else { return }
        let volume = mountedIdentity(url)
        guard let sourceID = USBSourceIdentityResolver.selectedSourceID(
            for: volume,
            devices: visibleUSBDevices
        ), let device = visibleUSBDevices.first(where: { $0.sourceID == sourceID }) else {
            return
        }
        // A mount updates connection status, but never opens another source
        // lane or moves the page underneath the user.
        guard selectedUSBSourceID == device.sourceID else { return }
        rekordboxDeviceRoot = url.path
        onDeviceInspect(url.path, device.sourceID)
    }

    private var overallUSBState: LumiComponentState {
        if visibleUSBDevices.contains(where: { $0.conflictTracks > 0 }) { return .degraded }
        if !mountedTrustedSources.isEmpty { return .ready }
        return visibleUSBDevices.isEmpty ? .empty : .stale
    }

    private func mountedURL(for device: RekordboxDeviceState) -> URL? {
        if let bookmarked = bookmarkedDeviceURL(sourceID: device.sourceID),
           FileManager.default.fileExists(
               atPath: bookmarked.appendingPathComponent(
                   "PIONEER/rekordbox/exportLibrary.db"
               ).path
           ) {
            return bookmarked
        }
        return FileManager.default.mountedVolumeURLs(
            includingResourceValuesForKeys: [.volumeNameKey],
            options: [.skipHiddenVolumes]
        )?.first { url in
            let sourceID = USBSourceIdentityResolver.selectedSourceID(
                for: mountedIdentity(url),
                devices: visibleUSBDevices
            )
            return sourceID == device.sourceID
                && FileManager.default.fileExists(
                    atPath: url.appendingPathComponent("PIONEER/rekordbox/exportLibrary.db").path
                )
        }
    }

    private func volumeSourceID(_ url: URL) -> String? {
        let stable = try? url.resourceValues(forKeys: [.volumeUUIDStringKey]).volumeUUIDString
        return USBStableSourceIdentity.sourceID(
            fileSystemUUID: stable,
            displayName: volumeDisplayName(url),
            hardwareSerial: USBStableSourceIdentity.hardwareSerial(for: url)
        )
    }

    private func mountedIdentity(_ url: URL) -> MountedUSBIdentity {
        return MountedUSBIdentity(
            sourceID: volumeSourceID(url),
            displayName: volumeDisplayName(url)
        )
    }

    private func deviceSyncState(_ device: RekordboxDeviceState) -> LumiComponentState {
        if device.conflictTracks > 0 { return .degraded }
        if device.protectedTracks > 0 { return .stale }
        return .ready
    }

    private func deviceSyncLabel(_ device: RekordboxDeviceState) -> String {
        if device.conflictTracks > 0 { return "\(device.conflictTracks) REVIEW" }
        if device.protectedTracks > 0 { return "\(device.protectedTracks) OLDER HELD" }
        return "CURRENT"
    }

    private func compactStatus(_ label: String, _ state: LumiComponentState) -> some View {
        HStack(spacing: 6) {
            Circle().fill(state.color).frame(width: 7, height: 7)
            Text(label).font(LumiTypography.technical)
        }
        .foregroundStyle(state.color)
        .padding(.horizontal, 9)
        .padding(.vertical, 5)
        .background(state.color.opacity(0.1))
        .clipShape(Capsule())
    }

    private func metric(_ title: String, _ value: UInt64) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text("\(value)").font(LumiTypography.cardTitle)
            Text(title.uppercased())
                .font(LumiTypography.technical)
                .foregroundStyle(LumiColor.textSecondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func shortRevision(_ revision: String) -> String {
        revision.count > 14 ? "\(revision.prefix(12))…" : revision
    }

    private func formattedSyncDate(_ value: String) -> String {
        guard value != "Unknown" else { return value }
        return value.replacingOccurrences(of: "T", with: " ").replacingOccurrences(of: "Z", with: "")
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

    private func chooseRekordboxDevice() {
        let choices = mountedRekordboxUSBs()
        usbSelectionFeedback = nil
        guard !choices.isEmpty else {
            usbSelectionFeedback = "No connected Rekordbox OneLibrary USB source was found. Connect the USB and try again."
            return
        }
        guard choices.count > 1 else {
            inspectRekordboxDevice(at: choices[0])
            return
        }
        mountedUSBChoices = choices
        isUSBSourceChoicePresented = true
    }

    private func inspectRekordboxDevice(at url: URL) {
        usbSelectionFeedback = nil
        let selected = USBSourceIdentityResolver.selectedSourceID(
            for: mountedIdentity(url),
            devices: visibleUSBDevices
        )
        guard let sourceID = stableSourceID(for: url, preferredSourceID: selected),
              let authorizedURL = authorizedDeviceURL(
                expected: url,
                sourceID: sourceID
              ) else { return }
        rekordboxDeviceRoot = authorizedURL.path
        selectedUSBSourceID = sourceID
        selectedUSBPlaylistIDs = []
        usbPlaylistSearch = ""
        onDeviceInspect(authorizedURL.path, selectedUSBSourceID)
    }

    private func inspectTrustedDevice(_ device: RekordboxDeviceState, root: String) {
        usbSelectionFeedback = nil
        let url = URL(fileURLWithPath: root, isDirectory: true)
        guard let authorizedURL = authorizedDeviceURL(
            expected: url,
            sourceID: device.sourceID
        ) else { return }
        selectedUSBSourceID = stableSourceID(
            for: authorizedURL,
            preferredSourceID: device.sourceID
        )
        rekordboxDeviceRoot = authorizedURL.path
        onDeviceInspect(authorizedURL.path, selectedUSBSourceID)
    }

    private func authorizedDeviceURL(expected: URL, sourceID: String) -> URL? {
        var bookmarks = decodedDeviceBookmarks()
        if let encodedBookmark = bookmarks[sourceID],
           let bookmark = Data(base64Encoded: encodedBookmark) {
            var stale = false
            if let resolved = try? URL(
                resolvingBookmarkData: bookmark,
                options: [.withSecurityScope, .withoutUI],
                relativeTo: nil,
                bookmarkDataIsStale: &stale
            ), !stale,
               resolved.standardizedFileURL.path == expected.standardizedFileURL.path {
                return resolved
            }

            // A re-formatted or re-mounted USB can invalidate its bookmark.
            // Remove the stale grant before asking for authorization again so
            // the trusted-source row never gets trapped in a retry loop.
            bookmarks.removeValue(forKey: sourceID)
            if let data = try? JSONEncoder().encode(bookmarks),
               let encoded = String(data: data, encoding: .utf8) {
                deviceBookmarksJSON = encoded
            }
        }

        let panel = NSOpenPanel()
        panel.title = "Authorize Rekordbox USB"
        panel.message = "Select \(volumeDisplayName(expected)) once. Lumi stores secure read-only access for future reconnects."
        panel.prompt = "Authorize USB"
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.canCreateDirectories = false
        panel.directoryURL = expected.deletingLastPathComponent()
        guard panel.runModal() == .OK, let selected = panel.url else { return nil }
        let database = selected.appendingPathComponent("PIONEER/rekordbox/exportLibrary.db")
        guard FileManager.default.fileExists(atPath: database.path) else {
            usbSelectionFeedback = "The selected folder is not a Rekordbox OneLibrary USB source."
            return nil
        }
        let selectedSourceID = stableSourceID(
            for: selected,
            preferredSourceID: USBSourceIdentityResolver.selectedSourceID(
                for: mountedIdentity(selected),
                devices: visibleUSBDevices
            )
        )
        guard selectedSourceID == sourceID else {
            usbSelectionFeedback = "Select \(volumeDisplayName(expected)); another trusted USB was chosen."
            return nil
        }
        do {
            let bookmark = try selected.bookmarkData(
                options: .withSecurityScope,
                includingResourceValuesForKeys: nil,
                relativeTo: nil
            )
            bookmarks = decodedDeviceBookmarks()
            bookmarks[sourceID] = bookmark.base64EncodedString()
            let data = try JSONEncoder().encode(bookmarks)
            guard let encoded = String(data: data, encoding: .utf8) else { return nil }
            deviceBookmarksJSON = encoded
            usbSelectionFeedback = nil
            return selected
        } catch {
            usbSelectionFeedback = "Lumi could not retain read-only access to this USB. Choose it again."
            return nil
        }
    }

    private func decodedDeviceBookmarks() -> [String: String] {
        guard let data = deviceBookmarksJSON.data(using: .utf8),
              let decoded = try? JSONDecoder().decode([String: String].self, from: data) else {
            return [:]
        }
        return decoded
    }

    private func bookmarkedDeviceURL(sourceID: String) -> URL? {
        guard let encodedBookmark = decodedDeviceBookmarks()[sourceID],
              let bookmark = Data(base64Encoded: encodedBookmark) else { return nil }
        var stale = false
        guard let resolved = try? URL(
            resolvingBookmarkData: bookmark,
            options: [.withSecurityScope, .withoutUI],
            relativeTo: nil,
            bookmarkDataIsStale: &stale
        ), !stale else { return nil }
        return resolved
    }

    private func stableSourceID(for url: URL, preferredSourceID: String?) -> String? {
        let displayName = volumeDisplayName(url)
        let collisionSafePreferredID = preferredSourceID.flatMap { sourceID in
            guard let existing = visibleUSBDevices.first(where: { $0.sourceID == sourceID }) else {
                return sourceID
            }
            // Equal-model FAT disks can publish the same UUID/serial. Reuse an
            // existing identity only when its trusted source label also
            // matches; otherwise allocate an independent local identity.
            return existing.displayName.caseInsensitiveCompare(displayName) == .orderedSame
                ? sourceID
                : nil
        }
        return collisionSafePreferredID ?? USBLocalSourceIdentity.generated()
    }

    private func mountedRekordboxUSBs() -> [URL] {
        FileManager.default.mountedVolumeURLs(
            includingResourceValuesForKeys: [.volumeNameKey],
            options: [.skipHiddenVolumes]
        )?
        .filter {
            FileManager.default.fileExists(
                atPath: $0.appendingPathComponent("PIONEER/rekordbox/exportLibrary.db").path
            )
        }
        .sorted {
            volumeDisplayName($0).localizedCaseInsensitiveCompare(volumeDisplayName($1))
                == .orderedAscending
        } ?? []
    }

    private func volumeDisplayName(_ url: URL) -> String {
        (try? url.resourceValues(forKeys: [.volumeNameKey]).volumeName)
            ?? url.lastPathComponent
    }

}
