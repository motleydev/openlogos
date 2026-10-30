// B072: the workspace shell (specs/22, the B072 brief). Top to bottom: the palette field (the
// only top bar; on the Mac it is the window's unified toolbar row, beside the traffic lights)
// with the workspace switcher and the text-size menu to its right; the numbered columns
// separated by 1 pt rules; the floating tool well over the columns' leading edge. The
// palette's results drop under the field, 770 pt wide, at most half the window tall.
import SwiftUI

struct ShellView: View {
    @StateObject private var model: ShellModel
    /// The window's width, for the field's width in the toolbar row.
    @State private var width: CGFloat = 1440

    init(model: @autoclosure @escaping () -> ShellModel) {
        _model = StateObject(wrappedValue: model())
    }

    var body: some View {
        GeometryReader { geometry in
            VStack(spacing: 0) {
                #if !os(macOS)
                PaletteBar(model: model, palette: model.palette)
                #endif
                Rectangle().fill(model.theme.rulesColor).frame(height: ShellTokens.rule)
                ZStack(alignment: .topLeading) {
                    ColumnsView(model: model)
                    if model.activeWorkspace.notesDockOpen {
                        ShellNotesDock(model: model)
                    }
                    ToolWell(model: model)
                    PaletteOverlay(model: model, palette: model.palette, maxHeight: geometry.size.height * ShellTokens.paletteResultsMaxFraction, width: Self.fieldWidth(geometry.size.width))
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
        .toolbar {
            if #available(macOS 26.0, *) {
                // No glass capsule behind the items: the row is the page colour of the theme.
                ToolbarItem(placement: .principal) { paletteField }
                    .sharedBackgroundVisibility(.hidden)
                ToolbarItem(placement: .primaryAction) { trailingMenus }
                    .sharedBackgroundVisibility(.hidden)
            } else {
                ToolbarItem(placement: .principal) { paletteField }
                ToolbarItem(placement: .primaryAction) { trailingMenus }
            }
        }
        .toolbar(removing: .title)
        .toolbarBackground(model.theme.pageColor, for: .windowToolbar)
        .toolbarBackground(.visible, for: .windowToolbar)
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
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
    }

    #if os(macOS)
    private var paletteField: some View {
        PaletteField(model: model, palette: model.palette)
            .frame(width: Self.fieldWidth(width))
    }

    private var trailingMenus: some View {
        HStack(spacing: ShellTokens.Spacing.m) {
            WorkspaceSwitcher(model: model)
            TextSizeMenu(model: model)
        }
    }
    #endif

    /// 770 pt, less what the traffic lights and the trailing menus need in a narrow window.
    static func fieldWidth(_ window: CGFloat) -> CGFloat {
        max(240, min(ShellTokens.paletteDropdownMaxWidth, window - 360))
    }
}

// MARK: - Palette field

/// The field with the switcher and the text menu beside it (iPad; the Mac puts the same
/// pieces in the window's toolbar row).
struct PaletteBar: View {
    @ObservedObject var model: ShellModel
    @ObservedObject var palette: PaletteState

    var body: some View {
        HStack(spacing: ShellTokens.Spacing.s) {
            PaletteField(model: model, palette: palette)
            WorkspaceSwitcher(model: model)
            TextSizeMenu(model: model)
        }
        .padding(.trailing, ShellTokens.Spacing.s)
        .frame(height: ShellTokens.paletteHeight)
        .background(model.theme.pageColor)
    }
}

struct PaletteField: View {
    @ObservedObject var model: ShellModel
    @ObservedObject var palette: PaletteState
    @FocusState private var focused: Bool

    var body: some View {
            HStack(spacing: ShellTokens.Spacing.s) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(model.theme.chromeColor)
                    .accessibilityHidden(true)
                TextField("", text: $palette.text, prompt: Text("Passage, word, command, tab…  ⌘K").foregroundStyle(model.theme.chromeColor))
                    .textFieldStyle(.plain)
                    .font(ShellTokens.chromeFont)
                    .focused($focused)
                    .accessibilityLabel("Command palette")
                    .accessibilityIdentifier("shell-palette-field")
                    .onChange(of: palette.text) { _, _ in model.paletteTextChanged() }
                    // A click in the field opens the list; the launch-time default focus does not.
                    .onChange(of: focused) { _, isFocused in if isFocused && model.launched && !model.paletteOpen { model.focusPalette() } }
                    .onChange(of: palette.focusRequest) { _, _ in focused = true }
                    .onChange(of: palette.open) { _, open in if !open { focused = false } }
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
            #if os(macOS)
            .frame(height: 30)
            .background(RoundedRectangle(cornerRadius: ShellTokens.Radius.control).fill(model.theme.wellColor))
            .overlay(RoundedRectangle(cornerRadius: ShellTokens.Radius.control).strokeBorder(model.theme.rulesColor, lineWidth: 1))
            #else
            .frame(maxHeight: .infinity)
            #endif
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
            Menu("Line spacing") {
                ForEach(ShellTokens.lineSpacings, id: \.self) { spacing in
                    let current = abs(model.lineSpacing - spacing) < 0.001
                    Button {
                        model.run("view.line_spacing", args: ShellCommandArgs(number: Double(spacing)))
                    } label: {
                        let title = String(format: "%.2g", spacing)
                        if current { Label(title, systemImage: "checkmark") } else { Text(title) }
                    }
                }
            }
            Divider()
            ForEach(["system"] + ShellTokens.themes.map(\.id), id: \.self) { id in
                Button {
                    model.run("view.theme", args: ShellCommandArgs(text: id))
                } label: {
                    let title = ShellTokens.theme(named: id)?.title ?? "System"
                    if model.session.reading.theme == id { Label(title, systemImage: "checkmark") } else { Text(title) }
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

/// The result list, shown while the palette is open.
struct PaletteOverlay: View {
    let model: ShellModel
    @ObservedObject var palette: PaletteState
    let maxHeight: CGFloat
    let width: CGFloat

    var body: some View {
        if palette.open, let result = palette.result {
            PaletteResults(model: model, palette: palette, result: result, maxHeight: maxHeight, width: width)
        }
    }
}

struct PaletteResults: View {
    let model: ShellModel
    @ObservedObject var palette: PaletteState
    let result: ShellPaletteResult
    let maxHeight: CGFloat
    var width: CGFloat = ShellTokens.paletteDropdownMaxWidth

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
        .clipShape(RoundedRectangle(cornerRadius: ShellTokens.Radius.toolWell))
        .overlay(RoundedRectangle(cornerRadius: ShellTokens.Radius.toolWell).strokeBorder(model.theme.rulesColor, lineWidth: 1))
        .shadow(color: .black.opacity(model.theme.isDark ? 0.5 : 0.14), radius: 14, y: 6)
        // Anchored under the field, which is centred in the toolbar row.
        .frame(width: width)
        .padding(.top, ShellTokens.Spacing.xs)
        .frame(maxWidth: .infinity, alignment: .top)
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
    let model: ShellModel
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
            } else if let shortcut = ShellModel.rowShortcut(item) {
                Text(shortcut)
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
