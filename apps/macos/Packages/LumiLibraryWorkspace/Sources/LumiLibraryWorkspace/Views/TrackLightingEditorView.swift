import AppKit
import LumiDesignSystem
import SwiftUI

public struct TrackLightingEditorView: View {
    private let analysis: TrackEditorAnalysis
    private let autoloopCatalog: AutoloopCatalogState?
    private let phraseColorPalette: LumiPhraseColorPalette
    private let keyNotation: KeyNotationPreference
    private let feedback: String?
    private let rendersInteractiveControls: Bool
    private let isEmbedded: Bool
    private let onTimelineEdit: @MainActor (TrackTimelineEditRequest) -> Void
    private let onTimelineHistory: @MainActor (TrackTimelineHistoryRequest) -> Void
    private let onSourceReconcile: @MainActor (TrackSourceReconcileRequest) -> Void
    private let onReuseTimeline: @MainActor (CreativeTimelineReuseRequest) -> Void
    @StateObject private var audio: TrackAudioPreviewController
    @State private var viewport: TrackEditorViewport
    @State private var selectedPhraseID: UInt64?
    @State private var loopSelectedPhrase = false
    @State private var selectionStartBeat: UInt32
    @State private var selectionEndBeat: UInt32
    @State private var selectionAnchorBeat: UInt32?
    @State private var magnificationAnchorBeats: Double?
    @State private var draggingBoundaryAfterPhraseIndex: UInt16?
    @State private var hoveredBoundaryAfterPhraseIndex: UInt16?
    @State private var pendingBoundaryBeat: UInt32?
    @State private var conflictSides: [UInt16: TrackSourceConflictSide]
    @State private var pendingCreativeReuse: CreativeTimelineCandidate?
    @AppStorage(LumiPreferenceKey.waveformZoomAnchor)
    private var waveformZoomAnchorRaw = LumiWaveformZoomAnchor.mouse.rawValue
    @AppStorage(LumiPreferenceKey.waveformReverseHorizontalScroll)
    private var reversesHorizontalScroll = false
    @Environment(\.dismiss) private var dismiss

    private let background = LumiColor.canvas
    private let panel = LumiColor.surfaceElevated
    private let primary = LumiColor.textPrimary
    private let secondary = LumiColor.textSecondary
    private let accent = LumiColor.accent

    public init(
        analysis: TrackEditorAnalysis,
        autoloopCatalog: AutoloopCatalogState? = nil,
        phraseColorPalette: LumiPhraseColorPalette = .defaults,
        keyNotation: KeyNotationPreference,
        feedback: String? = nil,
        rendersInteractiveControls: Bool = true,
        isEmbedded: Bool = false,
        onTimelineEdit: @escaping @MainActor (TrackTimelineEditRequest) -> Void = { _ in },
        onTimelineHistory: @escaping @MainActor (TrackTimelineHistoryRequest) -> Void = { _ in },
        onSourceReconcile: @escaping @MainActor (TrackSourceReconcileRequest) -> Void = { _ in },
        onReuseTimeline: @escaping @MainActor (CreativeTimelineReuseRequest) -> Void = { _ in }
    ) {
        self.analysis = analysis
        self.autoloopCatalog = autoloopCatalog
        self.phraseColorPalette = phraseColorPalette
        self.keyNotation = keyNotation
        self.feedback = feedback
        self.rendersInteractiveControls = rendersInteractiveControls
        self.isEmbedded = isEmbedded
        self.onTimelineEdit = onTimelineEdit
        self.onTimelineHistory = onTimelineHistory
        self.onSourceReconcile = onSourceReconcile
        self.onReuseTimeline = onReuseTimeline
        _audio = StateObject(wrappedValue: TrackAudioPreviewController(analysis: analysis))
        _viewport = State(
            initialValue: TrackEditorViewport(
                startBeat: 0,
                visibleBeats: Double(max(1, analysis.totalBeats)),
                totalBeats: UInt64(analysis.totalBeats),
                beatsPerBar: analysis.beatsPerBar
            )
        )
        _selectedPhraseID = State(initialValue: analysis.phrases.first?.id)
        let firstStart = analysis.phrases.first?.startBeat ?? 0
        let firstEnd = analysis.phrases.first?.endBeat ?? UInt32(analysis.beatsPerBar)
        _selectionStartBeat = State(initialValue: firstStart)
        _selectionEndBeat = State(initialValue: firstEnd)
        _conflictSides = State(
            initialValue: Dictionary(
                uniqueKeysWithValues: (analysis.sourceReconciliation?.conflicts ?? [])
                    .map { ($0.phraseIndex, TrackSourceConflictSide.lumi) }
            )
        )
    }

    public var body: some View {
        VStack(spacing: 0) {
            header
            Divider().overlay(Color.white.opacity(0.12))
            transport
            editToolbar
            sourceReconciliationPanel
            HStack(spacing: 12) {
                VStack(spacing: 10) {
                    editorCanvas
                    overview
                }
                phraseInspector
                    .frame(width: 268)
            }
            .padding(.horizontal, 20)
            footer
        }
        .foregroundStyle(primary)
        .background(background)
        .environment(\.colorScheme, .dark)
        .frame(
            minWidth: isEmbedded ? 0 : 980,
            idealWidth: isEmbedded ? nil : 1_160,
            minHeight: isEmbedded ? 500 : 620,
            idealHeight: isEmbedded ? nil : 720
        )
        .preferredColorScheme(.dark)
        .accessibilityIdentifier("lumi.trackEditor")
        .focusable()
        .focusEffectDisabled()
        .overlay { LumiSpacebarMonitor { audio.togglePlayback() } }
        .onKeyPress(keys: [.leftArrow, .rightArrow], phases: .down) { press in
            let direction = press.key == .leftArrow ? -1 : 1
            if press.modifiers.contains(.shift) {
                audio.moveByBar(direction)
            } else {
                audio.moveByBeat(direction)
            }
            revealPlayhead()
            return .handled
        }
        .onKeyPress("p") {
            if let roleID = selectedPhrase?.roleID { placePhrasePoint(roleID: roleID) }
            return .handled
        }
        .onKeyPress(.delete) {
            guard analysis.phrases.count > 1, let index = selectedPhraseIndex else { return .ignored }
            deleteSelected(absorbPrevious: index > 0)
            return .handled
        }
        .onChange(of: audio.positionMillis) { _, _ in
            if audio.isPlaying { revealPlayhead() }
        }
        .onChange(of: analysis) { previous, current in
            adoptTimelineUpdate(previous: previous, current: current)
            conflictSides = Dictionary(
                uniqueKeysWithValues: (current.sourceReconciliation?.conflicts ?? [])
                    .map { ($0.phraseIndex, conflictSides[$0.phraseIndex] ?? .lumi) }
            )
        }
        .onDisappear { audio.shutdown() }
        .confirmationDialog(
            "Reuse Lumi phrases?",
            isPresented: Binding(
                get: { pendingCreativeReuse != nil },
                set: { if !$0 { pendingCreativeReuse = nil } }
            ),
            presenting: pendingCreativeReuse
        ) { candidate in
            Button("Copy phrases from \(candidate.title)") {
                onReuseTimeline(
                    CreativeTimelineReuseRequest(
                        sourceTrackID: candidate.trackID,
                        targetTrackID: analysis.track.id,
                        expectedTargetRevision: analysis.timeline.revision
                    )
                )
                pendingCreativeReuse = nil
            }
            Button("Cancel", role: .cancel) { pendingCreativeReuse = nil }
        } message: { candidate in
            Text(
                "This creates a new Lumi revision for \(analysis.track.title). "
                    + "The old track and its phrase work remain unchanged."
            )
        }
    }

