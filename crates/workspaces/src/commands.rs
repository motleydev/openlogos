//! The one command registry of specs/22 §1 and §3 (B070). Buttons, menus, shortcuts and the
//! palette dispatch the same [`Invocation`] through [`Shell::apply`], so the palette is a
//! superset of the interface by construction: a host may not offer an action that is not a
//! [`CommandSpec`] in [`registry`].
//!
//! Each spec carries its id, title, aliases, keywords, scope, argument shape, default
//! shortcuts per platform (specs/22 §6, plus the specs/21 §9 bindings §6 does not replace)
//! and whether it is undoable. [`enabled`] says whether a command can run now and, if not,
//! why, so a disabled command still appears with its reason.
//!
//! [`apply`] is pure: it takes a session and returns the new session, an [`UndoRecord`] for
//! every command that changes the layout or a tab's passage, and the [`Effects`] a host needs
//! (the tabs that followed a navigation). Commands whose work lives in the host (selection,
//! ink, drawers, text size, theme, export, focusing the palette) are declared here and come
//! back as `handled: false` for the host to perform. [`Shell`] keeps the undo and redo
//! stacks and a revision counter a host can key caches on.
//!
//! Shortcut strings name keys as `Cmd`, `Alt`, `Shift` and `Ctrl` joined by `+`; `Cmd` is ⌘
//! on Apple platforms and the host maps it to Ctrl in a browser on other systems. `{n}` stands
//! for the digits 1–9 of a command whose argument is a column number. A spec with
//! `bare_key` set binds a single key that works only while no text field has focus.
//!
//! Focus changes (switching workspace, column or tab) are not undoable: undo restores what
//! the reader changed, not where they looked. Navigation (go to passage, back, forward) and
//! every layout change are.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::ScrollAnchor;
use crate::navigation::{Visit, followers_among};
use crate::shell::{
    CLOSED_TABS_KEPT, ClosedTab, Column, INK_WIDTHS, LINE_SPACINGS, NotesDock, Session,
    TEXT_SIZE_STEPS, THEMES, Tab, Tool, Workspace, is_swatch,
};

/// What a command acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// The set of workspaces, or the whole window.
    Session,
    /// The active workspace.
    Workspace,
    /// The active column.
    Column,
    /// The active tab.
    Tab,
    /// The current text selection.
    Selection,
}

/// The argument a command takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArgumentShape {
    /// None.
    None,
    /// A column number, `column` in [`CommandArgs`].
    ColumnNumber,
    /// Free text, `text` (a title, a theme name); optional where the command has a default.
    Text,
    /// A passage, `reference`, plus an optional `column` or `new_column` target.
    Reference,
    /// A workspace id, `workspace_id`.
    Workspace,
    /// A tab id, `tab_id`, with an optional `workspace_id`.
    Tab,
    /// A link group label, `group`; defaults to `1`.
    LinkGroup,
    /// A number, `number` (a line spacing, a brush width).
    Number,
    /// A column number, `column`, and its new width share, `number`.
    ColumnWidth,
    /// A tab, `tab_id` (with an optional `workspace_id`), and where it is scrolled to,
    /// `segment_id` and `scalar`.
    Scroll,
}

/// The platforms whose default shortcuts the registry records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    /// A browser (apps/web).
    Web,
    /// The Mac app.
    Mac,
    /// The iPad app with a hardware keyboard.
    Ipad,
}

/// Default shortcuts per platform; the first of each list is the default, the rest are the
/// platform-conventional aliases specs/22 §6 allows native hosts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shortcuts {
    /// Web.
    pub web: Vec<String>,
    /// Mac.
    pub mac: Vec<String>,
    /// iPad.
    pub ipad: Vec<String>,
}

impl Shortcuts {
    /// The list for one platform.
    #[must_use]
    pub fn for_platform(&self, platform: Platform) -> &[String] {
        match platform {
            Platform::Web => &self.web,
            Platform::Mac => &self.mac,
            Platform::Ipad => &self.ipad,
        }
    }
}

/// One registered command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandSpec {
    /// Stable id, `area.verb`.
    pub id: String,
    /// What the palette and menus show.
    pub title: String,
    /// Other names the palette matches.
    pub aliases: Vec<String>,
    /// Further words the palette matches, ranked below title and aliases.
    pub keywords: Vec<String>,
    /// What it acts on.
    pub scope: Scope,
    /// The argument it takes.
    pub argument: ArgumentShape,
    /// Default shortcuts.
    pub shortcuts: Shortcuts,
    /// Whether the shortcut is a bare key that works only outside text fields.
    pub bare_key: bool,
    /// Whether [`apply`] returns an undo record for it.
    pub undoable: bool,
    /// Whether the core performs it; `false` means [`apply`] returns `handled: false` and
    /// the host performs it.
    pub handled_by_core: bool,
    /// Whether it records state without bumping [`Shell::revision`] (a scroll position:
    /// frequent, and nothing the palette indexes or a layout draws from).
    pub quiet: bool,
}

/// Arguments for an invocation; each command reads the fields its [`ArgumentShape`] names.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CommandArgs {
    /// A column number, 1-based.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    /// Open in a new column after the active one.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub new_column: bool,
    /// Free text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// A passage, as `parseReference` formats it (OSIS) or as the resource names it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// A workspace id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// A tab id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_id: Option<String>,
    /// A link group label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// A number: a line spacing, a brush width, a column's width share.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<f64>,
    /// A scroll anchor's segment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment_id: Option<String>,
    /// A scroll anchor's offset in scalar values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scalar: Option<u32>,
}

/// One command with its arguments: what every button, menu item, shortcut and palette row
/// dispatches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Invocation {
    /// A registered command id.
    pub command_id: String,
    /// Its arguments.
    #[serde(default)]
    pub args: CommandArgs,
}

impl Invocation {
    /// An invocation without arguments.
    #[must_use]
    pub fn new(command_id: &str) -> Invocation {
        Invocation {
            command_id: command_id.to_owned(),
            args: CommandArgs::default(),
        }
    }

    /// An invocation with arguments.
    #[must_use]
    pub fn with(command_id: &str, args: CommandArgs) -> Invocation {
        Invocation {
            command_id: command_id.to_owned(),
            args,
        }
    }
}

/// What the host knows that the session does not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CommandContext {
    /// Whether text is selected in some tab.
    pub has_selection: bool,
}

/// Why a command cannot run now.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DisabledReason {
    /// Needs a text selection.
    #[error("Select some text first")]
    NeedsSelection,
    /// The undo stack is empty.
    #[error("Nothing to undo")]
    NothingToUndo,
    /// The redo stack is empty.
    #[error("Nothing to redo")]
    NothingToRedo,
    /// The active tab has nowhere to go back to.
    #[error("No earlier passage in this tab")]
    NoBackHistory,
    /// The active tab has nowhere to go forward to.
    #[error("No later passage in this tab")]
    NoForwardHistory,
    /// Closing or switching needs another workspace.
    #[error("This is the only workspace")]
    OnlyWorkspace,
    /// Closing needs another column.
    #[error("This is the only column")]
    OnlyColumn,
    /// Closing the last tab of the last column would leave nothing to read.
    #[error("This is the last tab")]
    LastTab,
    /// The column holds one tab.
    #[error("This column has one tab")]
    OneTab,
    /// No tab was closed in this workspace.
    #[error("No closed tab to reopen")]
    NoClosedTab,
    /// The active column is already at that edge.
    #[error("Column {number} is already at the edge")]
    AtEdge {
        /// The active column's number.
        number: u32,
    },
    /// The column number does not exist.
    #[error("There is no column {number}")]
    NoSuchColumn {
        /// The number asked for.
        number: u32,
    },
    /// The tab is already in that column.
    #[error("The tab is already in column {number}")]
    AlreadyInColumn {
        /// The column.
        number: u32,
    },
    /// The tab is already in that mode.
    #[error("The tab is already {mode}")]
    AlreadyInMode {
        /// `pinned`, `following` or `in group n`.
        mode: String,
    },
    /// The workspace id does not exist.
    #[error("There is no workspace {workspace_id}")]
    NoSuchWorkspace {
        /// The id.
        workspace_id: String,
    },
    /// The tab id does not exist.
    #[error("There is no tab {tab_id}")]
    NoSuchTab {
        /// The id.
        tab_id: String,
    },
    /// A setting is already at the end of its range.
    #[error("The {setting} is already at its limit")]
    AtLimit {
        /// `text size`, `line spacing` or `brush width`.
        setting: String,
    },
    /// An argument outside what the command accepts.
    #[error("{detail}")]
    InvalidValue {
        /// What is accepted.
        detail: String,
    },
    /// A required argument is missing or empty.
    #[error("Needs {argument}")]
    NeedsArgument {
        /// The [`CommandArgs`] field.
        argument: String,
    },
}

