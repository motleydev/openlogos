// B072: the workspace shell (specs/22, the B072 brief). Top to bottom: the palette field (the
// only top bar) with the workspace switcher and the text-size menu to its right; the numbered
// columns separated by 1 pt rules; the floating tool well over the columns' leading edge.
// The palette's results drop over the columns, at most half the window tall.
import SwiftUI

struct ShellView: View {
    @StateObject private var model: ShellModel

    init(model: @autoclosure @escaping () -> ShellModel) {
        _model = StateObject(wrappedValue: model())
    }

    var body: some View {
        GeometryReader { geometry in
            VStack(spacing: 0) {
                PaletteBar(model: model)
                Rectangle().fill(model.theme.rulesColor).frame(height: ShellTokens.rule)
                ZStack(alignment: .topLeading) {
                    ColumnsView(model: model)
                    if model.activeWorkspace.notesDockOpen {
                        ShellNotesDock(model: model)
                    }
                    ToolWell(model: model)
                    if model.paletteOpen, let result = model.paletteResult {
                        PaletteResults(model: model, result: result, maxHeight: geometry.size.height * 0.5)
                    }
                    if let toast = model.toast {
                        ToastView(text: toast, theme: model.theme)
                            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
                            .padding(.bottom, ShellTokens.Spacing.xl)
                            .allowsHitTesting(false)
                    }
                }
            }
        }
        .background(model.theme.pageColor)
        .foregroundStyle(model.theme.textColor)
        .tint(model.theme.accentColor)
        .preferredColorScheme(model.theme.isDark ? .dark : .light)
        #if os(macOS)
        .onAppear {
            model.installKeyMonitor()
            // The reading surface, not the palette field, has focus at launch.
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                model.focusActiveTab()
                model.launched = true
            }
        }
        .onDisappear { model.removeKeyMonitor() }
        #endif
        .task { await ShellLaunch.apply(to: model) }
    }
}

// MARK: - Palette field

struct PaletteBar: View {
    @ObservedObject var model: ShellModel
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: ShellTokens.Spacing.s) {
            HStack(spacing: ShellTokens.Spacing.s) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(model.theme.chromeColor)
                    .accessibilityHidden(true)
                TextField("", text: $model.paletteText, prompt: Text("Passage, word, command, tab…  ⌘K").foregroundStyle(model.theme.chromeColor))
                    .textFieldStyle(.plain)
                    .font(ShellTokens.chromeFont)
                    .focused($focused)
                    .accessibilityLabel("Command palette")
                    .accessibilityIdentifier("shell-palette-field")
                    .onChange(of: model.paletteText) { _, _ in model.paletteTextChanged() }
                    // A click in the field opens the list; the launch-time default focus does not.
                    .onChange(of: focused) { _, isFocused in if isFocused && model.launched && !model.paletteOpen { model.focusPalette() } }
                    .onChange(of: model.paletteFocusRequest) { _, _ in focused = true }
                    .onChange(of: model.paletteOpen) { _, open in if !open { focused = false } }
                    .onKeyPress(.downArrow) { model.movePaletteSelection(1); return .handled }
                    .onKeyPress(.upArrow) { model.movePaletteSelection(-1); return .handled }
                    .onKeyPress(.escape) { model.closePalette(); return .handled }
                    .onKeyPress(.tab) { model.toggleExpandSelectedSection(); return .handled }
                    .onKeyPress(.return, phases: .down) { press in
                        if press.modifiers.contains(.option) {
                            model.toggleExpandSelectedSection()
                        } else {
                            model.runSelectedRow()
                        }
                        return .handled
                    }
            }
            .padding(.horizontal, ShellTokens.Spacing.m)
            .frame(maxHeight: .infinity)
            WorkspaceSwitcher(model: model)
            TextSizeMenu(model: model)
        }
        .padding(.trailing, ShellTokens.Spacing.s)
        .frame(height: ShellTokens.paletteHeight)
        .background(model.theme.pageColor)
    }
}

struct WorkspaceSwitcher: View {
    @ObservedObject var model: ShellModel

