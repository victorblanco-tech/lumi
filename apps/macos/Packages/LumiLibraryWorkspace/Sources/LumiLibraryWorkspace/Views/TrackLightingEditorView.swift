import LumiDesignSystem
import SwiftUI

public struct TrackLightingEditorView: View {
    private let analysis: TrackEditorAnalysis
    private let keyNotation: KeyNotationPreference
    private let feedback: String?
    private let rendersInteractiveControls: Bool
    private let onTimelineEdit: @MainActor (TrackTimelineEditRequest) -> Void
    private let onTimelineHistory: @MainActor (TrackTimelineHistoryRequest) -> Void
    @StateObject private var audio: TrackAudioPreviewController
    @State private var viewport: TrackEditorViewport
    @State private var selectedPhraseID: UInt64?
    @State private var loopSelectedPhrase = false
    @State private var selectionStartBar: UInt32
    @State private var selectionEndBar: UInt32
    @State private var selectionAnchorBar: UInt32?
    @Environment(\.dismiss) private var dismiss

    private let background = Color(red: 0.025, green: 0.032, blue: 0.045)
    private let panel = Color(red: 0.055, green: 0.070, blue: 0.095)
    private let primary = Color(red: 0.94, green: 0.97, blue: 1)
    private let secondary = Color(red: 0.56, green: 0.64, blue: 0.73)
    private let accent = Color(red: 0.25, green: 0.76, blue: 1)

    public init(
        analysis: TrackEditorAnalysis,
        keyNotation: KeyNotationPreference,
        feedback: String? = nil,
        rendersInteractiveControls: Bool = true,
        onTimelineEdit: @escaping @MainActor (TrackTimelineEditRequest) -> Void = { _ in },
        onTimelineHistory: @escaping @MainActor (TrackTimelineHistoryRequest) -> Void = { _ in }
    ) {
        self.analysis = analysis
        self.keyNotation = keyNotation
        self.feedback = feedback
        self.rendersInteractiveControls = rendersInteractiveControls
        self.onTimelineEdit = onTimelineEdit
        self.onTimelineHistory = onTimelineHistory
        _audio = StateObject(wrappedValue: TrackAudioPreviewController(analysis: analysis))
        _viewport = State(
            initialValue: TrackEditorViewport(
                startBar: 0,
                visibleBars: min(8, max(1, analysis.totalBars)),
                totalBars: analysis.totalBars,
                beatsPerBar: analysis.beatsPerBar
            )
        )
        _selectedPhraseID = State(initialValue: analysis.phrases.first?.id)
        let firstStart = analysis.phrases.first?.startBeat ?? 0
        let firstEnd = analysis.phrases.first?.endBeat ?? UInt32(analysis.beatsPerBar)
        _selectionStartBar = State(
            initialValue: firstStart / UInt32(max(1, analysis.beatsPerBar))
        )
        _selectionEndBar = State(
            initialValue: firstEnd / UInt32(max(1, analysis.beatsPerBar))
        )
    }

    public var body: some View {
        VStack(spacing: 0) {
            header
            Divider().overlay(Color.white.opacity(0.12))
            transport
            editToolbar
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
        .frame(minWidth: 980, idealWidth: 1_160, minHeight: 620, idealHeight: 720)
        .preferredColorScheme(.dark)
        .accessibilityIdentifier("lumi.trackEditor")
        .focusable()
        .onKeyPress(.space) {
            audio.togglePlayback()
            return .handled
        }
        .onKeyPress(.leftArrow) {
            audio.moveByBar(-1)
            revealPlayhead()
            return .handled
        }
        .onKeyPress(.rightArrow) {
            audio.moveByBar(1)
            revealPlayhead()
            return .handled
        }
        .onChange(of: audio.positionMillis) { _, _ in
            if audio.isPlaying { revealPlayhead() }
        }
        .onChange(of: analysis) { previous, current in
            adoptTimelineUpdate(previous: previous, current: current)
        }
        .onDisappear { audio.shutdown() }
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
            Button(editorCopy("editor.close")) { dismiss() }
                .buttonStyle(.bordered)
                .accessibilityIdentifier("lumi.trackEditor.close")
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 15)
        .background(panel)
    }