/// Why an invocation failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommandError {
    /// Not in the registry.
    #[error("unknown command {command_id}")]
    UnknownCommand {
        /// The id given.
        command_id: String,
    },
    /// Registered but not possible now.
    #[error("{command_id} is disabled: {reason}")]
    Disabled {
        /// The command.
        command_id: String,
        /// Why.
        reason: DisabledReason,
    },
    /// Undo and redo act on a [`Shell`]'s stacks, not on a bare session.
    #[error("{command_id} needs the undo history of a Shell")]
    RequiresHistory {
        /// The command.
        command_id: String,
    },
}

/// What a session looked like before a command, for the parts the command could change:
/// the workspace order, the active workspace, the id serial and the workspaces it touched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Workspace ids in order.
    pub workspace_ids: Vec<String>,
    /// The active workspace.
    pub active_workspace_id: String,
    /// The session's id serial.
    pub next_serial: u64,
    /// Full copies of the workspaces the command touched.
    pub workspaces: Vec<Workspace>,
}

impl Snapshot {
    fn take(session: &Session, touched: &[String]) -> Snapshot {
        Snapshot {
            workspace_ids: session.workspaces.iter().map(|w| w.id.clone()).collect(),
            active_workspace_id: session.active_workspace_id.clone(),
            next_serial: session.next_serial,
            workspaces: session
                .workspaces
                .iter()
                .filter(|w| touched.contains(&w.id))
                .cloned()
                .collect(),
        }
    }

    fn restore(&self, session: &mut Session) {
        for saved in &self.workspaces {
            match session.workspaces.iter_mut().find(|w| w.id == saved.id) {
                Some(w) => *w = saved.clone(),
                None => session.workspaces.push(saved.clone()),
            }
        }
        session
            .workspaces
            .retain(|w| self.workspace_ids.contains(&w.id));
        session.workspaces.sort_by_key(|w| {
            self.workspace_ids
                .iter()
                .position(|id| *id == w.id)
                .unwrap_or(usize::MAX)
        });
        session.active_workspace_id = self.active_workspace_id.clone();
        session.next_serial = self.next_serial;
    }
}

/// How to take one command back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UndoRecord {
    /// The command it undoes.
    pub command_id: String,
    /// Its title, for "Undo New column".
    pub title: String,
    /// The session before the command, as far as the command could change it.
    pub before: Snapshot,
}

/// What a host does after a command besides re-rendering.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effects {
    /// Tabs moved by a navigation's link group or follow mode, to remap through the host's
    /// reference mapping when their scheme differs from the driver's.
    pub followed_tab_ids: Vec<String>,
}

/// The result of [`apply`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Applied {
    /// The session after the command (unchanged when `handled` is false).
    pub session: Session,
    /// How to undo it, for an undoable command.
    pub undo: Option<UndoRecord>,
    /// What else the host does.
    pub effects: Effects,
    /// `false` when the host performs the command.
    pub handled: bool,
}

struct Def {
    id: &'static str,
    title: &'static str,
    aliases: &'static [&'static str],
    keywords: &'static [&'static str],
    scope: Scope,
    argument: ArgumentShape,
    web: &'static [&'static str],
    mac: &'static [&'static str],
    bare_key: bool,
    undoable: bool,
    core: bool,
    quiet: bool,
}

const fn def(
    id: &'static str,
    title: &'static str,
    scope: Scope,
    argument: ArgumentShape,
    undoable: bool,
    core: bool,
) -> Def {
    Def {
        id,
        title,
        aliases: &[],
        keywords: &[],
        scope,
        argument,
        web: &[],
        mac: &[],
        bare_key: false,
        undoable,
        core,
        quiet: false,
    }
}

impl Def {
    const fn names(
        mut self,
        aliases: &'static [&'static str],
        keywords: &'static [&'static str],
    ) -> Def {
        self.aliases = aliases;
        self.keywords = keywords;
        self
    }

    /// Web and Mac bindings; iPad takes the Mac list (hardware keyboard).
    const fn keys(mut self, web: &'static [&'static str], mac: &'static [&'static str]) -> Def {
        self.web = web;
        self.mac = mac;
        self
    }

    const fn quiet(mut self) -> Def {
        self.quiet = true;
        self
    }

    const fn bare(mut self, key: &'static [&'static str]) -> Def {
        self.web = key;
        self.mac = key;
        self.bare_key = true;
        self
    }
}

use ArgumentShape as A;
use Scope as S;