    private var header: some View {
        HStack(spacing: 20) {
            VStack(alignment: .leading, spacing: 3) {
                Text(analysis.track.title)
                    .font(.system(size: 23, weight: .semibold, design: .rounded))
                Text(analysis.track.artist)
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(secondary)
            }
            Spacer()
            metric("KEY", KeyNotationFormatter(notation: keyNotation).string(from: analysis.track.musicalKey))
            metric("BPM", formatEditorBPM(analysis.track.bpmMilli))
            metric("REMAIN", formatEditorTime(remainingMillis))
            metric("BAR · BEAT", barBeatLabel)
            if rendersInteractiveControls, !analysis.creativeReuseCandidates.isEmpty {
                Menu {
                    ForEach(analysis.creativeReuseCandidates) { candidate in
                        Button {
                            pendingCreativeReuse = candidate
                        } label: {
                            Label {
                                Text(
                                    candidate.title
                                        + (candidate.likelyVersion ? " · Suggested version" : "")
                                )
                            } icon: {
                                Image(
                                    systemName: candidate.exactBeatCompatibility
                                        ? "checkmark.circle"
                                        : "exclamationmark.triangle"
                                )
                            }
                        }
                        .disabled(!candidate.exactBeatCompatibility)
                    }
                } label: {
                    Label("Reuse Lumi Phrases", systemImage: "square.on.square")
                }
                .menuStyle(.borderlessButton)
                .help(
                    "Copy an existing Lumi-authored phrase timeline into this track as a new revision. "
                        + "Only exact beat-compatible versions can be applied automatically."
                )
                .accessibilityIdentifier("lumi.trackEditor.reusePhrases")
            }
            if !isEmbedded {
                Button(editorCopy("editor.close")) {
                    dismiss()
                }
                .buttonStyle(.bordered)
                .accessibilityIdentifier("lumi.trackEditor.close")
            }
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 15)
        .frame(minHeight: 68)
        .background(panel)
    }

    @ViewBuilder
    private var sourceReconciliationPanel: some View {
        if let refresh = analysis.sourceReconciliation {
            VStack(alignment: .leading, spacing: 9) {
                HStack(spacing: 10) {
                    Label(editorCopy("editor.sourceChanges"), systemImage: "arrow.triangle.2.circlepath")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(accent)
                    Text("\(refresh.fromRevision) → \(refresh.toRevision)")
                        .font(.system(size: 10, weight: .medium, design: .monospaced))
                        .foregroundStyle(secondary)
                    ForEach(refresh.changes, id: \.self) { change in
                        Text(readableSourceChange(change))
                            .font(.system(size: 9, weight: .bold))
                            .padding(.horizontal, 7)
                            .padding(.vertical, 3)
                            .background(accent.opacity(0.14))
                            .clipShape(Capsule())
                    }
                    Spacer()
                    Button(editorCopy("editor.keepLumi")) { onSourceReconcile(.keepLumi) }
                    Button(editorCopy("editor.rebase")) { onSourceReconcile(.rebase) }
                        .disabled(refresh.metadataOnly)
                    Button(editorCopy("editor.replaceSource")) { onSourceReconcile(.replaceWithSource) }
                        .tint(Color.orange)
                }
                if !refresh.conflicts.isEmpty {
                    HStack(spacing: 8) {
                        Text(editorCopy("editor.mergeEachConflict"))
                            .font(.system(size: 10, weight: .semibold))
                            .foregroundStyle(secondary)
                        ScrollView(.horizontal, showsIndicators: false) {
                            HStack(spacing: 6) {
                                ForEach(refresh.conflicts) { conflict in
                                    Menu {
                                        Button("Lumi") {
                                            conflictSides[conflict.phraseIndex] = .lumi
                                        }
                                        Button("Source") {
                                            conflictSides[conflict.phraseIndex] = .source
                                        }
                                    } label: {
                                        Text(
                                            "P\(conflict.phraseIndex + 1) · "
                                                + (conflictSides[conflict.phraseIndex] == .source
                                                    ? "Source" : "Lumi")
                                        )
                                        .font(.system(size: 9, weight: .semibold))
                                        .padding(.horizontal, 7)
                                        .frame(height: 24)
                                        .background(Color.white.opacity(0.08))
                                        .clipShape(RoundedRectangle(cornerRadius: 5))
                                    }
                                    .fixedSize()
                                }
                            }
                        }
                        Button(editorCopy("editor.applyMerge")) {
                            onSourceReconcile(
                                .merge(refresh.conflicts.map { conflict in
                                    TrackSourceConflictChoice(
                                        phraseIndex: conflict.phraseIndex,
                                        side: conflictSides[conflict.phraseIndex] ?? .lumi
                                    )
                                })
                            )
                        }
                        .buttonStyle(.borderedProminent)
                        .tint(accent)
                    }
                }
                if !refresh.rebaseAmbiguities.isEmpty {
                    Label(
                        String(
                            format: editorCopy("editor.rebaseRounding"),
                            UInt64(refresh.rebaseAmbiguities.count)
                        ),
                        systemImage: "exclamationmark.triangle.fill"
                    )
                    .font(.system(size: 9, weight: .medium))
                    .foregroundStyle(Color.orange)
                }
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 10)
            .background(accent.opacity(0.07))
            .accessibilityIdentifier("lumi.trackEditor.sourceReconciliation")
        } else {
            HStack {
                Label(editorCopy("editor.sourceIndependent"), systemImage: "shield.checkered")
                    .font(.system(size: 10, weight: .medium))
                    .foregroundStyle(secondary)
                Spacer()
                Button(editorCopy("editor.compareSource")) {
                    onSourceReconcile(.previewDemoChanges)
                }
                .accessibilityIdentifier("lumi.trackEditor.compareSource")
            }
            .padding(.horizontal, 20)
            .frame(height: 38)
            .background(panel.opacity(0.7))
        }
    }

    private func readableSourceChange(_ value: String) -> String {
        switch value {
        case "beatGrid": "Beat grid"
        case "rawPhrases": "Raw phrases"
        default: value.capitalized
        }
    }

    private var transport: some View {
        HStack(spacing: 12) {
            Button { stepBeat(-1) } label: {
                Image(systemName: "backward.end.fill")
            }
            .help("\(editorCopy("editor.previousBar")) · Left Arrow")
            .accessibilityLabel(editorCopy("editor.previousBar"))
            .accessibilityIdentifier("lumi.trackEditor.previousBar")

            Button { audio.togglePlayback() } label: {
                Image(systemName: audio.isPlaying ? "pause.fill" : "play.fill")
                    .frame(width: 20)
            }
            .buttonStyle(.borderedProminent)
            .tint(accent)
            .help("\(editorCopy("editor.playPause")) · Space")
            .accessibilityLabel(editorCopy("editor.playPause"))
            .accessibilityIdentifier("lumi.trackEditor.playPause")

            Button { audio.stop() } label: {
                Image(systemName: "stop.fill")
            }
            .help(editorCopy("editor.stop"))
            .accessibilityLabel(editorCopy("editor.stop"))
            .accessibilityIdentifier("lumi.trackEditor.stop")

            Button { stepBeat(1) } label: {
                Image(systemName: "forward.end.fill")
            }
            .help("\(editorCopy("editor.nextBar")) · Right Arrow")
            .accessibilityLabel(editorCopy("editor.nextBar"))
            .accessibilityIdentifier("lumi.trackEditor.nextBar")

            Divider().frame(height: 24)

            Toggle(isOn: $loopSelectedPhrase) {
                Label(editorCopy("editor.loopPhrase"), systemImage: "repeat")
            }
            .toggleStyle(.button)
            .disabled(selectedPhrase == nil)
            .onChange(of: loopSelectedPhrase) { _, enabled in
                audio.setLoop(enabled ? selectedPhrase : nil)
            }
            .accessibilityIdentifier("lumi.trackEditor.loopPhrase")

            Spacer()

            Image(systemName: "speaker.fill")
                .foregroundStyle(secondary)
            TrackEditorVolumeSlider(value: $audio.volume, accent: accent)
                .frame(width: 120)
                .accessibilityLabel(editorCopy("editor.volume"))
                .accessibilityIdentifier("lumi.trackEditor.volume")

            LumiWaveformZoomControls(
                zoom: zoomSliderBinding,
                visibleBars: viewport.visibleBars,
                zoomAnchor: waveformZoomAnchorBinding,
                reversesHorizontalScroll: $reversesHorizontalScroll,
                accessibilityPrefix: "lumi.trackEditor"
            )
        }
        .padding(.horizontal, 20)
        .frame(height: 58)
        .background(background)
        .buttonStyle(.bordered)
        .controlSize(.regular)
    }