    private var transport: some View {
        HStack(spacing: 12) {
            Button { stepBar(-1) } label: {
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

            Button { stepBar(1) } label: {
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

            HStack(spacing: 6) {
                Button { zoom(by: 1) } label: {
                    Image(systemName: "minus.magnifyingglass")
                }
                .disabled(zoomOptions.firstIndex(of: viewport.visibleBars) == zoomOptions.count - 1)
                Text(String(format: editorCopy("editor.visibleBars"), UInt64(viewport.visibleBars)))
                    .font(.system(size: 11, weight: .semibold, design: .monospaced))
                    .frame(width: 48)
                Button { zoom(by: -1) } label: {
                    Image(systemName: "plus.magnifyingglass")
                }
                .disabled(zoomOptions.firstIndex(of: viewport.visibleBars) == 0)
            }
            .accessibilityElement(children: .combine)
            .accessibilityLabel(
                String(format: editorCopy("editor.visibleBarsLabel"), UInt64(viewport.visibleBars))
            )
            .accessibilityIdentifier("lumi.trackEditor.zoom")
        }
        .padding(.horizontal, 20)
        .frame(height: 58)
        .background(background)
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
        .controlSize(.small)
        .padding(.horizontal, 20)
        .frame(height: 44)
        .background(panel.opacity(0.72))
    }

    private var phraseInspector: some View {
        VStack(alignment: .leading, spacing: 14) {
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
                        ForEach(analysis.roles) { role in
                            Text(role.name).tag(role.id)
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
                    title: editorCopy("editor.startBar"),
                    displayValue: phraseStartBarZeroBased(phrase) + 1,
                    canDecrease: canMoveStart(by: -1),
                    canIncrease: canMoveStart(by: 1),
                    decrease: { moveStartBoundary(by: -1) },
                    increase: { moveStartBoundary(by: 1) }
                )
                boundaryRow(
                    title: editorCopy("editor.endBar"),
                    displayValue: phraseEndBarZeroBased(phrase),
                    canDecrease: canMoveEnd(by: -1),
                    canIncrease: canMoveEnd(by: 1),
                    decrease: { moveEndBoundary(by: -1) },
                    increase: { moveEndBoundary(by: 1) }
                )

                Divider().overlay(Color.white.opacity(0.12))
                inspectorLabel(editorCopy("editor.barSelection"))
                HStack {
                    selectionStepper(
                        title: editorCopy("editor.from"),
                        value: $selectionStartBar,
                        range: 0...max(0, selectionEndBar - 1)
                    )
                    selectionStepper(
                        title: editorCopy("editor.to"),
                        value: $selectionEndBar,
                        range: min(analysis.totalBars, selectionStartBar + 1)...analysis.totalBars
                    )
                }
                Button {
                    onTimelineEdit(
                        .create(
                            startBar: selectionStartBar,
                            endBar: selectionEndBar,
                            roleID: phrase.roleID
                        )
                    )
                } label: {
                    Label(editorCopy("editor.createSelection"), systemImage: "plus.rectangle.on.rectangle")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .tint(accent)
                .disabled(selectionStartBar >= selectionEndBar)
                .accessibilityIdentifier("lumi.trackEditor.createSelection")

                Divider().overlay(Color.white.opacity(0.12))
                inspectorFact(editorCopy("editor.origin"), phrase.origin)
                inspectorFact(editorCopy("editor.loopStrategy"), phrase.loopStrategy.uppercased())
                inspectorFact(editorCopy("editor.revisionReason"), readableReason(analysis.timeline.reason))
            } else {
                Text(editorCopy("editor.noPhrase"))
                    .foregroundStyle(secondary)
            }

            Spacer()
            Label(editorCopy("editor.planIsolation"), systemImage: "lock.shield")
                .font(.system(size: 10, weight: .medium))
                .foregroundStyle(secondary)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityIdentifier("lumi.trackEditor.planIsolation")
        }
        .padding(14)
        .background(panel)
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .overlay {
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color.white.opacity(0.12), lineWidth: 1)
        }
        .accessibilityIdentifier("lumi.trackEditor.inspector")
    }

    private var editorCanvas: some View {
        GeometryReader { proxy in
            Canvas { context, size in
                drawEditor(context: &context, size: size)
            }
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { value in
                        if value.location.y < phraseLaneTop(height: proxy.size.height) {
                            seek(atX: value.location.x, width: proxy.size.width)
                        } else {
                            updateBarSelection(atX: value.location.x, width: proxy.size.width)
                        }
                    }
                    .onEnded { value in
                        if value.location.y >= phraseLaneTop(height: proxy.size.height) {
                            if abs(value.translation.width) < 3 {
                                selectPhrase(atX: value.location.x, width: proxy.size.width)
                            }
                            selectionAnchorBar = nil
                        }
                    }
            )
        }
        .frame(minHeight: 315)
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
                        let bar = UInt32(progress * Double(max(1, analysis.totalBars - 1)))
                        viewport = TrackEditorViewport(
                            startBar: bar,
                            visibleBars: viewport.visibleBars,
                            totalBars: analysis.totalBars,
                            beatsPerBar: analysis.beatsPerBar
                        )
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
                Circle().fill(phraseColor(phrase.role)).frame(width: 9, height: 9)
                Text("\(phrase.role) · bars \(phraseStartBar(phrase))–\(phraseEndBar(phrase)) · \(phrase.origin)")
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

        for beat in viewport.startBeat...viewport.endBeat {
            let x = viewport.x(forBeat: Double(beat), width: width)
            let isBar = beat.isMultiple(of: UInt32(analysis.beatsPerBar))
            var line = Path()
            line.move(to: CGPoint(x: x, y: isBar ? 22 : 39))
            line.addLine(to: CGPoint(x: x, y: phraseTop - 4))
            context.stroke(
                line,
                with: .color(Color.white.opacity(isBar ? 0.28 : 0.10)),
                lineWidth: isBar ? 1.2 : 0.6
            )
            if isBar, beat < viewport.endBeat {
                let label = Text("\(beat / UInt32(analysis.beatsPerBar) + 1)")
                    .font(.system(size: 11, weight: .bold, design: .monospaced))
                    .foregroundColor(primary)
                context.draw(label, at: CGPoint(x: x + 5, y: 13), anchor: .topLeading)
            }
            if beat < viewport.endBeat {
                let beatLabel = Text("\(beat % UInt32(analysis.beatsPerBar) + 1)")
                    .font(.system(size: 8, weight: .medium, design: .monospaced))
                    .foregroundColor(secondary)
                context.draw(beatLabel, at: CGPoint(x: x + 3, y: 31), anchor: .topLeading)
            }
        }

        let totalBeats = max(1, analysis.beats.count)
        let sampleWidth = max(1, width / Double(viewport.visibleBeats))
        for (index, point) in analysis.waveform.enumerated() {
            let beat = Double(index) / Double(max(1, analysis.waveform.count - 1)) * Double(totalBeats)
            guard beat >= Double(viewport.startBeat), beat <= Double(viewport.endBeat) else { continue }
            let x = viewport.x(forBeat: beat, width: width)
            let low = Double(point.low) / 255 * amplitude
            let mid = Double(point.mid) / 255 * amplitude * 0.88
            let high = Double(point.high) / 255 * amplitude * 0.76
            context.fill(
                Path(CGRect(x: x, y: center - low, width: sampleWidth, height: low * 2)),
                with: .color(Color(red: 0.14, green: 0.34, blue: 0.95).opacity(0.62))
            )
            context.fill(
                Path(CGRect(x: x, y: center - mid, width: sampleWidth, height: mid * 2)),
                with: .color(Color(red: 0.24, green: 0.86, blue: 0.78).opacity(0.76))
            )
            context.fill(
                Path(CGRect(x: x, y: center - high, width: sampleWidth, height: high * 2)),
                with: .color(Color(red: 1.0, green: 0.78, blue: 0.25).opacity(0.86))
            )
        }

        for phrase in analysis.phrases where phrase.endBeat > viewport.startBeat && phrase.startBeat < viewport.endBeat {
            let start = viewport.x(forBeat: Double(phrase.startBeat), width: width)
            let end = viewport.x(forBeat: Double(phrase.endBeat), width: width)
            let rect = CGRect(x: start + 1, y: phraseTop, width: max(2, end - start - 2), height: 54)
            context.fill(Path(roundedRect: rect, cornerRadius: 4), with: .color(phraseColor(phrase.role).opacity(0.74)))
            if phrase.id == selectedPhraseID {
                context.stroke(Path(roundedRect: rect, cornerRadius: 4), with: .color(.white), lineWidth: 2)
            }
            let label = Text(phrase.role.uppercased())
                .font(.system(size: 10, weight: .bold, design: .monospaced))
                .foregroundColor(.white)
            context.draw(label, at: CGPoint(x: rect.minX + 7, y: rect.midY), anchor: .leading)
        }

        let selectionStartBeat = selectionStartBar * UInt32(analysis.beatsPerBar)
        let selectionEndBeat = selectionEndBar * UInt32(analysis.beatsPerBar)
        if selectionEndBeat > viewport.startBeat && selectionStartBeat < viewport.endBeat {
            let start = viewport.x(forBeat: Double(selectionStartBeat), width: width)
            let end = viewport.x(forBeat: Double(selectionEndBeat), width: width)
            let rect = CGRect(
                x: start,
                y: phraseTop - 4,
                width: max(2, end - start),
                height: 62
            )
            context.fill(Path(rect), with: .color(accent.opacity(0.10)))
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
    }

    private func drawOverview(context: inout GraphicsContext, size: CGSize) {
        let width = Double(size.width)
        for (index, point) in analysis.waveform.enumerated() {
            let x = Double(index) / Double(max(1, analysis.waveform.count - 1)) * width
            let height = Double(max(point.low, max(point.mid, point.high))) / 255 * 42
            context.fill(
                Path(CGRect(x: x, y: (Double(size.height) - height) / 2, width: max(1, width / Double(max(1, analysis.waveform.count))), height: height)),
                with: .color(accent.opacity(0.65))
            )
        }
        let start = Double(viewport.startBar) / Double(max(1, analysis.totalBars)) * width
        let visible = Double(viewport.visibleBars) / Double(max(1, analysis.totalBars)) * width
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

    private var zoomOptions: [UInt32] {
        let standard: [UInt32] = [1, 2, 4, 8, 16, 32]
        let options = standard.filter { $0 <= analysis.totalBars }
        return options.isEmpty ? [1] : options
    }

    private var selectedPhrase: TrackEditorPhrase? {
        analysis.phrases.first { $0.id == selectedPhraseID }
    }

    private var selectedPhraseIndex: Int? {
        analysis.phrases.firstIndex { $0.id == selectedPhraseID }
    }

    private var canSplitSelectedPhrase: Bool {
        guard let phrase = selectedPhrase else { return false }
        return phraseEndBarZeroBased(phrase) - phraseStartBarZeroBased(phrase) > 1
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

    private func inspectorFact(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label)
                .foregroundStyle(secondary)
            Spacer()
            Text(value)
                .foregroundStyle(primary)
                .multilineTextAlignment(.trailing)
        }
        .font(.system(size: 10, weight: .medium, design: .monospaced))
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
            "Bar \(value.wrappedValue + (title == editorCopy("editor.to") ? 0 : 1))"
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
        let start = phraseStartBarZeroBased(phrase)
        let end = phraseEndBarZeroBased(phrase)
        let boundary = min(max(start + 1, currentBarIndex), end - 1)
        onTimelineEdit(.split(phraseIndex: phraseIndex, atBar: boundary))
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
        let target = Int(phraseStartBarZeroBased(phrase)) + delta
        return target > Int(phraseStartBarZeroBased(analysis.phrases[index - 1]))
            && target < Int(phraseEndBarZeroBased(phrase))
    }

    private func canMoveEnd(by delta: Int) -> Bool {
        guard let phrase = selectedPhrase,
              let index = selectedPhraseIndex,
              index + 1 < analysis.phrases.count else { return false }
        let target = Int(phraseEndBarZeroBased(phrase)) + delta
        return target > Int(phraseStartBarZeroBased(phrase))
            && target < Int(phraseEndBarZeroBased(analysis.phrases[index + 1]))
    }

    private func moveStartBoundary(by delta: Int) {
        guard canMoveStart(by: delta),
              let phrase = selectedPhrase,
              let index = selectedPhraseIndex,
              let boundaryIndex = UInt16(exactly: index - 1) else { return }
        let target = UInt32(Int(phraseStartBarZeroBased(phrase)) + delta)
        onTimelineEdit(.moveBoundary(afterPhraseIndex: boundaryIndex, toBar: target))
    }

    private func moveEndBoundary(by delta: Int) {
        guard canMoveEnd(by: delta),
              let phrase = selectedPhrase,
              let index = selectedPhraseIndex,
              let boundaryIndex = UInt16(exactly: index) else { return }
        let target = UInt32(Int(phraseEndBarZeroBased(phrase)) + delta)
        onTimelineEdit(.moveBoundary(afterPhraseIndex: boundaryIndex, toBar: target))
    }

    private func updateBarSelection(atX x: Double, width: Double) {
        let beat = max(0, viewport.beat(atX: x, width: width))
        let bar = TrackEditorEditGeometry.containingBar(
            beat: beat,
            beatsPerBar: analysis.beatsPerBar,
            totalBars: analysis.totalBars
        )
        let anchor = selectionAnchorBar ?? bar
        selectionAnchorBar = anchor
        let selection = TrackEditorEditGeometry.barSelection(
            anchorBar: anchor,
            currentBar: bar,
            totalBars: analysis.totalBars
        )
        selectionStartBar = selection.lowerBound
        selectionEndBar = selection.upperBound
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
            selectionStartBar = phrase.startBeat / UInt32(max(1, current.beatsPerBar))
            selectionEndBar = phrase.endBeat / UInt32(max(1, current.beatsPerBar))
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

    private var currentBarIndex: UInt32 {
        UInt32(max(0, Int(currentBeat) / Int(max(1, analysis.beatsPerBar))))
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
        selectionStartBar = phraseStartBarZeroBased(phrase)
        selectionEndBar = phraseEndBarZeroBased(phrase)
        if loopSelectedPhrase { audio.setLoop(phrase) }
    }

    private func stepBar(_ delta: Int) {
        audio.moveByBar(delta)
        revealPlayhead()
    }

    private func zoom(by delta: Int) {
        guard let index = zoomOptions.firstIndex(of: viewport.visibleBars) else { return }
        let next = min(max(0, index + delta), zoomOptions.count - 1)
        viewport = viewport.zoomed(to: zoomOptions[next], aroundBar: currentBarIndex)
    }

    private func revealPlayhead() {
        let target = currentBarIndex
        guard target < viewport.startBar || target >= viewport.startBar + viewport.visibleBars else { return }
        viewport = TrackEditorViewport(
            startBar: target,
            visibleBars: viewport.visibleBars,
            totalBars: analysis.totalBars,
            beatsPerBar: analysis.beatsPerBar
        )
    }

    private func phraseStartBar(_ phrase: TrackEditorPhrase) -> UInt32 {
        phraseStartBarZeroBased(phrase) + 1
    }

    private func phraseEndBar(_ phrase: TrackEditorPhrase) -> UInt32 {
        phraseEndBarZeroBased(phrase)
    }

    private func phraseStartBarZeroBased(_ phrase: TrackEditorPhrase) -> UInt32 {
        phrase.startBeat / UInt32(max(1, analysis.beatsPerBar))
    }

    private func phraseEndBarZeroBased(_ phrase: TrackEditorPhrase) -> UInt32 {
        max(phrase.startBeat + 1, phrase.endBeat) / UInt32(max(1, analysis.beatsPerBar))
    }

    private func phraseColor(_ role: String) -> Color {
        switch role.lowercased() {
        case "intro", "outro": Color(red: 0.25, green: 0.55, blue: 0.95)
        case "bridge": Color(red: 0.37, green: 0.42, blue: 0.78)
        case "breakdown", "breakdown 1", "breakdown 2", "breakdown 3": Color(red: 0.48, green: 0.28, blue: 0.83)
        case "synth": Color(red: 0.82, green: 0.24, blue: 0.72)
        case "pre-drop": Color(red: 0.95, green: 0.46, blue: 0.20)
        case "build", "buildup 1", "buildup 2", "buildup 3": Color(red: 0.96, green: 0.66, blue: 0.12)
        case "drop": Color(red: 0.92, green: 0.20, blue: 0.26)
        default: Color(red: 0.20, green: 0.68, blue: 0.60)
        }
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