/// The registry in palette order: specs/22 §3's first cut, the tools of §4 and the
/// bindings of §6.
const DEFS: &[Def] = &[
    def(
        "palette.focus",
        "Focus palette",
        S::Session,
        A::None,
        false,
        false,
    )
    .names(
        &["search", "command palette"],
        &["find", "go to", "launcher"],
    )
    .keys(&["Cmd+K", "/"], &["Cmd+K", "/"]),
    def(
        "nav.navigate",
        "Go to passage",
        S::Tab,
        A::Reference,
        true,
        true,
    )
    .names(
        &["open passage", "navigate"],
        &["reference", "verse", "chapter", "open"],
    ),
    def("nav.back", "Back", S::Tab, A::None, true, true)
        .names(&["go back"], &["history", "previous passage"])
        .keys(&["Cmd+["], &["Cmd+["]),
    def("nav.forward", "Forward", S::Tab, A::None, true, true)
        .names(&["go forward"], &["history", "next passage"])
        .keys(&["Cmd+]"], &["Cmd+]"]),
    def(
        "workspace.new",
        "New workspace",
        S::Session,
        A::Text,
        true,
        true,
    )
    .names(&["create workspace"], &["add", "window"])
    .keys(&["Alt+Shift+N"], &["Alt+Shift+N", "Cmd+Shift+N"]),
    def(
        "workspace.close",
        "Close workspace",
        S::Session,
        A::None,
        true,
        true,
    )
    .names(&[], &["remove", "delete"]),
    def(
        "workspace.rename",
        "Rename workspace",
        S::Workspace,
        A::Text,
        true,
        true,
    )
    .names(&[], &["title", "name"]),
    def(
        "workspace.duplicate",
        "Duplicate workspace",
        S::Session,
        A::None,
        true,
        true,
    )
    .names(&["copy workspace"], &["clone"]),
    def(
        "workspace.switch",
        "Switch to workspace",
        S::Session,
        A::Workspace,
        false,
        true,
    )
    .names(&["go to workspace", "open workspace"], &["change"]),
    def(
        "workspace.next",
        "Next workspace",
        S::Session,
        A::None,
        false,
        true,
    )
    .names(&[], &["switch"])
    .keys(&["Alt+Shift+]"], &["Alt+Shift+]"]),
    def(
        "workspace.previous",
        "Previous workspace",
        S::Session,
        A::None,
        false,
        true,
    )
    .names(&[], &["switch"])
    .keys(&["Alt+Shift+["], &["Alt+Shift+["]),
    def(
        "column.new",
        "New column",
        S::Workspace,
        A::None,
        true,
        true,
    )
    .names(&["add column", "split"], &["pane"])
    .keys(&["Alt+N"], &["Alt+N"]),
    def(
        "column.close",
        "Close column",
        S::Column,
        A::None,
        true,
        true,
    )
    .names(&["remove column"], &["pane"]),
    def(
        "column.move_left",
        "Move column left",
        S::Column,
        A::None,
        true,
        true,
    )
    .names(&[], &["reorder", "pane"]),
    def(
        "column.move_right",
        "Move column right",
        S::Column,
        A::None,
        true,
        true,
    )
    .names(&[], &["reorder", "pane"]),
    def(
        "column.resize",
        "Resize column",
        S::Workspace,
        A::ColumnWidth,
        true,
        true,
    )
    .names(&["column width"], &["width", "wider", "narrower", "pane"]),
    def(
        "column.equalize",
        "Equalize column widths",
        S::Workspace,
        A::None,
        true,
        true,
    )
    .names(
        &["equal columns", "even columns"],
        &["width", "balance", "reset"],
    ),
    def(
        "column.focus",
        "Focus column",
        S::Workspace,
        A::ColumnNumber,
        false,
        true,
    )
    .names(&["go to column", "activate column"], &["pane", "switch"])
    .keys(&["Alt+{n}"], &["Alt+{n}", "Cmd+{n}"]),
    def("tab.new", "New tab", S::Column, A::None, true, true)
        .names(&["open tab"], &["add"])
        .keys(&["Alt+T"], &["Alt+T", "Cmd+T"]),
    def("tab.close", "Close tab", S::Tab, A::None, true, true)
        .names(&[], &["remove"])
        .keys(&["Alt+W"], &["Alt+W", "Cmd+W"]),
    def(
        "tab.reopen_closed",
        "Reopen closed tab",
        S::Workspace,
        A::None,
        true,
        true,
    )
    .names(&["restore tab", "undo close tab"], &["recover"]),
    def("tab.next", "Next tab", S::Column, A::None, false, true)
        .names(&[], &["switch"])
        .keys(&["Alt+]"], &["Alt+]", "Ctrl+Tab"]),
    def(
        "tab.previous",
        "Previous tab",
        S::Column,
        A::None,
        false,
        true,
    )
    .names(&[], &["switch"])
    .keys(&["Alt+["], &["Alt+[", "Ctrl+Shift+Tab"]),
    def(
        "tab.move_to_column",
        "Move tab to column",
        S::Tab,
        A::ColumnNumber,
        true,
        true,
    )
    .names(&["move tab"], &["send"])
    .keys(&["Alt+Shift+{n}"], &["Alt+Shift+{n}"]),
    def(
        "tab.duplicate_to_column",
        "Duplicate tab into column",
        S::Tab,
        A::ColumnNumber,
        true,
        true,
    )
    .names(&["duplicate tab", "copy tab"], &["clone"]),
    def(
        "tab.record_scroll",
        "Record scroll position",
        S::Tab,
        A::Scroll,
        false,
        true,
    )
    .names(&[], &["scroll", "anchor"])
    .quiet(),
    def("tab.focus", "Go to tab", S::Session, A::Tab, false, true).names(&["switch to tab"], &[]),
    def("tab.link", "Link tab", S::Tab, A::LinkGroup, true, true)
        .names(&["link", "join group"], &["group", "sync"])
        .keys(&["Cmd+Shift+L"], &["Cmd+Shift+L"]),
    def("tab.pin", "Pin tab", S::Tab, A::None, true, true)
        .names(&["pin", "unlink"], &["fix", "independent"])
        .keys(&["Cmd+Shift+P"], &["Cmd+Shift+P"]),
    def(
        "tab.follow",
        "Follow active passage",
        S::Tab,
        A::None,
        true,
        true,
    )
    .names(&["follow"], &["track"]),
    def(
        "view.notes_dock",
        "Toggle notes dock",
        S::Workspace,
        A::None,
        true,
        true,
    )
    .names(&["notes", "open notes dock"], &["dock"]),
    def(
        "view.collection_drawer",
        "Toggle collection drawer",
        S::Workspace,
        A::None,
        false,
        false,
    )
    .names(&["collection"], &["drawer"])
    .keys(&["Cmd+Shift+O"], &["Cmd+Shift+O"]),
    def(
        "view.study_drawer",
        "Toggle study drawer",
        S::Workspace,
        A::None,
        false,
        false,
    )
    .names(&["study"], &["drawer"]),
    def(
        "tool.select",
        "Select tool",
        S::Session,
        A::None,
        false,
        true,
    )
    .names(&["select"], &["tool", "cursor"])
    .bare(&["V"]),
    def(
        "tool.highlight",
        "Highlight tool",
        S::Session,
        A::None,
        false,
        true,
    )
    .names(&["highlight"], &["tool", "marker"])
    .bare(&["H"]),
    def("tool.pen", "Pen tool", S::Session, A::None, false, true)
        .names(&["pen"], &["tool", "ink", "draw"])
        .bare(&["P"]),
    def(
        "tool.highlighter_pen",
        "Highlighter pen tool",
        S::Session,
        A::None,
        false,
        true,
    )
    .names(&["highlighter pen", "highlighter"], &["tool", "ink"])
    .bare(&["M"]),
    def(
        "tool.eraser",
        "Eraser tool",
        S::Session,
        A::None,
        false,
        true,
    )
    .names(&["eraser"], &["tool", "ink", "erase"])
    .bare(&["E"]),
    def(
        "tool.collect",
        "Collect tool",
        S::Session,
        A::None,
        false,
        true,
    )
    .names(&["collect"], &["tool", "collection"])
    .bare(&["C"]),
    def(
        "tool.swatch",
        "Choose swatch",
        S::Session,
        A::Text,
        false,
        true,
    )
    .names(&["colour", "color", "swatch"], &["ink", "well", "tool"]),
    def(
        "tool.width",
        "Set brush width",
        S::Session,
        A::Number,
        false,
        true,
    )
    .names(&["brush width", "pen width"], &["ink", "size", "tool"]),
    def(
        "tool.width_up",
        "Thicker brush",
        S::Session,
        A::None,
        false,
        true,
    )
    .names(
        &["increase brush width", "wider brush"],
        &["ink", "size", "tool"],
    ),
    def(
        "tool.width_down",
        "Thinner brush",
        S::Session,
        A::None,
        false,
        true,
    )
    .names(
        &["decrease brush width", "narrower brush"],
        &["ink", "size", "tool"],
    ),
    def(
        "tool.swap_swatches",
        "Swap swatches",
        S::Session,
        A::None,
        false,
        true,
    )
    .names(&[], &["colour", "color", "tool"])
    .bare(&["X"]),
    def(
        "view.tool_palette",
        "Toggle tool palette",
        S::Session,
        A::None,
        false,
        false,
    )
    .names(&["tools", "hide tools", "show tools"], &["palette"]),
    def(
        "selection.highlight",
        "Highlight selection",
        S::Selection,
        A::None,
        false,
        false,
    )
    .names(&[], &["mark"])
    .keys(&["Cmd+Shift+H"], &["Cmd+Shift+H"]),
    def(
        "selection.collect",
        "Collect selection",
        S::Selection,
        A::None,
        false,
        false,
    )
    .names(&[], &["collection", "save"])
    .keys(&["Cmd+Shift+C"], &["Cmd+Shift+C"]),
    def(
        "selection.study",
        "Study selection",
        S::Selection,
        A::None,
        false,
        false,
    )
    .names(&[], &["lexicon", "word study"])
    .keys(&["Cmd+Shift+S"], &["Cmd+Shift+S"]),
    def(
        "view.text_size_up",
        "Increase text size",
        S::Session,
        A::None,
        false,
        true,
    )
    .names(
        &["text size up", "bigger text"],
        &["zoom", "font", "larger"],
    ),
    def(
        "view.text_size_down",
        "Decrease text size",
        S::Session,
        A::None,
        false,
        true,
    )
    .names(&["text size down", "smaller text"], &["zoom", "font"]),
    def(
        "view.text_size_reset",
        "Reset text size",
        S::Session,
        A::None,
        false,
        true,
    )
    .names(&["actual size"], &["zoom", "font"]),
    def(
        "view.line_spacing",
        "Set line spacing",
        S::Session,
        A::Number,
        false,
        true,
    )
    .names(&["line spacing", "line height"], &["leading", "spacing"]),
    def(
        "view.line_spacing_up",
        "Increase line spacing",
        S::Session,
        A::None,
        false,
        true,
    )
    .names(
        &["more line spacing", "looser lines"],
        &["leading", "spacing"],
    ),
    def(
        "view.line_spacing_down",
        "Decrease line spacing",
        S::Session,
        A::None,
        false,
        true,
    )
    .names(
        &["less line spacing", "tighter lines"],
        &["leading", "spacing"],
    ),
    def(
        "view.theme",
        "Change theme",
        S::Session,
        A::Text,
        false,
        true,
    )
    .names(&["theme"], &["dark", "light", "sepia", "appearance"]),
    def(
        "view.focus_mode",
        "Focus mode",
        S::Session,
        A::None,
        false,
        false,
    )
    .names(&["zen mode"], &["distraction"])
    .keys(&["Cmd+Shift+F"], &["Cmd+Shift+F"]),
    def(
        "workspace.export",
        "Export workspace",
        S::Workspace,
        A::None,
        false,
        false,
    )
    .names(&["share workspace"], &["save", "file"]),
    def("edit.undo", "Undo", S::Session, A::None, false, true)
        .names(&[], &["revert"])
        .keys(&["Cmd+Z"], &["Cmd+Z"]),
    def("edit.redo", "Redo", S::Session, A::None, false, true)
        .names(&[], &[])
        .keys(&["Cmd+Shift+Z"], &["Cmd+Shift+Z"]),
];

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