    private var editToolbar: some View {
        HStack(spacing: 8) {
            Button {
                onTimelineHistory(.undo)
            } label: {
                Label(editorCopy("editor.undo"), systemImage: "arrow.uturn.backward")
            }
            .disabled(!analysis.timeline.canUndo)
            .keyboardShortcut("z", modifiers: .command)
            .accessibilityIdentifier("lumi.trackEditor.undo")

            Button {
                onTimelineHistory(.redo)
            } label: {
                Label(editorCopy("editor.redo"), systemImage: "arrow.uturn.forward")
            }
            .disabled(!analysis.timeline.canRedo)
            .keyboardShortcut("z", modifiers: [.command, .shift])
            .accessibilityIdentifier("lumi.trackEditor.redo")

            Divider().frame(height: 24)

            Button {
                splitSelectedPhrase()
            } label: {
                Label(editorCopy("editor.split"), systemImage: "scissors")
            }
            .disabled(!canSplitSelectedPhrase)
            .accessibilityIdentifier("lumi.trackEditor.split")

            Button {
                mergeSelectedPrevious()
            } label: {
                Label(editorCopy("editor.mergePrevious"), systemImage: "arrow.left.to.line.compact")
            }
            .disabled(selectedPhraseIndex == nil || selectedPhraseIndex == 0)
            .accessibilityIdentifier("lumi.trackEditor.mergePrevious")

            Button {
                mergeSelectedNext()
            } label: {
                Label(editorCopy("editor.mergeNext"), systemImage: "arrow.right.to.line.compact")
            }
            .disabled(selectedPhraseIndex.map { $0 + 1 >= analysis.phrases.count } ?? true)
            .accessibilityIdentifier("lumi.trackEditor.mergeNext")

            if rendersInteractiveControls {
                Menu {
                    Button(editorCopy("editor.absorbPrevious")) { deleteSelected(absorbPrevious: true) }
                        .disabled(selectedPhraseIndex == nil || selectedPhraseIndex == 0)
                    Button(editorCopy("editor.absorbNext")) { deleteSelected(absorbPrevious: false) }
                        .disabled(selectedPhraseIndex.map { $0 + 1 >= analysis.phrases.count } ?? true)
                } label: {
                    Label(editorCopy("editor.delete"), systemImage: "trash")
                }
                .disabled(analysis.phrases.count <= 1 || selectedPhrase == nil)
                .accessibilityIdentifier("lumi.trackEditor.delete")
            } else {
                Label(editorCopy("editor.delete"), systemImage: "trash")
                    .padding(.horizontal, 8)
            }

            Spacer()

            if rendersInteractiveControls {
                Menu {
                    ForEach(analysis.timeline.revisions) { revision in
                        Button {
                            onTimelineHistory(.restore(revision: revision.revision))
                        } label: {
                            Text(revisionLabel(revision))
                        }
                        .disabled(revision.revision == analysis.timeline.revision)
                    }
                } label: {
                    Label(editorCopy("editor.history"), systemImage: "clock.arrow.circlepath")
                }
                .accessibilityIdentifier("lumi.trackEditor.history")
            } else {
                Label(editorCopy("editor.history"), systemImage: "clock.arrow.circlepath")
                    .padding(.horizontal, 8)
            }

            Label(
                String(format: editorCopy("editor.savedRevision"), analysis.timeline.revision),
                systemImage: "checkmark.circle.fill"
            )
            .font(.system(size: 11, weight: .semibold, design: .monospaced))
            .foregroundStyle(Color.green)
            .accessibilityIdentifier("lumi.trackEditor.savedRevision")
        }
        .buttonStyle(.bordered)
        .controlSize(.regular)
        .padding(.horizontal, 20)
        .frame(height: 44)
        .background(panel.opacity(0.72))
    }