    var body: some View {
        let ws = model.activeWorkspace
        Menu {
            ForEach(model.session.workspaces, id: \.id) { workspace in
                Button {
                    model.run("workspace.switch", args: ShellCommandArgs(workspaceId: workspace.id))
                } label: {
                    if workspace.id == ws.id {
                        Label(workspace.title, systemImage: "checkmark")
                    } else {
                        Text(workspace.title)
                    }
                }
            }
            Divider()
            ForEach(["workspace.new", "workspace.duplicate", "workspace.close", "workspace.next", "workspace.previous"], id: \.self) { id in
                CommandButton(model: model, id: id)
            }
        } label: {
            Text("\(ws.title) · \(model.activeWorkspaceIndex + 1)/\(model.session.workspaces.count)")
                .font(ShellTokens.chromeFont)
                .foregroundStyle(model.theme.chromeColor)
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .accessibilityLabel("Workspace \(ws.title), \(model.activeWorkspaceIndex + 1) of \(model.session.workspaces.count)")
        .accessibilityIdentifier("shell-workspace-switcher")
    }
}

struct TextSizeMenu: View {
    @ObservedObject var model: ShellModel

    var body: some View {
        Menu {
            CommandButton(model: model, id: "view.text_size_down", title: "A−  Smaller")
            CommandButton(model: model, id: "view.text_size_reset", title: "A  Default size")
            CommandButton(model: model, id: "view.text_size_up", title: "A+  Larger")
            Divider()
            // Line spacing has no registered command yet: a host setting (see the B072 report).
            Picker("Line spacing", selection: $model.spacingIndex) {
                ForEach(ShellTokens.lineSpacings.indices, id: \.self) { index in
                    Text(String(format: "%.2g", ShellTokens.lineSpacings[index])).tag(index)
                }
            }
            Divider()
            ForEach(ShellTokens.themes) { theme in
                Button {
                    model.run("view.theme", args: ShellCommandArgs(text: theme.id))
                } label: {
                    if model.theme == theme { Label(theme.title, systemImage: "checkmark") } else { Text(theme.title) }
                }
            }
        } label: {
            Text("Aa \(Int(model.readingSize))")
                .font(ShellTokens.chromeFont)
                .foregroundStyle(model.theme.chromeColor)
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .accessibilityLabel("Text size \(Int(model.readingSize)) points, line spacing \(String(format: "%.2g", model.lineSpacing)), \(model.theme.title) theme")
        .accessibilityIdentifier("shell-text-menu")
    }
}

/// A menu button for a registered command: its title, its Mac shortcut, disabled with the
/// reason when the core says so.
struct CommandButton: View {
    @ObservedObject var model: ShellModel
    let id: String
    var title: String?
    var args = ShellCommandArgs()

    var body: some View {
        let info = model.command(id)
        let reason = model.disabledReason(id)
        Button {
            model.run(id, args: args)
        } label: {
            Text(title ?? info?.title ?? id) + Text(model.shortcut(id).map { "   \($0)" } ?? "").foregroundColor(.secondary)
        }
        .disabled(reason != nil)
        .help(reason ?? info?.title ?? id)
    }
}

// MARK: - Palette results

struct PaletteResults: View {
    @ObservedObject var model: ShellModel
    let result: ShellPaletteResult
    let maxHeight: CGFloat

    var body: some View {
        let rows = model.paletteRows
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    if let notice = result.notice {
                        Text(notice).font(ShellTokens.chromeFont).foregroundStyle(model.theme.chromeColor)
                            .padding(ShellTokens.Spacing.m)
                    }
                    if rows.isEmpty, result.notice == nil {
                        Text(model.paletteText.isEmpty ? "Type a passage, a command (>), a tab or workspace (@), or words to find (#)" : "No results").font(ShellTokens.chromeFont).foregroundStyle(model.theme.chromeColor)
                            .padding(ShellTokens.Spacing.m)
                    }
                    ForEach(Array(result.groups.enumerated()), id: \.offset) { _, group in
                        Text(Self.heading(group.section))
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(model.theme.chromeColor)
                            .textCase(.uppercase)
                            .padding(.horizontal, ShellTokens.Spacing.m)
                            .padding(.top, ShellTokens.Spacing.s)
                            .padding(.bottom, ShellTokens.Spacing.xs)
                            .accessibilityAddTraits(.isHeader)
                        ForEach(Array(group.items.enumerated()), id: \.offset) { _, item in
                            let index = rows.firstIndex(of: item) ?? 0
                            PaletteRow(model: model, item: item, selected: index == model.paletteSelection)
                                .id(index)
                                .onTapGesture { model.runRow(item) }
                        }
                        if group.total > UInt32(group.items.count) || model.expandedSection == group.section {
                            Button {
                                model.expand(group.section)
                            } label: {
                                Text(model.expandedSection == group.section ? "Show fewer" : "Show all \(group.total)  ⌥↩")
                                    .font(ShellTokens.chromeFont)
                                    .foregroundStyle(model.theme.accentColor)
                            }
                            .buttonStyle(.plain)
                            .padding(.horizontal, ShellTokens.Spacing.m)
                            .padding(.vertical, ShellTokens.Spacing.xs)
                        }
                    }
                }
                .padding(.vertical, ShellTokens.Spacing.xs)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .onChange(of: model.paletteSelection) { _, index in proxy.scrollTo(index) }
        }
        .frame(maxHeight: maxHeight)
        .fixedSize(horizontal: false, vertical: true)
        .background(model.theme.wellColor)
        .overlay(alignment: .bottom) { Rectangle().fill(model.theme.rulesColor).frame(height: 1) }
        .shadow(color: .black.opacity(model.theme.isDark ? 0.5 : 0.12), radius: 12, y: 6)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Palette results")
        .accessibilityIdentifier("shell-palette-results")
    }