/// Every registered command, in palette order.
#[must_use]
pub fn registry() -> &'static [CommandSpec] {
    static REGISTRY: OnceLock<Vec<CommandSpec>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        DEFS.iter()
            .map(|d| CommandSpec {
                id: d.id.to_owned(),
                title: d.title.to_owned(),
                aliases: strings(d.aliases),
                keywords: strings(d.keywords),
                scope: d.scope,
                argument: d.argument,
                shortcuts: Shortcuts {
                    web: strings(d.web),
                    mac: strings(d.mac),
                    ipad: strings(d.mac),
                },
                bare_key: d.bare_key,
                undoable: d.undoable,
                handled_by_core: d.core,
                quiet: d.quiet,
            })
            .collect()
    })
}

/// The registered command with this id.
#[must_use]
pub fn command(id: &str) -> Option<&'static CommandSpec> {
    registry().iter().find(|c| c.id == id)
}

fn tool_of(id: &str) -> Option<Tool> {
    Some(match id {
        "tool.select" => Tool::Select,
        "tool.highlight" => Tool::Highlight,
        "tool.pen" => Tool::Pen,
        "tool.highlighter_pen" => Tool::HighlighterPen,
        "tool.eraser" => Tool::Eraser,
        "tool.collect" => Tool::Collect,
        _ => return None,
    })
}

fn need(argument: &str) -> DisabledReason {
    DisabledReason::NeedsArgument {
        argument: argument.to_owned(),
    }
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// The workspace an invocation acts on: `args.workspace_id` or the active one.
fn target_workspace<'a>(
    session: &'a Session,
    args: &CommandArgs,
) -> Result<&'a Workspace, DisabledReason> {
    match &args.workspace_id {
        Some(id) => session
            .workspace(id)
            .ok_or_else(|| DisabledReason::NoSuchWorkspace {
                workspace_id: id.clone(),
            }),
        None => Ok(session.active()),
    }
}

fn column_number(workspace: &Workspace, args: &CommandArgs) -> Result<usize, DisabledReason> {
    let number = args.column.ok_or_else(|| need("column"))?;
    if number == 0 || number as usize > workspace.columns.len() {
        return Err(DisabledReason::NoSuchColumn { number });
    }
    Ok(number as usize)
}

/// Whether `invocation` can run against `session` now, given whether the caller's undo and
/// redo stacks are empty. `Ok` for a host-performed command whose preconditions hold.
pub fn enabled(
    session: &Session,
    can_undo: bool,
    can_redo: bool,
    invocation: &Invocation,
    context: &CommandContext,
) -> Result<(), DisabledReason> {
    let args = &invocation.args;
    let ws = session.active();
    let column = ws.active_column();
    let tab = ws.active_tab();
    match invocation.command_id.as_str() {
        "edit.undo" if !can_undo => Err(DisabledReason::NothingToUndo),
        "edit.redo" if !can_redo => Err(DisabledReason::NothingToRedo),
        "selection.highlight" | "selection.collect" | "selection.study"
            if !context.has_selection =>
        {
            Err(DisabledReason::NeedsSelection)
        }
        "nav.navigate" => {
            non_empty(&args.reference).ok_or_else(|| need("reference"))?;
            if args.column.is_some() && !args.new_column {
                column_number(ws, args)?;
            }
            Ok(())
        }
        "nav.back" if tab.history.back.is_empty() => Err(DisabledReason::NoBackHistory),
        "nav.forward" if tab.history.forward.is_empty() => Err(DisabledReason::NoForwardHistory),
        "workspace.close" | "workspace.next" | "workspace.previous"
            if session.workspaces.len() < 2 =>
        {
            Err(DisabledReason::OnlyWorkspace)
        }
        "workspace.close" => target_workspace(session, args).map(|_| ()),
        "workspace.rename" => {
            target_workspace(session, args)?;
            non_empty(&args.text)
                .map(|_| ())
                .ok_or_else(|| need("text"))
        }
        "workspace.switch" => {
            args.workspace_id
                .as_ref()
                .ok_or_else(|| need("workspace_id"))?;
            target_workspace(session, args).map(|_| ())
        }
        "column.close" if ws.columns.len() < 2 => Err(DisabledReason::OnlyColumn),
        "column.move_left" if ws.active_index() == 0 => Err(DisabledReason::AtEdge { number: 1 }),
        "column.move_right" if ws.active_index() + 1 == ws.columns.len() => {
            Err(DisabledReason::AtEdge {
                number: ws.columns.len() as u32,
            })
        }
        "column.focus" | "tab.duplicate_to_column" => column_number(ws, args).map(|_| ()),
        "tab.move_to_column" => {
            let number = column_number(ws, args)?;
            if number == ws.active_index() + 1 {
                return Err(DisabledReason::AlreadyInColumn {
                    number: number as u32,
                });
            }
            Ok(())
        }
        "tab.close" if ws.columns.len() == 1 && column.tabs.len() == 1 => {
            Err(DisabledReason::LastTab)
        }
        "tab.reopen_closed" if ws.closed_tabs.is_empty() => Err(DisabledReason::NoClosedTab),
        "tab.next" | "tab.previous" if column.tabs.len() < 2 => Err(DisabledReason::OneTab),
        "tab.focus" => {
            let tab_id = args.tab_id.as_ref().ok_or_else(|| need("tab_id"))?;
            let target = target_workspace(session, args)?;
            target
                .find_tab(tab_id)
                .map(|_| ())
                .ok_or_else(|| DisabledReason::NoSuchTab {
                    tab_id: tab_id.clone(),
                })
        }
        "tab.link" => {
            let group = non_empty(&args.group).unwrap_or("1");
            if tab.link_group.as_deref() == Some(group) && !tab.pinned {
                return Err(DisabledReason::AlreadyInMode {
                    mode: format!("in group {group}"),
                });
            }
            Ok(())
        }
        "view.text_size_up" if session.reading.text_size_step >= *TEXT_SIZE_STEPS.end() => {
            Err(limit("text size"))
        }
        "view.text_size_down" if session.reading.text_size_step <= *TEXT_SIZE_STEPS.start() => {
            Err(limit("text size"))
        }
        "view.line_spacing" => one_of(args.number, &LINE_SPACINGS, "line spacing").map(|_| ()),
        "view.line_spacing_up" | "view.line_spacing_down" => step(
            &LINE_SPACINGS,
            session.reading.line_spacing,
            id_up(invocation),
        )
        .map(|_| ())
        .ok_or_else(|| limit("line spacing")),
        "view.theme" => {
            let theme = non_empty(&args.text).ok_or_else(|| need("text"))?;
            if THEMES.contains(&theme) {
                Ok(())
            } else {
                Err(DisabledReason::InvalidValue {
                    detail: format!("A theme is one of {}", THEMES.join(", ")),
                })
            }
        }
        "tool.width" => one_of(args.number, &INK_WIDTHS, "brush width").map(|_| ()),
        "tool.width_up" | "tool.width_down" => {
            step(&INK_WIDTHS, session.ink.width, id_up(invocation))
                .map(|_| ())
                .ok_or_else(|| limit("brush width"))
        }
        "tool.swatch" => {
            let swatch = non_empty(&args.text).ok_or_else(|| need("text"))?;
            if is_swatch(swatch) {
                Ok(())
            } else {
                Err(DisabledReason::InvalidValue {
                    detail: "A swatch is a colour as #RRGGBB".to_owned(),
                })
            }
        }
        "column.resize" | "column.equalize" if ws.columns.len() < 2 => {
            Err(DisabledReason::OnlyColumn)
        }
        "column.resize" => {
            column_number(ws, args)?;
            let share = args.number.ok_or_else(|| need("number"))?;
            let max = 1.0 - MIN_COLUMN_FRACTION * (ws.columns.len() - 1) as f64;
            if share.is_finite() && (MIN_COLUMN_FRACTION..=max).contains(&share) {
                Ok(())
            } else {
                Err(DisabledReason::InvalidValue {
                    detail: format!(
                        "A column's share is between {MIN_COLUMN_FRACTION} and {max:.2}"
                    ),
                })
            }
        }
        "tab.record_scroll" => {
            let tab_id = args.tab_id.as_ref().ok_or_else(|| need("tab_id"))?;
            non_empty(&args.segment_id).ok_or_else(|| need("segment_id"))?;
            args.scalar.ok_or_else(|| need("scalar"))?;
            target_workspace(session, args)?
                .find_tab(tab_id)
                .map(|_| ())
                .ok_or_else(|| DisabledReason::NoSuchTab {
                    tab_id: tab_id.clone(),
                })
        }
        "tab.pin" if tab.pinned => Err(DisabledReason::AlreadyInMode {
            mode: "pinned".to_owned(),
        }),
        "tab.follow" if !tab.pinned && tab.link_group.is_none() => {
            Err(DisabledReason::AlreadyInMode {
                mode: "following".to_owned(),
            })
        }
        _ => Ok(()),
    }
}