    private var phraseInspector: some View {
        phraseInspectorContent
        .background(panel)
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .overlay {
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color.white.opacity(0.12), lineWidth: 1)
        }
        .accessibilityIdentifier("lumi.trackEditor.inspector")
    }

    private var phraseInspectorContent: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(editorCopy("editor.phraseInspector"))
                    .font(.system(size: 14, weight: .bold, design: .rounded))
                Spacer()
                Text("R\(analysis.timeline.revision)")
                    .font(.system(size: 11, weight: .bold, design: .monospaced))
                    .foregroundStyle(accent)
            }

            if let phrase = selectedPhrase, let phraseIndex = selectedPhraseIndex {
                inspectorLabel(editorCopy("editor.role"))
                if rendersInteractiveControls {
                    Picker(editorCopy("editor.role"), selection: roleBinding(for: phrase, index: phraseIndex)) {
                        ForEach(analysis.roles.filter { !$0.archived || $0.id == phrase.roleID }) { role in
                            Text(role.archived ? "\(role.name) · \(editorCopy("editor.archived"))" : role.name)
                                .tag(role.id)
                        }
                    }
                    .labelsHidden()
                    .accessibilityIdentifier("lumi.trackEditor.role")
                } else {
                    HStack {
                        Text(phrase.role)
                        Spacer()
                        Image(systemName: "chevron.up.chevron.down")
                    }
                    .font(.system(size: 12, weight: .medium))
                    .padding(.horizontal, 10)
                    .frame(height: 30)
                    .background(Color.white.opacity(0.08))
                    .clipShape(RoundedRectangle(cornerRadius: 6))
                }

                inspectorLabel(editorCopy("editor.boundaries"))
                boundaryRow(
                    title: "Start beat",
                    displayValue: phrase.startBeat + 1,
                    canDecrease: canMoveStart(by: -1),
                    canIncrease: canMoveStart(by: 1),
                    decrease: { moveStartBoundary(by: -1) },
                    increase: { moveStartBoundary(by: 1) }
                )
                boundaryRow(
                    title: "End beat",
                    displayValue: phrase.endBeat,
                    canDecrease: canMoveEnd(by: -1),
                    canIncrease: canMoveEnd(by: 1),
                    decrease: { moveEndBoundary(by: -1) },
                    increase: { moveEndBoundary(by: 1) }
                )

                Divider().overlay(Color.white.opacity(0.12))
                inspectorLabel("Beat selection")
                HStack {
                    selectionStepper(
                        title: editorCopy("editor.from"),
                        value: $selectionStartBeat,
                        range: 0...max(0, selectionEndBeat - 1)
                    )
                    selectionStepper(
                        title: editorCopy("editor.to"),
                        value: $selectionEndBeat,
                        range: min(analysis.totalBeats, selectionStartBeat + 1)...analysis.totalBeats
                    )
                }
                if rendersInteractiveControls {
                    Menu {
                        ForEach(analysis.roles.filter { !$0.archived }) { role in
                            Button(role.name) { placePhrasePoint(roleID: role.id) }
                        }
                    } label: {
                        Label("Place Phrase Point", systemImage: "mappin.and.ellipse")
                            .frame(maxWidth: .infinity)
                    }
                    .menuStyle(.button)
                    .buttonStyle(.borderedProminent)
                    .tint(accent)
                    .disabled(quantizedPlayheadBeat >= analysis.totalBeats)
                    .accessibilityIdentifier("lumi.trackEditor.createSelection")
                } else {
                    Label("Place Phrase Point", systemImage: "mappin.and.ellipse")
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 5)
                        .background(accent)
                        .clipShape(RoundedRectangle(cornerRadius: 5))
                }

                loopStrategyEditor(phrase)
            } else {
                Text(editorCopy("editor.noPhrase"))
                    .foregroundStyle(secondary)
            }
        }
        .padding(10)
    }

    private var editorCanvas: some View {
        GeometryReader { proxy in
            Canvas { context, size in
                drawEditor(context: &context, size: size)
            }
            .contentShape(Rectangle())
            .overlay {
                LumiWaveformInteractionMonitor(
                    onScroll: { deltaX in
                        let direction = reversesHorizontalScroll ? -1.0 : 1.0
                        viewport = viewport.panned(byPixels: deltaX * direction, width: proxy.size.width)
                    },
                    onZoom: { delta, pointerFraction in
                        zoomFromScroll(delta, pointerFraction: pointerFraction)
                    }
                )
            }
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { value in
                        if draggingBoundaryAfterPhraseIndex == nil,
                           value.startLocation.y >= phraseLaneTop(height: proxy.size.height),
                           let boundary = boundaryIndex(
                               atX: value.startLocation.x,
                               width: proxy.size.width,
                               tolerance: 8
                           ) {
                            draggingBoundaryAfterPhraseIndex = boundary
                        }
                        if let boundary = draggingBoundaryAfterPhraseIndex {
                            updatePendingBoundary(
                                boundary,
                                atX: value.location.x,
                                width: proxy.size.width
                            )
                        } else if value.location.y < phraseLaneTop(height: proxy.size.height) {
                            seek(atX: value.location.x, width: proxy.size.width)
                        } else {
                            updateBeatSelection(atX: value.location.x, width: proxy.size.width)
                        }
                    }
                    .onEnded { value in
                        if let boundary = draggingBoundaryAfterPhraseIndex,
                           let beat = pendingBoundaryBeat {
                            onTimelineEdit(
                                .moveBoundary(afterPhraseIndex: boundary, toBeat: beat)
                            )
                        } else if value.location.y >= phraseLaneTop(height: proxy.size.height) {
                            if abs(value.translation.width) < 3 {
                                selectPhrase(atX: value.location.x, width: proxy.size.width)
                            }
                        }
                        draggingBoundaryAfterPhraseIndex = nil
                        pendingBoundaryBeat = nil
                        selectionAnchorBeat = nil
                    }
            )
            .simultaneousGesture(
                MagnifyGesture()
                    .onChanged { value in
                        let baseline = magnificationAnchorBeats ?? viewport.visibleBeats
                        magnificationAnchorBeats = baseline
                        viewport = viewport.zoomed(
                            to: baseline / max(0.05, value.magnification),
                            aroundBeat: currentBeat
                        )
                    }
                    .onEnded { _ in magnificationAnchorBeats = nil }
            )
            .onContinuousHover { phase in
                switch phase {
                case .active(let location):
                    let boundary = location.y >= phraseLaneTop(height: proxy.size.height)
                        ? boundaryIndex(atX: location.x, width: proxy.size.width, tolerance: 8)
                        : nil
                    hoveredBoundaryAfterPhraseIndex = boundary
                    (boundary == nil ? NSCursor.arrow : NSCursor.resizeLeftRight).set()
                case .ended:
                    hoveredBoundaryAfterPhraseIndex = nil
                    NSCursor.arrow.set()
                }
            }
        }
        .frame(minHeight: isEmbedded ? 285 : 315)
        .background(panel)
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .overlay {
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color.white.opacity(0.12), lineWidth: 1)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(editorCopy("editor.timelineLabel"))
        .accessibilityValue("\(barBeatLabel), \(selectedPhrase?.role ?? editorCopy("editor.noPhrase"))")
        .accessibilityIdentifier("lumi.trackEditor.timeline")
    }

    @ViewBuilder
    private func loopStrategyEditor(_ phrase: TrackEditorPhrase) -> some View {
        let role = autoloopCatalog?.roles.first { $0.id == phrase.roleID }
        let variants = role?.variants.filter { !$0.archived } ?? []
        VStack(alignment: .leading, spacing: 5) {
            HStack {
                inspectorLabel(editorCopy("editor.loopStrategy"))
                Spacer()
                Label(
                    loopStrategyStatusLabel(phrase.loopStrategy),
                    systemImage: phrase.loopStrategy.locked ? "lock.fill" : "wand.and.stars"
                )
                .font(.system(size: 9, weight: .semibold))
                .foregroundStyle(loopStrategyStatusColor(phrase.loopStrategy))
            }
            Text(loopStrategySummary(phrase.loopStrategy, variants: variants))
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(primary)

            if rendersInteractiveControls, let catalog = autoloopCatalog {
                HStack(spacing: 6) {
                    Button(editorCopy("editor.loopAutomatic")) {
                        setLoopStrategy(phrase, .automatic)
                    }
                    .disabled(phrase.loopStrategy.kind == "auto")
                    Menu(editorCopy("editor.lockVariant")) {
                        ForEach(variants) { variant in
                            Button(variant.name) {
                                setLoopStrategy(phrase, .fixedVariant(variant.id))
                            }
                        }
                    }
                    .disabled(variants.isEmpty)
                }
                .buttonStyle(.bordered)
                .controlSize(.small)

                Menu {
                    ForEach(catalog.themes) { theme in
                        Menu(theme.name) {
                            Button(editorCopy("editor.themeAutomatic")) {
                                updateThemeOverride(phrase, themeID: theme.id, variantID: nil)
                            }
                            ForEach(compatibleVariants(theme, variants: variants)) { variant in
                                Button(variant.name) {
                                    updateThemeOverride(
                                        phrase,
                                        themeID: theme.id,
                                        variantID: variant.id
                                    )
                                }
                            }
                        }
                    }
                } label: {
                    Label(
                        "\(editorCopy("editor.themeOverrides")) · \(phrase.loopStrategy.themeOverrides.count)",
                        systemImage: "square.grid.2x2"
                    )
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .menuStyle(.button)
                .controlSize(.small)
            }

            if phrase.loopStrategy.status != "ready" {
                Label(
                    "\(phrase.loopStrategy.issues.count) \(editorCopy("editor.strategyIssues"))",
                    systemImage: "exclamationmark.triangle.fill"
                )
                .font(.system(size: 9, weight: .medium))
                .foregroundStyle(Color.orange)
            }
        }
        .accessibilityIdentifier("lumi.trackEditor.loopStrategy")
    }

    private func compatibleVariants(
        _ theme: AutoloopThemeState,
        variants: [AutoloopVariantState]
    ) -> [AutoloopVariantState] {
        variants.filter { variant in
            variant.cells.contains { $0.themeID == theme.id && !$0.isMissing }
        }
    }

    private func setLoopStrategy(
        _ phrase: TrackEditorPhrase,
        _ strategy: TrackLoopStrategyRequest
    ) {
        guard let phraseIndex = UInt16(exactly: phrase.id) else { return }
        onTimelineEdit(.setLoopStrategy(phraseIndex: phraseIndex, strategy: strategy))
    }

    private func updateThemeOverride(
        _ phrase: TrackEditorPhrase,
        themeID: UInt64,
        variantID: String?
    ) {
        var overrides = phrase.loopStrategy.kind == "themeSpecificExact"
            ? phrase.loopStrategy.themeOverrides
            : []
        overrides.removeAll { $0.themeID == themeID }
        if let variantID {
            overrides.append(TrackEditorThemeVariantOverride(themeID: themeID, variantID: variantID))
        }
        overrides.sort { $0.themeID < $1.themeID }
        setLoopStrategy(
            phrase,
            overrides.isEmpty ? .automatic : .themeSpecificExact(overrides)
        )
    }

    private func loopStrategySummary(
        _ strategy: TrackEditorLoopStrategy,
        variants: [AutoloopVariantState]
    ) -> String {
        switch strategy.kind {
        case "fixedVariant":
            return variants.first { $0.id == strategy.fixedVariantID }?.name
                ?? strategy.fixedVariantID
                ?? editorCopy("editor.variantUnavailable")
        case "themeSpecificExact":
            return "\(strategy.themeOverrides.count) \(editorCopy("editor.themeOverrides").lowercased())"
        default:
            return editorCopy("editor.automaticSelection")
        }
    }

    private func loopStrategyStatusLabel(_ strategy: TrackEditorLoopStrategy) -> String {
        switch strategy.status {
        case "stale": editorCopy("editor.strategyStale")
        case "incomplete": editorCopy("editor.strategyIncomplete")
        default: strategy.locked ? editorCopy("editor.strategyLocked") : editorCopy("editor.strategyAutomatic")
        }
    }

    private func loopStrategyStatusColor(_ strategy: TrackEditorLoopStrategy) -> Color {
        switch strategy.status {
        case "stale": Color.red
        case "incomplete": Color.orange
        default: strategy.locked ? accent : secondary
        }
    }

    private var overview: some View {
        GeometryReader { proxy in
            Canvas { context, size in
                drawOverview(context: &context, size: size)
            }
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { value in
                        let progress = min(max(0, value.location.x / max(1, proxy.size.width)), 1)
                        let beat = progress * Double(analysis.totalBeats)
                        viewport = viewport.centered(onBeat: beat)
                    }
            )
        }
        .frame(height: 58)
        .background(panel)
        .clipShape(RoundedRectangle(cornerRadius: 6))
        .accessibilityLabel(editorCopy("editor.overviewLabel"))
        .accessibilityIdentifier("lumi.trackEditor.overview")
    }

    private var footer: some View {
        HStack(spacing: 16) {
            if let reason = audio.unavailableReason {
                Label(reason, systemImage: "exclamationmark.triangle.fill")
                    .foregroundStyle(Color.orange)
                    .accessibilityIdentifier("lumi.trackEditor.previewUnavailable")
            } else {
                Label(editorCopy("editor.originalAudio"), systemImage: "lock.open.display")
                    .foregroundStyle(secondary)
            }
            Spacer()
            if let phrase = selectedPhrase {
                Circle().fill(phraseColor(phrase.roleID)).frame(width: 9, height: 9)
                Text("\(phrase.role) · beats \(phrase.startBeat + 1)–\(phrase.endBeat) · \(phrase.origin)")
            }
            if let feedback {
                Text(feedback)
                    .foregroundStyle(feedback.lowercased().contains("could not") ? Color.orange : accent)
                    .lineLimit(1)
                    .accessibilityIdentifier("lumi.trackEditor.feedback")
            }
            Text(editorCopy("editor.shortcuts"))
                .foregroundStyle(secondary)
        }
        .font(.system(size: 12, weight: .medium, design: .monospaced))
        .padding(.horizontal, 20)
        .frame(height: 52)
    }

    private func metric(_ title: String, _ value: String) -> some View {
        VStack(alignment: .trailing, spacing: 2) {
            Text(title)
                .font(.system(size: 9, weight: .bold, design: .monospaced))
                .foregroundStyle(secondary)
            Text(value)
                .font(.system(size: 16, weight: .semibold, design: .monospaced))
        }
        .accessibilityElement(children: .combine)
    }

    private func drawEditor(context: inout GraphicsContext, size: CGSize) {
        let width = Double(size.width)
        let waveformTop = 56.0
        let phraseTop = phraseLaneTop(height: size.height)
        let waveformBottom = phraseTop - 12
        let center = (waveformTop + waveformBottom) / 2
        let amplitude = max(24, (waveformBottom - waveformTop) / 2 - 8)

        let firstVisibleBeat = max(0, Int(viewport.startBeat.rounded(.down)))
        let lastVisibleBeat = min(Int(analysis.totalBeats), Int(viewport.endBeat.rounded(.up)))
        for beat in firstVisibleBeat...lastVisibleBeat {
            let x = viewport.x(forBeat: Double(beat), width: width)
            let isBar = beat.isMultiple(of: Int(analysis.beatsPerBar))
            var line = Path()
            line.move(to: CGPoint(x: x, y: isBar ? 22 : 39))
            line.addLine(to: CGPoint(x: x, y: phraseTop - 4))
            context.stroke(
                line,
                with: .color(Color.white.opacity(isBar ? 0.28 : 0.10)),
                lineWidth: isBar ? 1.2 : 0.6
            )
            if isBar, Double(beat) < viewport.endBeat {
                let label = Text("\(beat / Int(analysis.beatsPerBar) + 1)")
                    .font(.system(size: 11, weight: .bold, design: .monospaced))
                    .foregroundColor(primary)
                context.draw(label, at: CGPoint(x: x + 5, y: 13), anchor: .topLeading)
            }
        }

        for pixel in 0..<max(1, Int(width.rounded(.up))) {
            let x = Double(pixel)
            let point = interpolatedWaveformPoint(atBeat: viewport.beat(atX: x, width: width))
            drawRGBWaveformSample(
                context: &context,
                x: x,
                center: center,
                maximumAmplitude: amplitude,
                point: point
            )
        }

        drawHotCueMarkers(context: &context, width: width, bottom: phraseTop - 4, viewport: viewport)

        for phrase in analysis.phrases where Double(phrase.endBeat) > viewport.startBeat && Double(phrase.startBeat) < viewport.endBeat {
            let start = viewport.x(forBeat: Double(phrase.startBeat), width: width)
            let end = viewport.x(forBeat: Double(phrase.endBeat), width: width)
            let rect = CGRect(x: start + 1, y: phraseTop, width: max(2, end - start - 2), height: 54)
            // Phrase colors are user-authored visual identity. Render the
            // persisted sRGB value without blending it into the dark canvas,
            // so the Editor matches the Settings swatch and every other
            // phrase-aware surface.
            context.fill(Path(roundedRect: rect, cornerRadius: 4), with: .color(phraseColor(phrase.roleID)))
            if phrase.id == selectedPhraseID {
                context.stroke(Path(roundedRect: rect, cornerRadius: 4), with: .color(.white), lineWidth: 2)
            }
            let label = Text(phrase.role.uppercased())
                .font(.system(size: 10, weight: .bold, design: .monospaced))
                .foregroundColor(.white)
            context.draw(label, at: CGPoint(x: rect.minX + 7, y: rect.midY), anchor: .leading)

            if start >= 0, start <= width {
                var markerLine = Path()
                markerLine.move(to: CGPoint(x: start, y: 43))
                markerLine.addLine(to: CGPoint(x: start, y: phraseTop))
                context.stroke(
                    markerLine,
                    with: .color(phraseColor(phrase.roleID).opacity(0.92)),
                    lineWidth: 1.5
                )
                var marker = Path()
                marker.move(to: CGPoint(x: start - 6, y: 43))
                marker.addLine(to: CGPoint(x: start + 6, y: 43))
                marker.addLine(to: CGPoint(x: start, y: 51))
                marker.closeSubpath()
                context.fill(marker, with: .color(phraseColor(phrase.roleID)))
                let pointLabel = Text("P\(phrase.id + 1)")
                    .font(.system(size: 8, weight: .bold, design: .monospaced))
                    .foregroundColor(primary)
                context.draw(pointLabel, at: CGPoint(x: start + 7, y: 43), anchor: .topLeading)
            }
        }

        for (index, phrase) in analysis.phrases.enumerated().dropFirst() {
            let boundary = UInt16(index - 1)
            let x = viewport.x(forBeat: Double(phrase.startBeat), width: width)
            guard x >= -6, x <= width + 6 else { continue }
            let emphasized = hoveredBoundaryAfterPhraseIndex == boundary
                || draggingBoundaryAfterPhraseIndex == boundary
            let handle = CGRect(x: x - 4, y: phraseTop + 7, width: 8, height: 40)
            context.fill(
                Path(roundedRect: handle, cornerRadius: 3),
                with: .color(Color.black.opacity(emphasized ? 0.84 : 0.58))
            )
            context.stroke(
                Path(roundedRect: handle, cornerRadius: 3),
                with: .color(Color.white.opacity(emphasized ? 1 : 0.68)),
                lineWidth: emphasized ? 1.8 : 1
            )
            for offset in [-1.3, 1.3] {
                var grip = Path()
                grip.move(to: CGPoint(x: x + offset, y: phraseTop + 18))
                grip.addLine(to: CGPoint(x: x + offset, y: phraseTop + 36))
                context.stroke(grip, with: .color(Color.white.opacity(0.78)), lineWidth: 0.8)
            }
        }

        if Double(selectionEndBeat) > viewport.startBeat && Double(selectionStartBeat) < viewport.endBeat {
            let start = viewport.x(forBeat: Double(selectionStartBeat), width: width)
            let end = viewport.x(forBeat: Double(selectionEndBeat), width: width)
            let rect = CGRect(
                x: start,
                y: phraseTop - 4,
                width: max(2, end - start),
                height: 62
            )
            // Keep the configured phrase color intact. The outline and beat
            // handles communicate the editable selection without a tinted
            // overlay that changes its perceived color.
            context.stroke(Path(rect), with: .color(accent), lineWidth: 1.5)
            context.fill(
                Path(ellipseIn: CGRect(x: start - 4, y: phraseTop - 8, width: 8, height: 8)),
                with: .color(accent)
            )
            context.fill(
                Path(ellipseIn: CGRect(x: end - 4, y: phraseTop - 8, width: 8, height: 8)),
                with: .color(accent)
            )
        }

        drawPlayhead(context: &context, width: width, height: Double(size.height), viewport: viewport)
        if let pendingBoundaryBeat {
            let x = viewport.x(forBeat: Double(pendingBoundaryBeat), width: width)
            var pending = Path()
            pending.move(to: CGPoint(x: x, y: 38))
            pending.addLine(to: CGPoint(x: x, y: Double(size.height)))
            context.stroke(pending, with: .color(accent), style: StrokeStyle(lineWidth: 2, dash: [4, 3]))
        }
    }

    private func drawOverview(context: inout GraphicsContext, size: CGSize) {
        let width = Double(size.width)
        let waveformBottom = Double(size.height) - 18
        let center = waveformBottom / 2
        let amplitude = max(4, center - 3)
        for pixel in 0..<max(1, Int(width.rounded(.up))) {
            let x = Double(pixel)
            let progress = x / max(1, width)
            let point = interpolatedWaveformPoint(
                atTimeMillis: UInt64(progress * Double(analysis.track.durationMillis))
            )
            drawRGBWaveformSample(
                context: &context,
                x: x,
                center: center,
                maximumAmplitude: amplitude,
                point: point
            )
        }
        for phrase in analysis.phrases {
            let duration = Double(max(1, analysis.track.durationMillis))
            let start = Double(TrackEditorCoordinateMapper.timeMillis(
                atBeat: Double(phrase.startBeat),
                analysis: analysis
            )) / duration * width
            let end = Double(TrackEditorCoordinateMapper.timeMillis(
                atBeat: Double(phrase.endBeat),
                analysis: analysis
            )) / duration * width
            let lane = CGRect(x: start, y: waveformBottom + 2, width: max(1, end - start), height: 12)
            context.fill(Path(lane), with: .color(phraseColor(phrase.roleID)))
        }
        for cue in analysis.hotCues {
            let x = Double(cue.timeMillis) / Double(max(1, analysis.track.durationMillis)) * width
            context.fill(
                Path(CGRect(x: x - 1, y: 0, width: 2, height: waveformBottom)),
                with: .color(hotCueColor(cue.colorRGB).opacity(0.86))
            )
        }
        let duration = Double(max(1, analysis.track.durationMillis))
        let start = Double(TrackEditorCoordinateMapper.timeMillis(
            atBeat: viewport.startBeat,
            analysis: analysis
        )) / duration * width
        let end = Double(TrackEditorCoordinateMapper.timeMillis(
            atBeat: viewport.endBeat,
            analysis: analysis
        )) / duration * width
        let visible = max(1, end - start)
        let frame = CGRect(x: start, y: 2, width: visible, height: Double(size.height) - 4)
        context.fill(Path(frame), with: .color(Color.white.opacity(0.08)))
        context.stroke(Path(frame), with: .color(Color.white.opacity(0.76)), lineWidth: 1.2)
        let progress = Double(audio.positionMillis) / Double(max(1, analysis.track.durationMillis))
        var playhead = Path()
        playhead.move(to: CGPoint(x: progress * width, y: 0))
        playhead.addLine(to: CGPoint(x: progress * width, y: Double(size.height)))
        context.stroke(playhead, with: .color(.white), lineWidth: 1.4)
    }

    private func drawPlayhead(
        context: inout GraphicsContext,
        width: Double,
        height: Double,
        viewport: TrackEditorViewport
    ) {
        let beat = TrackEditorCoordinateMapper.beat(atTimeMillis: audio.positionMillis, beats: analysis.beats)
        guard beat >= Double(viewport.startBeat), beat <= Double(viewport.endBeat) else { return }
        let x = viewport.x(forBeat: beat, width: width)
        var path = Path()
        path.move(to: CGPoint(x: x, y: 0))
        path.addLine(to: CGPoint(x: x, y: height))
        context.stroke(path, with: .color(.white), lineWidth: 2)
        context.fill(Path(CGRect(x: x - 3, y: 0, width: 6, height: 7)), with: .color(.white))
    }

    private func drawHotCueMarkers(
        context: inout GraphicsContext,
        width: Double,
        bottom: Double,
        viewport: TrackEditorViewport
    ) {
        for cue in analysis.hotCues {
            let beat = TrackEditorCoordinateMapper.beat(atTimeMillis: cue.timeMillis, beats: analysis.beats)
            guard beat >= viewport.startBeat, beat <= viewport.endBeat else { continue }
            let x = viewport.x(forBeat: beat, width: width)
            let color = hotCueColor(cue.colorRGB)
            var line = Path()
            line.move(to: CGPoint(x: x, y: 27))
            line.addLine(to: CGPoint(x: x, y: bottom))
            context.stroke(line, with: .color(color.opacity(0.82)), lineWidth: 1.25)
            let badge = CGRect(x: x - 8, y: 25, width: 16, height: 16)
            context.fill(Path(roundedRect: badge, cornerRadius: 3), with: .color(color))
            context.draw(
                Text(cue.letter)
                    .font(LumiTypography.hotCueLetter)
                    .foregroundColor(.black.opacity(0.82)),
                at: CGPoint(x: x, y: 33),
                anchor: .center
            )
        }
    }

    private func hotCueColor(_ rgb: UInt32) -> Color {
        Color(
            red: Double((rgb >> 16) & 0xff) / 255,
            green: Double((rgb >> 8) & 0xff) / 255,
            blue: Double(rgb & 0xff) / 255
        )
    }

    private var selectedPhrase: TrackEditorPhrase? {
        analysis.phrases.first { $0.id == selectedPhraseID }
    }

    private var selectedPhraseIndex: Int? {
        analysis.phrases.firstIndex { $0.id == selectedPhraseID }
    }

    private var canSplitSelectedPhrase: Bool {
        guard let phrase = selectedPhrase else { return false }
        return phrase.endBeat - phrase.startBeat > 1
    }

    private func roleBinding(for phrase: TrackEditorPhrase, index: Int) -> Binding<String> {
        Binding(
            get: { phrase.roleID },
            set: { roleID in
                guard roleID != phrase.roleID, let phraseIndex = UInt16(exactly: index) else { return }
                onTimelineEdit(.changeRole(phraseIndex: phraseIndex, roleID: roleID))
            }
        )
    }

    private func inspectorLabel(_ value: String) -> some View {
        Text(value.uppercased())
            .font(.system(size: 9, weight: .bold, design: .monospaced))
            .foregroundStyle(secondary)
    }

    private func boundaryRow(
        title: String,
        displayValue: UInt32,
        canDecrease: Bool,
        canIncrease: Bool,
        decrease: @escaping () -> Void,
        increase: @escaping () -> Void
    ) -> some View {
        HStack {
            Text(title)
                .font(.system(size: 11, weight: .medium))
            Spacer()
            Button(action: decrease) { Image(systemName: "minus") }
                .disabled(!canDecrease)
            Text("\(displayValue)")
                .font(.system(size: 11, weight: .semibold, design: .monospaced))
                .frame(width: 28)
            Button(action: increase) { Image(systemName: "plus") }
                .disabled(!canIncrease)
        }
        .buttonStyle(.bordered)
        .controlSize(.mini)
    }

    private func selectionStepper(
        title: String,
        value: Binding<UInt32>,
        range: ClosedRange<UInt32>
    ) -> some View {
        Group {
            if rendersInteractiveControls {
                Stepper(value: value, in: range) {
                    selectionValue(title: title, value: value.wrappedValue)
                }
            } else {
                selectionValue(title: title, value: value.wrappedValue)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .accessibilityLabel(title)
        .accessibilityValue(
            "Beat \(value.wrappedValue + (title == editorCopy("editor.to") ? 0 : 1))"
        )
    }

    private func selectionValue(title: String, value: UInt32) -> some View {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.system(size: 9, weight: .medium))
                    .foregroundStyle(secondary)
                Text("\(value + (title == editorCopy("editor.to") ? 0 : 1))")
                    .font(.system(size: 11, weight: .semibold, design: .monospaced))
            }
    }

    private func splitSelectedPhrase() {
        guard let phrase = selectedPhrase,
              let index = selectedPhraseIndex,
              let phraseIndex = UInt16(exactly: index) else { return }
        let boundary = min(max(phrase.startBeat + 1, quantizedPlayheadBeat), phrase.endBeat - 1)
        onTimelineEdit(.split(phraseIndex: phraseIndex, atBeat: boundary))
    }

    private func mergeSelectedPrevious() {
        guard let index = selectedPhraseIndex, let phraseIndex = UInt16(exactly: index) else { return }
        onTimelineEdit(.mergePrevious(phraseIndex: phraseIndex))
    }

    private func mergeSelectedNext() {
        guard let index = selectedPhraseIndex, let phraseIndex = UInt16(exactly: index) else { return }
        onTimelineEdit(.mergeNext(phraseIndex: phraseIndex))
    }

    private func deleteSelected(absorbPrevious: Bool) {
        guard let index = selectedPhraseIndex, let phraseIndex = UInt16(exactly: index) else { return }
        onTimelineEdit(
            absorbPrevious
                ? .deleteAbsorbPrevious(phraseIndex: phraseIndex)
                : .deleteAbsorbNext(phraseIndex: phraseIndex)
        )
    }

    private func canMoveStart(by delta: Int) -> Bool {
        guard let phrase = selectedPhrase, let index = selectedPhraseIndex, index > 0 else { return false }
        let target = Int(phrase.startBeat) + delta
        return target > Int(analysis.phrases[index - 1].startBeat)
            && target < Int(phrase.endBeat)
    }

    private func canMoveEnd(by delta: Int) -> Bool {
        guard let phrase = selectedPhrase,
              let index = selectedPhraseIndex,
              index + 1 < analysis.phrases.count else { return false }
        let target = Int(phrase.endBeat) + delta
        return target > Int(phrase.startBeat)
            && target < Int(analysis.phrases[index + 1].endBeat)
    }

    private func moveStartBoundary(by delta: Int) {
        guard canMoveStart(by: delta),
              let phrase = selectedPhrase,
              let index = selectedPhraseIndex,
              let boundaryIndex = UInt16(exactly: index - 1) else { return }
        let target = UInt32(Int(phrase.startBeat) + delta)
        onTimelineEdit(.moveBoundary(afterPhraseIndex: boundaryIndex, toBeat: target))
    }

    private func moveEndBoundary(by delta: Int) {
        guard canMoveEnd(by: delta),
              let phrase = selectedPhrase,
              let index = selectedPhraseIndex,
              let boundaryIndex = UInt16(exactly: index) else { return }
        let target = UInt32(Int(phrase.endBeat) + delta)
        onTimelineEdit(.moveBoundary(afterPhraseIndex: boundaryIndex, toBeat: target))
    }

    private func boundaryIndex(atX x: Double, width: Double, tolerance: Double) -> UInt16? {
        analysis.phrases.enumerated().dropFirst().first { _, phrase in
            abs(viewport.x(forBeat: Double(phrase.startBeat), width: width) - x) <= tolerance
        }.flatMap { index, _ in
            UInt16(exactly: index - 1)
        }
    }

    private func updatePendingBoundary(_ boundary: UInt16, atX x: Double, width: Double) {
        let left = Int(boundary)
        guard left >= 0, left + 1 < analysis.phrases.count else { return }
        let raw = TrackEditorEditGeometry.quantizedBeat(
            viewport.beat(atX: x, width: width),
            totalBeats: analysis.totalBeats
        )
        let minimum = analysis.phrases[left].startBeat + 1
        let maximum = analysis.phrases[left + 1].endBeat - 1
        pendingBoundaryBeat = min(maximum, max(minimum, raw))
    }

    private func updateBeatSelection(atX x: Double, width: Double) {
        let beat = max(0, viewport.beat(atX: x, width: width))
        let quantized = min(analysis.totalBeats - 1, TrackEditorEditGeometry.quantizedBeat(beat, totalBeats: analysis.totalBeats))
        let anchor = selectionAnchorBeat ?? quantized
        selectionAnchorBeat = anchor
        let selection = TrackEditorEditGeometry.beatSelection(
            anchorBeat: anchor,
            currentBeat: quantized,
            totalBeats: analysis.totalBeats
        )
        selectionStartBeat = selection.lowerBound
        selectionEndBeat = selection.upperBound
    }

    private func adoptTimelineUpdate(
        previous: TrackEditorAnalysis,
        current: TrackEditorAnalysis
    ) {
        guard previous.timeline.revision != current.timeline.revision else { return }
        let previousSelection = previous.phrases.first { $0.id == selectedPhraseID }
        let probeBeat = previousSelection.map { phrase in
            phrase.startBeat + (phrase.endBeat - phrase.startBeat - 1) / 2
        }
        selectedPhraseID = probeBeat.flatMap { beat in
            current.phrases.first(where: { beat >= $0.startBeat && beat < $0.endBeat })?.id
        } ?? current.phrases.first?.id
        if let phrase = current.phrases.first(where: { $0.id == selectedPhraseID }) {
            selectionStartBeat = phrase.startBeat
            selectionEndBeat = phrase.endBeat
            if loopSelectedPhrase {
                _ = audio.adoptEditedLoop(phrase)
            }
        }
    }

    private func revisionLabel(_ revision: TrackEditorRevision) -> String {
        "R\(revision.revision) · \(readableReason(revision.reason)) · \(revision.phraseCount) phrases"
    }

    private func readableReason(_ reason: String) -> String {
        reason
            .replacingOccurrences(of: "([a-z])([A-Z])", with: "$1 $2", options: .regularExpression)
            .capitalized
    }

    private var currentBeat: Double {
        TrackEditorCoordinateMapper.beat(atTimeMillis: audio.positionMillis, beats: analysis.beats)
    }

    private var quantizedPlayheadBeat: UInt32 {
        TrackEditorEditGeometry.quantizedBeat(currentBeat, totalBeats: analysis.totalBeats)
    }

    private var barBeatLabel: String {
        let wholeBeat = max(0, Int(currentBeat.rounded(.down)))
        let beatsPerBar = Int(max(1, analysis.beatsPerBar))
        return "\(wholeBeat / beatsPerBar + 1) · \(wholeBeat % beatsPerBar + 1)"
    }

    private var remainingMillis: UInt64 {
        analysis.track.durationMillis > audio.positionMillis
            ? analysis.track.durationMillis - audio.positionMillis
            : 0
    }

    private func phraseLaneTop(height: Double) -> Double {
        max(220, height - 58)
    }

    private func seek(atX x: Double, width: Double) {
        let beat = viewport.beat(atX: x, width: width)
        audio.seek(toMillis: TrackEditorCoordinateMapper.timeMillis(atBeat: beat, analysis: analysis))
    }

    private func selectPhrase(atX x: Double, width: Double) {
        let beat = UInt32(max(0, viewport.beat(atX: x, width: width).rounded(.down)))
        guard let phrase = analysis.phrases.first(where: { beat >= $0.startBeat && beat < $0.endBeat }) else {
            return
        }
        selectedPhraseID = phrase.id
        selectionStartBeat = phrase.startBeat
        selectionEndBeat = phrase.endBeat
        if loopSelectedPhrase { audio.setLoop(phrase) }
    }

    private func stepBeat(_ delta: Int) {
        audio.moveByBeat(delta)
        revealPlayhead()
    }

    private var zoomSliderBinding: Binding<Double> {
        Binding(
            get: {
                let total = Double(max(1, analysis.totalBeats))
                guard total > 1 else { return 1 }
                return min(max(0, log(total / viewport.visibleBeats) / log(total)), 1)
            },
            set: { value in
                let total = Double(max(1, analysis.totalBeats))
                let visible = total / pow(total, min(max(0, value), 1))
                viewport = viewport.zoomed(to: visible, aroundBeat: currentBeat)
            }
        )
    }

    private var waveformZoomAnchor: LumiWaveformZoomAnchor {
        LumiWaveformZoomAnchor(rawValue: waveformZoomAnchorRaw) ?? .mouse
    }

    private var waveformZoomAnchorBinding: Binding<LumiWaveformZoomAnchor> {
        Binding(
            get: { waveformZoomAnchor },
            set: { waveformZoomAnchorRaw = $0.rawValue }
        )
    }

    private func zoomFromScroll(_ delta: Double, pointerFraction: Double) {
        let boundedDelta = min(max(delta, -24), 24)
        let factor = exp(-boundedDelta * 0.025)
        let anchorBeat = switch waveformZoomAnchor {
        case .mouse:
            viewport.startBeat + viewport.visibleBeats * pointerFraction
        case .playhead:
            currentBeat
        }
        viewport = viewport.zoomed(
            to: viewport.visibleBeats * factor,
            aroundBeat: anchorBeat
        )
    }

    private func revealPlayhead() {
        guard currentBeat < viewport.startBeat || currentBeat >= viewport.endBeat else { return }
        viewport = viewport.centered(onBeat: currentBeat)
    }

    private func placePhrasePoint(roleID: String) {
        let beat = quantizedPlayheadBeat
        guard beat < analysis.totalBeats,
              let containingIndex = analysis.phrases.firstIndex(where: {
                  beat >= $0.startBeat && beat < $0.endBeat
              }),
              let phraseIndex = UInt16(exactly: containingIndex) else { return }
        let phrase = analysis.phrases[containingIndex]
        if beat == phrase.startBeat {
            if phrase.roleID != roleID {
                onTimelineEdit(.changeRole(phraseIndex: phraseIndex, roleID: roleID))
            }
        } else {
            onTimelineEdit(.create(startBeat: beat, endBeat: phrase.endBeat, roleID: roleID))
        }
    }

    private func interpolatedWaveformPoint(atBeat beat: Double) -> (low: Double, mid: Double, high: Double) {
        interpolatedWaveformPoint(
            atTimeMillis: TrackEditorCoordinateMapper.timeMillis(atBeat: beat, analysis: analysis)
        )
    }

    private func interpolatedWaveformPoint(
        atTimeMillis timeMillis: UInt64
    ) -> (low: Double, mid: Double, high: Double) {
        guard !analysis.waveform.isEmpty else { return (0, 0, 0) }
        let progress = min(
            max(0, Double(timeMillis) / Double(max(1, analysis.track.durationMillis))),
            1
        )
        let position = progress * Double(max(0, analysis.waveform.count - 1))
        let lower = Int(position.rounded(.down))
        let upper = min(analysis.waveform.count - 1, lower + 1)
        let fraction = position - Double(lower)
        let a = analysis.waveform[lower]
        let b = analysis.waveform[upper]
        func mix(_ lhs: UInt8, _ rhs: UInt8) -> Double {
            (Double(lhs) + (Double(rhs) - Double(lhs)) * fraction) / 255
        }
        return (mix(a.low, b.low), mix(a.mid, b.mid), mix(a.high, b.high))
    }

    private func drawRGBWaveformSample(
        context: inout GraphicsContext,
        x: Double,
        center: Double,
        maximumAmplitude: Double,
        point: (low: Double, mid: Double, high: Double)
    ) {
        let peak = max(point.low, max(point.mid, point.high))
        guard peak > 0.000_1 else { return }
        let amplitude = pow(peak, 0.58) * maximumAmplitude
        // Rekordbox PWV5 packs hue and height into the same channels. Height
        // controls geometry; normalizing the hue prevents quiet samples from
        // being dimmed a second time while preserving their RGB character.
        let red = pow(point.high / peak, 0.72)
        let green = pow(point.mid / peak, 0.72)
        let blue = pow(point.low / peak, 0.72)
        var line = Path()
        line.move(to: CGPoint(x: x, y: center - amplitude))
        line.addLine(to: CGPoint(x: x, y: center + amplitude))
        context.stroke(
            line,
            with: .color(
                Color(red: red, green: green, blue: blue).opacity(0.98)
            ),
            lineWidth: 1
        )
    }

    private func phraseColor(_ roleID: String) -> Color {
        phraseColorPalette.color(for: roleID)
    }
}