    static func heading(_ section: ShellPaletteSection) -> String {
        switch section {
        case .passage: return "Passage"
        case .commands: return "Commands"
        case .tabs: return "Tabs and workspaces"
        case .text: return "Text"
        }
    }
}

struct PaletteRow: View {
    @ObservedObject var model: ShellModel
    let item: ShellPaletteItem
    let selected: Bool

    var body: some View {
        let disabled = item.disabledReason != nil
        HStack(alignment: .firstTextBaseline, spacing: ShellTokens.Spacing.s) {
            Text(item.title)
                .font(ShellTokens.chromeFont)
                .lineLimit(1)
            if let subtitle = subtitleText {
                Text(subtitle)
                    .font(.system(size: 12))
                    .foregroundStyle(model.theme.chromeColor)
                    .lineLimit(1)
            }
            Spacer(minLength: ShellTokens.Spacing.s)
            if let reason = item.disabledReason {
                Text(reason).font(.system(size: 12)).foregroundStyle(model.theme.chromeColor).lineLimit(1)
            } else if let shortcut = item.shortcut {
                Text(ShellModel.symbols(shortcut))
                    .font(.system(size: 12).monospaced())
                    .foregroundStyle(model.theme.chromeColor)
            }
        }
        .opacity(disabled ? 0.5 : 1)
        .padding(.horizontal, ShellTokens.Spacing.m)
        .frame(height: 28)
        .background(
            RoundedRectangle(cornerRadius: ShellTokens.Radius.control)
                .fill(selected ? model.theme.accentColor.opacity(0.16) : .clear)
                .padding(.horizontal, ShellTokens.Spacing.xs)
        )
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? [.isButton, .isSelected] : .isButton)
        .accessibilityHint(item.disabledReason ?? "")
    }

    private var subtitleText: String? {
        // Text hits carry a snippet; commands a shortcut shown at the trailing edge instead.
        item.subtitle
    }
}

// MARK: - Notes dock

struct ShellNotesDock: View {
    @ObservedObject var model: ShellModel

    var body: some View {
        let note = model.activeWorkspace.notesDockNoteId ?? "fixture:note:1"
        VStack(spacing: 0) {
            Spacer()
            VStack(spacing: 0) {
                Rectangle().fill(model.theme.rulesColor).frame(height: 1)
                HStack {
                    Text("Notes").font(ShellTokens.chromeFont.weight(.semibold))
                    Spacer()
                    CommandButton(model: model, id: "view.notes_dock", title: "Close")
                        .buttonStyle(.plain)
                        .font(ShellTokens.chromeFont)
                }
                .padding(.horizontal, ShellTokens.Spacing.m)
                .frame(height: ShellTokens.identityRowHeight)
                if let text = model.textSource.text(resourceID: note, reference: nil) {
                    ReaderTextView(paneID: "dock", content: text.content, horizontalPadding: ShellTokens.Spacing.l, typography: model.typography)
                } else {
                    PlaceholderView(model: model, resource: model.textSource.resource(note), reference: nil)
                }
            }
            .frame(height: 200)
            .background(model.theme.pageColor)
        }
    }
}

// MARK: - Toast

struct ToastView: View {
    let text: String
    let theme: ShellTokens.Theme

    var body: some View {
        Text(text)
            .font(ShellTokens.chromeFont)
            .padding(.horizontal, ShellTokens.Spacing.l)
            .padding(.vertical, ShellTokens.Spacing.s)
            .background(Capsule().fill(theme.textColor))
            .foregroundStyle(theme.pageColor)
            .accessibilityIdentifier("shell-toast")
            .accessibilityAddTraits(.updatesFrequently)
    }
}