/// The smallest share a column can be resized to.
pub const MIN_COLUMN_FRACTION: f64 = 0.05;

fn limit(setting: &str) -> DisabledReason {
    DisabledReason::AtLimit {
        setting: setting.to_owned(),
    }
}

/// Whether a command id steps a setting up (`…_up`) rather than down.
fn id_up(invocation: &Invocation) -> bool {
    invocation.command_id.ends_with("_up")
}

/// The value of `values` that `number` names, within a rounding tolerance.
fn one_of(number: Option<f64>, values: &[f64], setting: &str) -> Result<f64, DisabledReason> {
    let number = number.ok_or_else(|| need("number"))?;
    values
        .iter()
        .copied()
        .find(|v| (v - number).abs() < 1e-6)
        .ok_or_else(|| DisabledReason::InvalidValue {
            detail: format!(
                "A {setting} is one of {}",
                values
                    .iter()
                    .map(f64::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        })
}

/// The next value of `values` above or below `current`; `None` at the end.
fn step(values: &[f64], current: f64, up: bool) -> Option<f64> {
    if up {
        values.iter().copied().find(|v| *v > current + 1e-9)
    } else {
        values.iter().rev().copied().find(|v| *v < current - 1e-9)
    }
}

/// A copy of `source` under a new id, with an empty history.
fn copy_tab(source: &Tab, id: String) -> Tab {
    Tab {
        id,
        history: Default::default(),
        ..source.clone()
    }
}

/// Inserts a column at `index` holding `tab`, giving it an equal share and scaling the
/// others, and makes it active.
fn insert_column(ws: &mut Workspace, index: usize, tab: Tab) {
    let id = ws.mint_id("c");
    let n = ws.columns.len() as f64 + 1.0;
    for column in &mut ws.columns {
        column.width_fraction *= (n - 1.0) / n;
    }
    ws.columns.insert(
        index,
        Column {
            id: id.clone(),
            shown_tab_id: tab.id.clone(),
            tabs: vec![tab],
            width_fraction: 1.0 / n,
        },
    );
    ws.active_column_id = id;
}

/// Removes column `index`, rescaling the others and moving activity to its neighbour.
fn remove_column(ws: &mut Workspace, index: usize) -> Column {
    let removed = ws.columns.remove(index);
    ws.layout_profiles.forget_column(&removed.id);
    ws.normalize_widths();
    if ws.active_column_id == removed.id {
        let next = index.min(ws.columns.len() - 1);
        ws.active_column_id = ws.columns[next].id.clone();
    }
    removed
}

/// Removes a tab from column `index`, showing its neighbour; removes the column if it
/// empties. Returns the tab and its position.
fn take_tab(ws: &mut Workspace, index: usize, tab_id: &str) -> (Tab, usize) {
    let column = &mut ws.columns[index];
    let position = column
        .tabs
        .iter()
        .position(|t| t.id == tab_id)
        .expect("caller found the tab");
    let tab = column.tabs.remove(position);
    if column.tabs.is_empty() {
        remove_column(ws, index);
    } else if column.shown_tab_id == tab.id {
        let next = position.min(column.tabs.len() - 1);
        column.shown_tab_id = column.tabs[next].id.clone();
    }
    (tab, position)
}

fn remember_closed(ws: &mut Workspace, closed: ClosedTab) {
    ws.closed_tabs.push(closed);
    if ws.closed_tabs.len() > CLOSED_TABS_KEPT {
        ws.closed_tabs.remove(0);
    }
}

fn tab_mut<'a>(ws: &'a mut Workspace, tab_id: &str) -> &'a mut Tab {
    ws.columns
        .iter_mut()
        .flat_map(|c| c.tabs.iter_mut())
        .find(|t| t.id == tab_id)
        .expect("caller found the tab")
}

fn current_visit(tab: &Tab, transaction_id: &str) -> Visit {
    Visit {
        reference: tab.reference.clone(),
        scroll_anchor: tab.scroll_anchor.clone(),
        transaction_id: transaction_id.to_owned(),
        driven: false,
    }
}

/// Moves a tab to `reference` and records the visit in its history, seeding the history
/// with where the tab was if it has never navigated.
fn move_tab(tab: &mut Tab, reference: &str, transaction_id: &str, driven: bool) {
    if tab.history.back.len() >= HISTORY_DEPTH {
        tab.history.back.remove(0);
    }
    if tab.history.current.is_none() {
        tab.history.current = Some(current_visit(tab, "restored"));
    }
    tab.reference = Some(reference.to_owned());
    tab.scroll_anchor = None;
    tab.history.visit(Visit {
        reference: tab.reference.clone(),
        scroll_anchor: None,
        transaction_id: transaction_id.to_owned(),
        driven,
    });
}

/// A deliberate navigation in `driver`: the driver moves, then its link group's other
/// members and every following tab of the workspace, shown or not (specs/22 §2). Followers
/// land on the same reference; a host whose follower uses another scheme remaps it.
fn navigate(ws: &mut Workspace, driver: &str, reference: &str) -> Vec<String> {
    let transaction = ws.mint_id("nav");
    let members: Vec<(String, Option<String>, bool)> = ws
        .tabs()
        .map(|t| (t.id.clone(), t.link_group.clone(), t.pinned))
        .collect();
    let mut followed = followers_among(
        members
            .iter()
            .map(|(id, group, pinned)| (id.as_str(), group.as_deref(), *pinned)),
        driver,
    );
    let driver_follows = members
        .iter()
        .any(|(id, group, pinned)| id == driver && group.is_none() && !pinned);
    if !driver_follows {
        for (id, group, pinned) in &members {
            if id != driver && group.is_none() && !pinned {
                followed.push(id.clone());
            }
        }
    }
    move_tab(tab_mut(ws, driver), reference, &transaction, true);
    for id in &followed {
        move_tab(tab_mut(ws, id), reference, &transaction, false);
    }
    followed
}

/// Runs one invocation against a session: the new session, an undo record for an undoable
/// command, and the effects. Fails for an unknown command, a disabled one (with the
/// reason) and for undo and redo, which need a [`Shell`].
pub fn apply(
    session: &Session,
    invocation: &Invocation,
    context: &CommandContext,
) -> Result<Applied, CommandError> {
    let mut s = session.clone();
    let (undo, effects, handled) = apply_in_place(&mut s, invocation, context)?;
    Ok(Applied {
        session: s,
        undo,
        effects,
        handled,
    })
}