private struct TrackEditorVolumeSlider: View {
    @Binding var value: Double
    let accent: Color

    var body: some View {
        GeometryReader { proxy in
            let width = max(1, proxy.size.width)
            ZStack(alignment: .leading) {
                Capsule().fill(Color.white.opacity(0.16)).frame(height: 4)
                Capsule().fill(accent).frame(width: width * value, height: 4)
                Circle()
                    .fill(Color.white)
                    .frame(width: 12, height: 12)
                    .offset(x: max(0, min(width - 12, width * value - 6)))
            }
            .frame(maxHeight: .infinity)
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0).onChanged { drag in
                    value = min(max(0, drag.location.x / width), 1)
                }
            )
        }
        .frame(height: 20)
        .accessibilityValue("\(Int(value * 100)) percent")
        .accessibilityAdjustableAction { direction in
            switch direction {
            case .increment: value = min(1, value + 0.1)
            case .decrement: value = max(0, value - 0.1)
            @unknown default: break
            }
        }
    }
}

private func formatEditorBPM(_ value: UInt64) -> String {
    String(format: "%.1f", Double(value) / 1_000)
}

private func editorCopy(_ key: String) -> String {
    LibraryWorkspaceLocalization.value(key)
}

private func formatEditorTime(_ millis: UInt64) -> String {
    let seconds = millis / 1_000
    return String(format: "%llu:%02llu", seconds / 60, seconds % 60)
}
