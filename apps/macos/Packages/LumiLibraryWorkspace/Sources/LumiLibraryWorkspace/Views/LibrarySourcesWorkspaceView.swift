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
    private let onSyncPreview: @Sendable (RekordboxXMLSyncPreviewRequest) -> Void
    private let onSyncApply: @Sendable (RekordboxXMLSyncPreviewRequest, String) -> Void
    private let onAnalysisImport: @Sendable (RekordboxXMLSyncPreviewRequest, String) -> Void
    private let onDeviceInspect: @Sendable (String) -> Void
    private let onDeviceSync: @Sendable (String, [UInt32]) -> Void

    @AppStorage(LumiPreferenceKey.rekordboxXMLFolder)
    private var rekordboxFolderPath = ""
    @AppStorage(LumiPreferenceKey.rekordboxXMLIncludeFutureChildren)
    private var includeFutureChildPlaylists = true
    @AppStorage(LumiPreferenceKey.rekordboxXMLFollowedPaths)
    private var followedPathsJSON = "[]"
    @AppStorage(LumiPreferenceKey.rekordboxDeviceRoot)
    private var rekordboxDeviceRoot = ""
    @AppStorage(LumiPreferenceKey.rekordboxDevicePlaylistSelections)
    private var devicePlaylistSelectionsJSON = "{}"

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
    @State private var selectedUSBSourceID: String?
    @State private var selectedUSBPlaylistIDs: Set<UInt32> = []
    @State private var expandedUSBPlaylistIDs: Set<UInt32> = []
    @State private var expandedUSBPlaylistFolderPaths: Set<String> = []
    @State private var usbPlaylistSearch = ""
    @State private var mountRevision = 0
    @State private var mountedUSBChoices: [URL] = []
    @State private var isUSBSourceChoicePresented = false
    @State private var usbSelectionFeedback: String?

    public init(
        library: LibraryWorkspaceState,
        settings: PhraseRoleSettingsState?,
        feedback: String? = nil,
        syncFeedback: String? = nil,
        syncFeedbackIsError: Bool = false,
        usbOperation: USBSourceOperationState = .idle,
        rendersInteractiveControls: Bool = true,
        onMutation: @escaping @Sendable (PhraseRoleMutationRequest) -> Void = { _ in },
        onSyncPreview: @escaping @Sendable (RekordboxXMLSyncPreviewRequest) -> Void = { _ in },
        onSyncApply: @escaping @Sendable (RekordboxXMLSyncPreviewRequest, String) -> Void = { _, _ in },
        onAnalysisImport: @escaping @Sendable (RekordboxXMLSyncPreviewRequest, String) -> Void = { _, _ in },
        onDeviceInspect: @escaping @Sendable (String) -> Void = { _ in },
        onDeviceSync: @escaping @Sendable (String, [UInt32]) -> Void = { _, _ in }
    ) {
        self.library = library
        self.settings = settings
        self.feedback = feedback
        self.syncFeedback = syncFeedback
        self.syncFeedbackIsError = syncFeedbackIsError
        self.usbOperation = usbOperation
        self.rendersInteractiveControls = rendersInteractiveControls
        self.onMutation = onMutation
        self.onSyncPreview = onSyncPreview
        self.onSyncApply = onSyncApply
        self.onAnalysisImport = onAnalysisImport
        self.onDeviceInspect = onDeviceInspect
        self.onDeviceSync = onDeviceSync
        _selectedProviderKind = State(initialValue: settings?.mappingProfiles.first?.providerKind)
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: LumiSpacing.xLarge) {
                header
                usbMediaSummary
                if let usbSelectionFeedback {
                    Label(usbSelectionFeedback, systemImage: "exclamationmark.triangle.fill")
                        .font(LumiTypography.caption.weight(.semibold))
                        .foregroundStyle(LumiColor.warning)
                        .accessibilityIdentifier("lumi.library.sources.usb.selectionFeedback")
                }
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
        .onChange(of: library.rekordboxDeviceInspection) { _, _ in
            expandedUSBPlaylistIDs.removeAll()
            expandedUSBPlaylistFolderPaths.removeAll()
            restoreDevicePlaylistSelection()
        }
        .onAppear {
            guard !didInitializeSource else { return }
            didInitializeSource = true
            restoreFollowedPaths()
            if !rekordboxFolderPath.isEmpty { scanImportFolder() }
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
            HStack(spacing: LumiSpacing.xLarge) {
                sourceIcon("externaldrive.fill.badge.checkmark", state: overallUSBState)
                VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                    Text("USB Sources").font(LumiTypography.cardTitle)
                    Text("\(visibleUSBDevices.count) trusted · \(mountedTrustedSources.count) connected · read only")
                        .font(LumiTypography.technical)
                        .foregroundStyle(LumiColor.textSecondary)
                    Text("Trusted media identifies live Pro DJ Link tracks. Older or incomparable backup analysis is registered but never replaces newer active Lumi data.")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                }
                Spacer()
                Button("Add USB Source…") { chooseRekordboxDevice() }
                    .buttonStyle(.borderedProminent)
                    .disabled(!rendersInteractiveControls)
                    .accessibilityIdentifier("lumi.library.sources.usb.add")
            }
        }
    }

    private var trustedUSBSources: some View {
        VStack(alignment: .leading, spacing: LumiSpacing.medium) {
            HStack {
                Text("Trusted USB Sources").font(LumiTypography.sectionTitle)
                Spacer()
                Text("SOURCE  ·  CONNECTION  ·  SYNC HEALTH")
                    .font(LumiTypography.technical)
                    .foregroundStyle(LumiColor.textSecondary)
            }
            if !hasExpandedUSBSourceLane {
                usbOperationStatus
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
                if let root = mountedURL(for: device)?.path {
                    onDeviceInspect(root)
                }
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
                                onDeviceInspect(root)
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
                    usbOperationStatus
                    if let inspection = activeDeviceInspection {
                        Divider()
                        devicePlaylistSelection(inspection)
                    } else {
                        Divider()
                        storedDevicePlaylists(device)
                    }
                    Divider()
                    HStack(spacing: LumiSpacing.xLarge) {
                        metric("Synced", device.activeTracks)
                        metric("Matched", device.matchedTracks)
                        metric("Unmatched · held", device.unmatchedTracks)
                        metric("Current", device.currentTracks)
                        metric("Updated", device.promotedTracks)
                    }
                    if device.protectedTracks > 0 || device.conflictTracks > 0 {
                        Label(
                            "\(device.protectedTracks) older track version\(device.protectedTracks == 1 ? "" : "s") protected · \(device.conflictTracks) incomparable change\(device.conflictTracks == 1 ? "" : "s") held for review",
                            systemImage: "shield.lefthalf.filled"
                        )
                        .font(LumiTypography.caption.weight(.semibold))
                        .foregroundStyle(device.conflictTracks > 0 ? LumiColor.warning : LumiColor.success)
                    } else {
                        Label("No downgrade risk detected. Active Lumi analysis and all Lumi-owned phrases and AutoLoop choices are protected.", systemImage: "checkmark.shield.fill")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.success)
                    }
                    HStack(spacing: LumiSpacing.large) {
                        sourceSettingRow(title: "Database revision", detail: shortRevision(device.databaseRevision), systemImage: "cylinder")
                        sourceSettingRow(title: "Version policy", detail: "Newer promotes · older/unknown holds", systemImage: "arrow.up.arrow.down")
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
                    usbOperationStatus
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

    @ViewBuilder
    private var usbOperationStatus: some View {
        if usbOperation.phase != .idle {
            HStack(alignment: .top, spacing: LumiSpacing.medium) {
                if usbOperation.isActive {
                    ProgressView()
                        .controlSize(.small)
                        .padding(.top, 2)
                } else {
                    Image(systemName: usbOperation.phase == .failed
                        ? "exclamationmark.triangle.fill"
                        : "checkmark.circle.fill")
                        .foregroundStyle(usbOperation.phase == .failed
                            ? LumiColor.warning
                            : LumiColor.success)
                        .padding(.top, 2)
                }
                VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                    Text(usbOperation.title)
                        .font(LumiTypography.body.weight(.semibold))
                    Text(usbOperation.detail)
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                    if usbOperation.isActive {
                        ProgressView()
                            .progressViewStyle(.linear)
                            .accessibilityLabel(usbOperation.title)
                    } else if usbOperation.phase == .completed {
                        Text("Completed safely · the USB disk was not modified")
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.success)
                    }
                }
                Spacer()
            }
            .padding(LumiSpacing.medium)
            .background(
                (usbOperation.phase == .failed ? LumiColor.warning : LumiColor.accent)
                    .opacity(0.10)
            )
            .overlay {
                RoundedRectangle(cornerRadius: LumiRadius.control)
                    .stroke(
                        usbOperation.phase == .failed ? LumiColor.warning : LumiColor.border,
                        lineWidth: 1
                    )
            }
            .clipShape(RoundedRectangle(cornerRadius: LumiRadius.control))
            .accessibilityIdentifier("lumi.library.sources.usb.operation")
        }
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

    private var rekordboxDeviceSource: some View {
        LumiPanel {
            VStack(alignment: .leading, spacing: LumiSpacing.large) {
                HStack(alignment: .top, spacing: LumiSpacing.large) {
                    sourceIcon("externaldrive.fill.badge.checkmark", state: deviceSourceState)
                    VStack(alignment: .leading, spacing: LumiSpacing.xSmall) {
                        HStack {
                            Text("Rekordbox Device Library")
                                .font(LumiTypography.cardTitle)
                            Text("USB / SD · READ ONLY")
                                .font(LumiTypography.technical)
                                .padding(.horizontal, 8)
                                .padding(.vertical, 4)
                                .background(LumiColor.surfaceElevated)
                                .clipShape(Capsule())
                        }
                        Text(deviceSourceStatus)
                            .font(LumiTypography.technical)
                            .foregroundStyle(LumiColor.textSecondary)
                        Text("Syncs performance identity plus Rekordbox metadata, beatgrid, RGB waveform and analysis revisions. Lumi never writes to or ejects the device.")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.textSecondary)
                    }
                    Spacer()
                    HStack(spacing: LumiSpacing.small) {
                        Button(rekordboxDeviceRoot.isEmpty ? "Choose Device…" : "Change Device…") {
                            chooseRekordboxDevice()
                        }
                        .buttonStyle(.bordered)
                        .disabled(!rendersInteractiveControls)
                        .accessibilityIdentifier("lumi.library.sources.device.choose")
                        Button("Sync Device") {
                            onDeviceInspect(rekordboxDeviceRoot)
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(rekordboxDeviceRoot.isEmpty || !rendersInteractiveControls)
                        .accessibilityIdentifier("lumi.library.sources.device.sync")
                    }
                }
                if !rekordboxDeviceRoot.isEmpty {
                    Divider()
                    HStack(spacing: LumiSpacing.large) {
                        sourceSettingRow(
                            title: "Selected device",
                            detail: rekordboxDeviceRoot,
                            systemImage: "externaldrive"
                        )
                        sourceSettingRow(
                            title: "Refresh policy",
                            detail: "Metadata, beatgrid and cue revisions on every sync",
                            systemImage: "arrow.triangle.2.circlepath"
                        )
                    }
                    if let syncFeedback, isDeviceSyncFeedback {
                        Label(
                            syncFeedback,
                            systemImage: syncFeedbackIsError
                                ? "exclamationmark.triangle.fill"
                                : "checkmark.shield.fill"
                        )
                        .font(LumiTypography.caption)
                        .foregroundStyle(syncFeedbackIsError ? LumiColor.warning : LumiColor.success)
                    }
                    Text("Cue changes already invalidate the stored analysis revision. Cue markers will become visible in a later UI step.")
                        .font(LumiTypography.caption)
                        .foregroundStyle(LumiColor.textSecondary)
                }
            }
        }
        .accessibilityIdentifier("lumi.library.sources.device")
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
                    if let syncFeedback, !isDeviceSyncFeedback {
                        Label(syncFeedback, systemImage: "checkmark.shield.fill")
                            .font(LumiTypography.caption)
                            .foregroundStyle(LumiColor.success)
                    }
                }
            }
            .accessibilityIdentifier("lumi.library.sources.rekordbox.syncPreview")
        } else if let syncFeedback, !isDeviceSyncFeedback {
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

    private var hasExpandedUSBSourceLane: Bool {
        guard let selectedUSBSourceID else { return false }
        return visibleUSBDevices.contains { $0.sourceID == selectedUSBSourceID }
            || provisionalUSBInspection?.sourceID == selectedUSBSourceID
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
        onDeviceSync(root, selectedUSBPlaylistIDs.sorted())
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
        onDeviceInspect(url.path)
    }

    private var overallUSBState: LumiComponentState {
        if visibleUSBDevices.contains(where: { $0.conflictTracks > 0 }) { return .degraded }
        if !mountedTrustedSources.isEmpty { return .ready }
        return visibleUSBDevices.isEmpty ? .empty : .stale
    }

    private func mountedURL(for device: RekordboxDeviceState) -> URL? {
        FileManager.default.mountedVolumeURLs(
            includingResourceValuesForKeys: [.volumeNameKey],
            options: [.skipHiddenVolumes]
        )?.first { url in
            return USBSourceIdentityResolver.volume(mountedIdentity(url), matches: device)
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
        MountedUSBIdentity(
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
        if device.conflictTracks > 0 { return "REVIEW" }
        if device.protectedTracks > 0 { return "OLDER HELD" }
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

    private var rekordboxSourceStatus: String {
        if isScanning { return "Scanning import folder read-only…" }
        if let sourceError { return sourceError }
        guard let discovery else {
            return rekordboxFolderPath.isEmpty ? "Not configured" : "Configured · no valid export loaded"
        }
        return "Ready · \(discovery.export.fileName) · \(availableExportCount) XML export\(availableExportCount == 1 ? "" : "s") found"
    }

    private var selectedDeviceSummary: RekordboxDeviceState? {
        guard !rekordboxDeviceRoot.isEmpty else { return nil }
        let volume = mountedIdentity(URL(fileURLWithPath: rekordboxDeviceRoot, isDirectory: true))
        guard let sourceID = USBSourceIdentityResolver.selectedSourceID(
            for: volume,
            devices: visibleUSBDevices
        ) else { return nil }
        return visibleUSBDevices.first { $0.sourceID == sourceID }
    }

    private var isDeviceSyncFeedback: Bool {
        guard let feedback = syncFeedback?.lowercased() else { return false }
        return feedback.contains("device") || feedback.contains("usb")
    }

    private var deviceSourceState: LumiComponentState {
        if selectedDeviceSummary != nil { return .ready }
        return rekordboxDeviceRoot.isEmpty ? .empty : .stale
    }

    private var deviceSourceStatus: String {
        guard let device = selectedDeviceSummary else {
            return rekordboxDeviceRoot.isEmpty
                ? "Not configured"
                : "Configured · sync required"
        }
        return "Ready · \(device.displayName) · \(device.matchedTracks)/\(device.activeTracks) tracks matched · \(device.unmatchedTracks) held"
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
        rekordboxDeviceRoot = url.path
        selectedUSBSourceID = USBSourceIdentityResolver.selectedSourceID(
            for: mountedIdentity(url),
            devices: visibleUSBDevices
        )
        selectedUSBPlaylistIDs = []
        usbPlaylistSearch = ""
        onDeviceInspect(url.path)
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