/// [`apply`] without the copy: mutates `s` and copies only the workspaces an undoable
/// command touches into its record, so a focus change costs no copy at all. On an error
/// `s` is unchanged: every check runs before the first mutation.
fn apply_in_place(
    s: &mut Session,
    invocation: &Invocation,
    context: &CommandContext,
) -> Result<(Option<UndoRecord>, Effects, bool), CommandError> {
    let id = invocation.command_id.as_str();
    let spec = command(id).ok_or_else(|| CommandError::UnknownCommand {
        command_id: id.to_owned(),
    })?;
    if matches!(id, "edit.undo" | "edit.redo") {
        return Err(CommandError::RequiresHistory {
            command_id: id.to_owned(),
        });
    }
    enabled(s, false, false, invocation, context).map_err(|reason| CommandError::Disabled {
        command_id: id.to_owned(),
        reason,
    })?;
    if !spec.handled_by_core {
        return Ok((None, Effects::default(), false));
    }

    let args = &invocation.args;
    let mut touched = vec![s.active_workspace_id.clone()];
    if let Some(w) = &args.workspace_id
        && !touched.contains(w)
    {
        touched.push(w.clone());
    }
    let before = spec.undoable.then(|| Snapshot::take(s, &touched));
    let mut effects = Effects::default();

    match id {
        "nav.navigate" => {
            let reference = non_empty(&args.reference).expect("enabled").to_owned();
            let ws = s.active_mut();
            let driver = if args.new_column {
                let source = ws.active_tab().clone();
                let tab_id = ws.mint_id("t");
                let mut tab = copy_tab(&source, tab_id.clone());
                // A tab opened beside the others must not drag their group along.
                tab.link_group = None;
                tab.pinned = true;
                let index = ws.active_index() + 1;
                insert_column(ws, index, tab);
                tab_id
            } else {
                if let Some(number) = args.column {
                    ws.active_column_id = ws.columns[number as usize - 1].id.clone();
                }
                ws.active_tab().id.clone()
            };
            effects.followed_tab_ids = navigate(ws, &driver, &reference);
        }
        "nav.back" | "nav.forward" => {
            let ws = s.active_mut();
            let tab_id = ws.active_tab().id.clone();
            let tab = tab_mut(ws, &tab_id);
            let visit = if id == "nav.back" {
                tab.history.go_back()
            } else {
                tab.history.go_forward()
            }
            .cloned()
            .expect("enabled");
            tab.reference = visit.reference;
            tab.scroll_anchor = visit.scroll_anchor;
        }
        "workspace.new" => {
            let source = s.active().active_tab().clone();
            let id = s.mint_workspace_id();
            let title = non_empty(&args.text).map_or_else(
                || format!("Workspace {}", s.workspaces.len() + 1),
                str::to_owned,
            );
            let mut ws = Workspace {
                schema_version: crate::shell::WORKSPACE_SCHEMA_VERSION.to_owned(),
                fixture_only: s.fixture_only,
                id: id.clone(),
                title,
                columns: Vec::new(),
                active_column_id: String::new(),
                notes_dock: NotesDock::default(),
                layout_profiles: Default::default(),
                next_serial: 1,
                closed_tabs: Vec::new(),
            };
            let tab_id = ws.mint_id("t");
            insert_column(&mut ws, 0, copy_tab(&source, tab_id));
            let index = s.active_index() + 1;
            s.workspaces.insert(index, ws);
            s.active_workspace_id = id;
        }
        "workspace.close" => {
            let target = target_workspace(s, args).expect("enabled").id.clone();
            let index = s
                .workspaces
                .iter()
                .position(|w| w.id == target)
                .expect("enabled");
            s.workspaces.remove(index);
            if s.active_workspace_id == target {
                let next = index.min(s.workspaces.len() - 1);
                s.active_workspace_id = s.workspaces[next].id.clone();
            }
        }
        "workspace.rename" => {
            let title = non_empty(&args.text).expect("enabled").to_owned();
            let target = target_workspace(s, args).expect("enabled").id.clone();
            if let Some(ws) = s.workspaces.iter_mut().find(|w| w.id == target) {
                ws.title = title;
            }
        }
        "workspace.duplicate" => {
            let mut copy = s.active().clone();
            copy.id = s.mint_workspace_id();
            copy.title = format!("{} copy", copy.title);
            copy.closed_tabs.clear();
            let index = s.active_index() + 1;
            s.active_workspace_id = copy.id.clone();
            s.workspaces.insert(index, copy);
        }
        "workspace.switch" => {
            s.active_workspace_id = args.workspace_id.clone().expect("enabled");
        }
        "workspace.next" | "workspace.previous" => {
            let n = s.workspaces.len();
            let index = s.active_index();
            let next = if id == "workspace.next" {
                (index + 1) % n
            } else {
                (index + n - 1) % n
            };
            s.active_workspace_id = s.workspaces[next].id.clone();
        }
        "column.new" => {
            let ws = s.active_mut();
            let source = ws.active_tab().clone();
            let tab_id = ws.mint_id("t");
            let index = ws.active_index() + 1;
            insert_column(ws, index, copy_tab(&source, tab_id));
        }
        "column.close" => {
            let ws = s.active_mut();
            let index = ws.active_index();
            let column = remove_column(ws, index);
            for (position, tab) in column.tabs.into_iter().enumerate() {
                remember_closed(
                    ws,
                    ClosedTab {
                        tab,
                        column_id: column.id.clone(),
                        index: position,
                    },
                );
            }
        }
        "column.move_left" | "column.move_right" => {
            let ws = s.active_mut();
            let index = ws.active_index();
            let other = if id == "column.move_left" {
                index - 1
            } else {
                index + 1
            };
            ws.columns.swap(index, other);
        }
        "column.focus" => {
            let ws = s.active_mut();
            let number = column_number(ws, args).expect("enabled");
            ws.active_column_id = ws.columns[number - 1].id.clone();
        }
        "tab.new" => {
            let ws = s.active_mut();
            let source = ws.active_tab().clone();
            let tab_id = ws.mint_id("t");
            let index = ws.active_index();
            let column = &mut ws.columns[index];
            let position = column.shown_index() + 1;
            column
                .tabs
                .insert(position, copy_tab(&source, tab_id.clone()));
            column.shown_tab_id = tab_id;
        }
        "tab.close" => {
            let ws = s.active_mut();
            let index = ws.active_index();
            let column_id = ws.columns[index].id.clone();
            let tab_id = ws.active_tab().id.clone();
            let (tab, position) = take_tab(ws, index, &tab_id);
            remember_closed(
                ws,
                ClosedTab {
                    tab,
                    column_id,
                    index: position,
                },
            );
        }
        "tab.reopen_closed" => {
            let ws = s.active_mut();
            let closed = ws.closed_tabs.pop().expect("enabled");
            let tab_id = closed.tab.id.clone();
            match ws.columns.iter().position(|c| c.id == closed.column_id) {
                Some(index) => {
                    let column = &mut ws.columns[index];
                    let position = closed.index.min(column.tabs.len());
                    column.tabs.insert(position, closed.tab);
                    column.shown_tab_id = tab_id;
                    ws.active_column_id = column.id.clone();
                }
                None => {
                    // Its column was closed too: it comes back as that column, after the
                    // active one, under its old id.
                    let n = ws.columns.len() as f64 + 1.0;
                    for column in &mut ws.columns {
                        column.width_fraction *= (n - 1.0) / n;
                    }
                    let index = ws.active_index() + 1;
                    ws.columns.insert(
                        index,
                        Column {
                            id: closed.column_id.clone(),
                            shown_tab_id: tab_id,
                            tabs: vec![closed.tab],
                            width_fraction: 1.0 / n,
                        },
                    );
                    ws.active_column_id = closed.column_id;
                }
            }
        }
        "tab.next" | "tab.previous" => {
            let ws = s.active_mut();
            let index = ws.active_index();
            let column = &mut ws.columns[index];
            let n = column.tabs.len();
            let position = column.shown_index();
            let next = if id == "tab.next" {
                (position + 1) % n
            } else {
                (position + n - 1) % n
            };
            column.shown_tab_id = column.tabs[next].id.clone();
        }
        "tab.move_to_column" => {
            let ws = s.active_mut();
            let number = column_number(ws, args).expect("enabled");
            let target_id = ws.columns[number - 1].id.clone();
            let index = ws.active_index();
            let tab_id = ws.active_tab().id.clone();
            let (tab, _) = take_tab(ws, index, &tab_id);
            let target = ws
                .columns
                .iter_mut()
                .find(|c| c.id == target_id)
                .expect("the target column survives");
            target.tabs.push(tab);
            target.shown_tab_id = tab_id;
            ws.active_column_id = target_id;
        }
        "tab.duplicate_to_column" => {
            let ws = s.active_mut();
            let number = column_number(ws, args).expect("enabled");
            let source = ws.active_tab().clone();
            let tab_id = ws.mint_id("t");
            let target = &mut ws.columns[number - 1];
            target.tabs.push(copy_tab(&source, tab_id.clone()));
            target.shown_tab_id = tab_id;
            ws.active_column_id = target.id.clone();
        }
        "tab.focus" => {
            let target = target_workspace(s, args).expect("enabled").id.clone();
            s.active_workspace_id = target;
            let tab_id = args.tab_id.clone().expect("enabled");
            let ws = s.active_mut();
            let (index, _) = ws.find_tab(&tab_id).expect("enabled");
            ws.columns[index].shown_tab_id = tab_id;
            ws.active_column_id = ws.columns[index].id.clone();
        }
        "tab.link" | "tab.pin" | "tab.follow" => {
            let ws = s.active_mut();
            let tab_id = ws.active_tab().id.clone();
            let tab = tab_mut(ws, &tab_id);
            match id {
                "tab.link" => {
                    tab.link_group = Some(non_empty(&args.group).unwrap_or("1").to_owned());
                    tab.pinned = false;
                }
                "tab.pin" => {
                    tab.link_group = None;
                    tab.pinned = true;
                }
                _ => {
                    tab.link_group = None;
                    tab.pinned = false;
                }
            }
        }
        "view.text_size_up" => s.reading.text_size_step += 1,
        "view.text_size_down" => s.reading.text_size_step -= 1,
        "view.text_size_reset" => s.reading.text_size_step = 0,
        "view.line_spacing" => {
            s.reading.line_spacing =
                one_of(args.number, &LINE_SPACINGS, "line spacing").expect("enabled");
        }
        "view.line_spacing_up" | "view.line_spacing_down" => {
            s.reading.line_spacing =
                step(&LINE_SPACINGS, s.reading.line_spacing, id_up(invocation)).expect("enabled");
        }
        "view.theme" => s.reading.theme = non_empty(&args.text).expect("enabled").to_owned(),
        "tool.width" => {
            s.ink.width = one_of(args.number, &INK_WIDTHS, "brush width").expect("enabled")
        }
        "tool.width_up" | "tool.width_down" => {
            s.ink.width = step(&INK_WIDTHS, s.ink.width, id_up(invocation)).expect("enabled");
        }
        "tool.swatch" => s.ink.swatch = non_empty(&args.text).expect("enabled").to_uppercase(),
        "tool.swap_swatches" => {
            let ink = &mut s.ink;
            std::mem::swap(&mut ink.swatch, &mut ink.alternate_swatch);
        }
        "column.resize" => {
            let ws = s.active_mut();
            let index = column_number(ws, args).expect("enabled") - 1;
            let share = args.number.expect("enabled");
            let others: f64 = ws
                .columns
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .map(|(_, c)| c.width_fraction)
                .sum();
            for (i, column) in ws.columns.iter_mut().enumerate() {
                column.width_fraction = if i == index {
                    share
                } else {
                    column.width_fraction * (1.0 - share) / others
                };
            }
        }
        "column.equalize" => {
            let ws = s.active_mut();
            let n = ws.columns.len() as f64;
            for column in &mut ws.columns {
                column.width_fraction = 1.0 / n;
            }
        }
        "tab.record_scroll" => {
            let target = target_workspace(s, args).expect("enabled").id.clone();
            let tab_id = args.tab_id.clone().expect("enabled");
            let ws = s
                .workspaces
                .iter_mut()
                .find(|w| w.id == target)
                .expect("enabled");
            tab_mut(ws, &tab_id).scroll_anchor = Some(ScrollAnchor {
                segment_id: non_empty(&args.segment_id).expect("enabled").to_owned(),
                scalar: args.scalar.expect("enabled"),
            });
        }
        "view.notes_dock" => {
            let ws = s.active_mut();
            ws.notes_dock.open = !ws.notes_dock.open;
        }
        _ => {
            if let Some(tool) = tool_of(id) {
                s.tool = tool;
            } else {
                unreachable!("every core command has an arm: {id}");
            }
        }
    }
    debug_assert!(s.check().is_ok(), "{id} broke the session: {:?}", s.check());
    let undo = before.map(|before| UndoRecord {
        command_id: id.to_owned(),
        title: spec.title.clone(),
        before,
    });
    Ok((undo, effects, true))
}

/// A command with its state, for menus and the palette.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandState {
    /// The command.
    pub spec: CommandSpec,
    /// `None` when it can run; otherwise why not.
    pub disabled: Option<DisabledReason>,
}

/// The outcome of [`Shell::apply`], [`Shell::undo`] or [`Shell::redo`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    /// The command that ran (for undo and redo, the command undone or redone).
    pub command_id: String,
    /// `false` when the host performs the command; the session did not change.
    pub handled: bool,
    /// Whether the command was pushed on the undo stack.
    pub recorded: bool,
    /// What else the host does.
    pub effects: Effects,
    /// The shell's revision after the command.
    pub revision: u64,
}

/// A session with its undo and redo stacks: what a host holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shell {
    /// The session.
    pub session: Session,
    /// Undo records, oldest first.
    pub undo_stack: Vec<UndoRecord>,
    /// Redo records, most recently undone last.
    pub redo_stack: Vec<UndoRecord>,
    /// Bumped by every change to the session; a host keys caches on it.
    pub revision: u64,
}

/// Back visits a tab keeps; the oldest is dropped beyond this.
pub const HISTORY_DEPTH: usize = 100;

/// Undo records a shell keeps.
pub const UNDO_DEPTH: usize = 100;

impl Shell {
    /// A shell over a checked session, with empty stacks.
    pub fn new(session: Session) -> Result<Shell, crate::shell::ShellError> {
        session.check()?;
        Ok(Shell {
            session,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            revision: 0,
        })
    }

    /// Whether `invocation` can run now.
    pub fn enabled(
        &self,
        invocation: &Invocation,
        context: &CommandContext,
    ) -> Result<(), DisabledReason> {
        enabled(
            &self.session,
            !self.undo_stack.is_empty(),
            !self.redo_stack.is_empty(),
            invocation,
            context,
        )
    }

    /// Every registered command with whether it can run now, without arguments (a command
    /// that needs one reports [`DisabledReason::NeedsArgument`]).
    #[must_use]
    pub fn commands(&self, context: &CommandContext) -> Vec<CommandState> {
        registry()
            .iter()
            .map(|spec| CommandState {
                spec: spec.clone(),
                disabled: self.enabled(&Invocation::new(&spec.id), context).err(),
            })
            .collect()
    }

    /// Runs an invocation, keeping the undo stack: an undoable command pushes its record
    /// and clears the redo stack. `edit.undo` and `edit.redo` run [`Shell::undo`] and
    /// [`Shell::redo`].
    pub fn apply(
        &mut self,
        invocation: &Invocation,
        context: &CommandContext,
    ) -> Result<Outcome, CommandError> {
        match invocation.command_id.as_str() {
            "edit.undo" => return self.undo(),
            "edit.redo" => return self.redo(),
            _ => {}
        }
        let (undo, effects, handled) = apply_in_place(&mut self.session, invocation, context)?;
        if !handled {
            return Ok(Outcome {
                command_id: invocation.command_id.clone(),
                handled: false,
                recorded: false,
                effects,
                revision: self.revision,
            });
        }
        let quiet = command(&invocation.command_id).is_some_and(|c| c.quiet);
        let recorded = undo.is_some();
        if let Some(record) = undo {
            self.undo_stack.push(record);
            if self.undo_stack.len() > UNDO_DEPTH {
                self.undo_stack.remove(0);
            }
            self.redo_stack.clear();
        }
        if !quiet {
            self.revision += 1;
        }
        Ok(Outcome {
            command_id: invocation.command_id.clone(),
            handled: true,
            recorded,
            effects,
            revision: self.revision,
        })
    }

    /// Swaps the session with a record's snapshot and returns the record that swaps back.
    fn swap(&mut self, record: UndoRecord) -> UndoRecord {
        let mut touched: Vec<String> = record
            .before
            .workspaces
            .iter()
            .map(|w| w.id.clone())
            .collect();
        for w in &self.session.workspaces {
            if !record.before.workspace_ids.contains(&w.id) {
                touched.push(w.id.clone());
            }
        }
        let inverse = UndoRecord {
            command_id: record.command_id.clone(),
            title: record.title.clone(),
            before: Snapshot::take(&self.session, &touched),
        };
        record.before.restore(&mut self.session);
        self.revision += 1;
        inverse
    }

    /// Takes back the most recent undoable command.
    pub fn undo(&mut self) -> Result<Outcome, CommandError> {
        let record = self.undo_stack.pop().ok_or(CommandError::Disabled {
            command_id: "edit.undo".to_owned(),
            reason: DisabledReason::NothingToUndo,
        })?;
        let command_id = record.command_id.clone();
        let inverse = self.swap(record);
        self.redo_stack.push(inverse);
        Ok(Outcome {
            command_id,
            handled: true,
            recorded: false,
            effects: Effects::default(),
            revision: self.revision,
        })
    }

    /// Runs the most recently undone command again.
    pub fn redo(&mut self) -> Result<Outcome, CommandError> {
        let record = self.redo_stack.pop().ok_or(CommandError::Disabled {
            command_id: "edit.redo".to_owned(),
            reason: DisabledReason::NothingToRedo,
        })?;
        let command_id = record.command_id.clone();
        let inverse = self.swap(record);
        self.undo_stack.push(inverse);
        Ok(Outcome {
            command_id,
            handled: true,
            recorded: false,
            effects: Effects::default(),
            revision: self.revision,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// The session of the conformance fixture `shell-commands`: "John study" (three
    /// columns, a link group, a pinned tab and a following lexicon) and "Genesis".
    pub(crate) fn fixture_session() -> Session {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../conformance/fixtures/shell-commands.json"
        ))
        .unwrap();
        serde_json::from_value(fixture["session"].clone()).unwrap()
    }

    /// Plausible arguments for every command, so each can run on the fixture session.
    fn args_for(spec: &CommandSpec) -> CommandArgs {
        let mut args = CommandArgs::default();
        match spec.argument {
            ArgumentShape::None => {}
            ArgumentShape::ColumnNumber => args.column = Some(2),
            ArgumentShape::Text => args.text = Some("Renamed".into()),
            ArgumentShape::Reference => args.reference = Some("John.3.18".into()),
            ArgumentShape::Workspace => args.workspace_id = Some("w2".into()),
            ArgumentShape::Tab => {
                args.workspace_id = Some("w1".into());
                args.tab_id = Some("t3".into());
            }
            ArgumentShape::LinkGroup => args.group = Some("2".into()),
            ArgumentShape::Number => {
                args.number = Some(if spec.id == "view.line_spacing" {
                    1.7
                } else {
                    8.0
                });
            }
            ArgumentShape::ColumnWidth => {
                args.column = Some(2);
                args.number = Some(0.5);
            }
            ArgumentShape::Scroll => {
                args.tab_id = Some("t3".into());
                args.segment_id = Some("John.3.16".into());
                args.scalar = Some(12);
            }
        }
        if spec.id == "view.theme" {
            args.text = Some("dark".into());
        }
        if spec.id == "tool.swatch" {
            args.text = Some("#a1b2c3".into());
        }
        args
    }

    #[test]
    fn ids_are_unique_and_shortcuts_do_not_collide() {
        let mut ids = BTreeSet::new();
        for spec in registry() {
            assert!(ids.insert(spec.id.as_str()), "{}", spec.id);
            assert!(!spec.title.is_empty());
            if spec.argument == ArgumentShape::ColumnNumber {
                assert!(
                    spec.shortcuts.web.iter().all(|s| s.contains("{n}")),
                    "{}",
                    spec.id
                );
            }
            if spec.undoable {
                assert!(
                    spec.handled_by_core,
                    "{} undoable but host-performed",
                    spec.id
                );
            }
        }
        for platform in [Platform::Web, Platform::Mac, Platform::Ipad] {
            let mut seen = BTreeSet::new();
            for spec in registry() {
                for key in spec.shortcuts.for_platform(platform) {
                    assert!(
                        seen.insert(key.clone()),
                        "{key} bound twice on {platform:?}"
                    );
                }
            }
        }
        // specs/22 §6.
        let keys = |id: &str| command(id).unwrap().shortcuts.clone();
        assert_eq!(keys("column.focus").web, vec!["Alt+{n}"]);
        assert_eq!(keys("column.focus").mac, vec!["Alt+{n}", "Cmd+{n}"]);
        assert_eq!(keys("tab.new").ipad, vec!["Alt+T", "Cmd+T"]);
        assert_eq!(keys("workspace.new").web, vec!["Alt+Shift+N"]);
        assert!(command("tool.eraser").unwrap().bare_key);
    }

    #[test]
    fn every_command_runs_and_every_undo_restores_the_session_exactly() {
        let session = fixture_session();
        let context = CommandContext {
            has_selection: true,
        };
        for spec in registry() {
            let invocation = Invocation::with(&spec.id, args_for(spec));
            let mut shell = Shell::new(session.clone()).unwrap();
            // Give back and forward something to do.
            shell
                .apply(
                    &Invocation::with(
                        "nav.navigate",
                        CommandArgs {
                            reference: Some("John.3.17".into()),
                            ..CommandArgs::default()
                        },
                    ),
                    &context,
                )
                .unwrap();
            if spec.id == "edit.redo" {
                shell.undo().unwrap();
            }
            if spec.id == "nav.forward" {
                shell.apply(&Invocation::new("nav.back"), &context).unwrap();
            }
            if spec.id == "column.move_left" {
                let focus = Invocation::with(
                    "column.focus",
                    CommandArgs {
                        column: Some(2),
                        ..CommandArgs::default()
                    },
                );
                shell.apply(&focus, &context).unwrap();
            }
            if spec.id == "tab.reopen_closed" {
                shell.apply(&Invocation::new("tab.next"), &context).unwrap();
                shell
                    .apply(&Invocation::new("tab.close"), &context)
                    .unwrap();
            }
            let before = shell.session.clone();
            let outcome = shell.apply(&invocation, &context);
            let outcome = match outcome {
                Ok(outcome) => outcome,
                Err(error) => panic!("{} failed: {error}", spec.id),
            };
            shell.session.check().unwrap();
            assert_eq!(outcome.handled, spec.handled_by_core, "{}", spec.id);
            if spec.id.starts_with("edit.") {
                continue;
            }
            assert_eq!(outcome.recorded, spec.undoable, "{}", spec.id);
            if spec.undoable {
                let after = shell.session.clone();
                assert_ne!(before, after, "{} changed nothing", spec.id);
                shell.undo().unwrap();
                assert_eq!(shell.session, before, "undo {}", spec.id);
                shell.redo().unwrap();
                assert_eq!(shell.session, after, "redo {}", spec.id);
            }
        }
    }

    #[test]
    fn pure_apply_leaves_its_input_alone_and_refuses_history_commands() {
        let session = fixture_session();
        let copy = session.clone();
        let applied = apply(
            &session,
            &Invocation::new("column.new"),
            &CommandContext::default(),
        )
        .unwrap();
        assert_eq!(session, copy);
        assert_eq!(applied.session.active().columns.len(), 4);
        assert_eq!(applied.undo.as_ref().unwrap().before.workspaces.len(), 1);
        assert_eq!(
            apply(
                &session,
                &Invocation::new("edit.undo"),
                &CommandContext::default()
            ),
            Err(CommandError::RequiresHistory {
                command_id: "edit.undo".into()
            })
        );
    }

    #[test]
    fn a_new_command_clears_redo_and_the_stack_is_bounded() {
        let mut shell = Shell::new(fixture_session()).unwrap();
        let context = CommandContext::default();
        shell
            .apply(&Invocation::new("view.notes_dock"), &context)
            .unwrap();
        shell.undo().unwrap();
        assert_eq!(shell.redo_stack.len(), 1);
        shell
            .apply(&Invocation::new("view.notes_dock"), &context)
            .unwrap();
        assert!(shell.redo_stack.is_empty());
        for _ in 0..(UNDO_DEPTH + 10) {
            shell
                .apply(&Invocation::new("view.notes_dock"), &context)
                .unwrap();
        }
        assert_eq!(shell.undo_stack.len(), UNDO_DEPTH);
        let revision = shell.revision;
        shell.apply(&Invocation::new("tool.pen"), &context).unwrap();
        assert_eq!(shell.revision, revision + 1);
    }

    #[test]
    fn errors_serialize_with_a_kind_tag() {
        let error = CommandError::Disabled {
            command_id: "column.close".into(),
            reason: DisabledReason::OnlyColumn,
        };
        let json = serde_json::to_value(&error).unwrap();
        assert_eq!(json["kind"], "disabled");
        assert_eq!(json["reason"]["kind"], "only_column");
        assert_eq!(
            error.to_string(),
            "column.close is disabled: This is the only column"
        );
    }
}
