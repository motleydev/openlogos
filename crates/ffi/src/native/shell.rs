//! The workspace shell over `crates/workspaces` (B070, specs/22): a [`ShellHandle`] object
//! holding a session with its undo and redo stacks, the command registry, `apply`, `undo`,
//! `redo` and the palette query. Additive; nothing existing is renamed.
//!
//! The handle keeps the palette's [`PaletteIndex`] keyed on the shell revision, so a host can
//! call `paletteQuery` on every keystroke and pay for indexing only after a command changed
//! the session. Session state crosses as read-only view records; persistence crosses as the
//! schema JSON of the session record and each workspace record.
//!
//! B076 adds the contextual tool well: `tool_well` with a [`ShellCommandContext`] (the
//! selection and where focus is), `set_resource_languages`, and `commands_in` / `apply_in`,
//! the context-taking forms of `commands` / `apply` (which keep meaning "focus on the tab").
//!
//! B077 adds the context menu: `context_menu` with a [`ShellContextTarget`] (what was
//! right-clicked) returns a [`ShellContextMenu`]: the focus change to apply first, the context
//! to dispatch rows under, and the sections. A submenu is one level deep
//! ([`ShellContextMenuSubitem`]), as the core builds it.
//!
//! B078 adds continuous reading: `set_resource_schemes`, `ShellOutcome.positions`
//! ([`ShellTabPosition`]: every tab a navigation or a scroll moved, followers with their
//! mapped references) and `ShellTab.highlight_range`.

use std::sync::{Arc, Mutex};

use openlogos_conformance::FixtureTextProvider;
use openlogos_workspaces::commands::{
    ArgumentShape, CommandArgs, CommandContext, CommandError, CommandSpec, DisabledReason, Focus,
    Invocation, Outcome, Platform, Scope, Shell, registry,
};
use openlogos_workspaces::menu::{ContextMenu, ContextMenuItem, ContextTarget};
use openlogos_workspaces::migrate_workspace_to_2;
use openlogos_workspaces::palette::{
    ColumnTarget, PaletteContext, PaletteIndex, PaletteItem, PaletteMode, PaletteSection,
    TextProvider, palette_query_indexed,
};
use openlogos_workspaces::reading::TabPosition;
use openlogos_workspaces::shell::{Session, SessionRecord};
use openlogos_workspaces::view::TabView;
use openlogos_workspaces::well::ToolWell;

/// Why a shell call failed. No field is called `message` or `cause` (Kotlin's `Throwable`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum ShellCommandError {
    /// A session or workspace document that does not parse, migrate or check.
    #[error("invalid shell document: {detail}")]
    InvalidDocument {
        /// What was wrong.
        detail: String,
    },
    /// A command id the registry does not have.
    #[error("unknown command {command_id}")]
    UnknownCommand {
        /// The id.
        command_id: String,
    },
    /// A registered command that cannot run now.
    #[error("{command_id} is disabled: {reason}")]
    Disabled {
        /// The command.
        command_id: String,
        /// The reason's tag (`only_column`, `needs_selection`, ...).
        kind: String,
        /// The reason, for display.
        reason: String,
    },
}

fn kind_of<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_owned))
        .unwrap_or_default()
}

fn name_of<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

impl From<CommandError> for ShellCommandError {
    fn from(error: CommandError) -> Self {
        match error {
            CommandError::UnknownCommand { command_id } => {
                ShellCommandError::UnknownCommand { command_id }
            }
            CommandError::Disabled { command_id, reason } => ShellCommandError::Disabled {
                command_id,
                kind: kind_of(&reason),
                reason: reason.to_string(),
            },
            CommandError::RequiresHistory { command_id } => {
                ShellCommandError::UnknownCommand { command_id }
            }
        }
    }
}

pub(crate) fn invalid(detail: impl ToString) -> ShellCommandError {
    ShellCommandError::InvalidDocument {
        detail: detail.to_string(),
    }
}

/// One tab, as a host renders it.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShellTab {
    /// Stable id within the workspace.
    pub id: String,
    /// `reading`, `notes`, `research` or `lexicon`.
    pub kind: String,
    /// The resource.
    pub content_id: String,
    /// The resource revision.
    pub content_revision: String,
    /// The passage.
    pub reference: Option<String>,
    /// The resource's registered short name, else its id (`setResourceTitles`).
    pub resource_title: String,
    /// The passage in short form for the identity row (`Jn 3:16`), if it parses.
    pub reference_label: Option<String>,
    /// The chapter in short form for the tab strip (`Jn 3`), if the passage parses.
    pub chapter_label: Option<String>,
    /// Scroll anchor segment, if scrolled.
    pub scroll_segment_id: Option<String>,
    /// Scroll anchor offset in scalar values.
    pub scroll_scalar: Option<u32>,
    /// `pinned`, `follows` or `group`.
    pub link_mode: String,
    /// The group label when `link_mode` is `group`.
    pub link_group: Option<String>,
    /// Whether Back would move this tab.
    pub can_go_back: bool,
    /// Whether Forward would move this tab.
    pub can_go_forward: bool,
    /// The word a Lexicon tab looks up (B076, `tab.open_lexicon`).
    pub lexicon_word: Option<String>,
    /// The language a Lexicon tab studies, a BCP 47 tag (B074).
    pub lexicon_language: Option<String>,
    /// The verses the last navigation named, OSIS, to mark until the next one (B078).
    pub highlight_range: Option<String>,
}

/// One column; `number` is its position, 1-based.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShellColumn {
    /// Stable id.
    pub id: String,
    /// Position, 1-based: what the identity row shows.
    pub number: u32,
    /// Its tabs, in order.
    pub tabs: Vec<ShellTab>,
    /// The tab shown.
    pub shown_tab_id: String,
    /// Width share.
    pub width_fraction: f64,
}

/// One workspace.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShellWorkspace {
    /// Stable id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Columns 1…n.
    pub columns: Vec<ShellColumn>,
    /// The active column's number.
    pub active_column_number: u32,
    /// The note in the dock.
    pub notes_dock_note_id: Option<String>,
    /// Whether the dock is open.
    pub notes_dock_open: bool,
}

/// The whole session as a host renders it.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShellSession {
    /// Every workspace, in order.
    pub workspaces: Vec<ShellWorkspace>,
    /// The active workspace.
    pub active_workspace_id: String,
    /// The selected tool (`select`, `highlight`, `pen`, `highlighter_pen`, `eraser`, `collect`,
    /// `word_study`, `type`), as stored.
    pub tool: String,
    /// The tool in effect for the active tab (B076); `tool_well` answers for the dock.
    pub effective_tool: String,
    /// The active tab's tool-well context: `translation`, `original`, `notes`, `reference`.
    pub tool_context: String,
    /// Bumped by every change.
    pub revision: u64,
    /// The command Undo would take back, if any.
    pub undo_title: Option<String>,
    /// The command Redo would run again, if any.
    pub redo_title: Option<String>,
    /// Text size, line spacing and theme.
    pub reading: ShellReading,
    /// Brush width and swatches.
    pub ink: ShellInk,
}

/// How the reader has set up the text (`shell::ReadingSettings`).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShellReading {
    /// Steps from the platform's default text size.
    pub text_size_step: i32,
    /// Line height as a multiple of the font size: 1.35, 1.5 or 1.7.
    pub line_spacing: f64,
    /// `system`, `light`, `dark` or `sepia`.
    pub theme: String,
}

/// The tool palette's brush width and wells (`shell::InkSettings`).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShellInk {
    /// Brush width.
    pub width: f64,
    /// The current swatch, `#RRGGBB`.
    pub swatch: String,
    /// The alternate swatch.
    pub alternate_swatch: String,
}

/// The core's pane minimums and measure caps (`workspaces::layout_constants`), in points.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct ShellLayoutConstants {
    /// Narrowest useful reading pane at standard text sizes.
    pub pane_minimum_standard_pt: f64,
    /// The same at accessibility text sizes.
    pub pane_minimum_accessibility_pt: f64,
    /// Column width above which text is centred at the measure, standard sizes.
    pub measure_cap_standard_pt: f64,
    /// The same at accessibility sizes.
    pub measure_cap_accessibility_pt: f64,
}

/// The pane minimums and measure caps, so hosts stop mirroring the constants.
#[uniffi::export]
#[must_use]
pub fn layout_constants() -> ShellLayoutConstants {
    let c = openlogos_workspaces::layout_constants();
    ShellLayoutConstants {
        pane_minimum_standard_pt: c.pane_minimum_standard_pt,
        pane_minimum_accessibility_pt: c.pane_minimum_accessibility_pt,
        measure_cap_standard_pt: c.measure_cap_standard_pt,
        measure_cap_accessibility_pt: c.measure_cap_accessibility_pt,
    }
}

/// The persisted form: the session record and one record per workspace, as schema JSON.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ShellRecords {
    /// schemas/session.schema.json.
    pub session_json: String,
    /// schemas/workspace-2.schema.json, in session order.
    pub workspace_jsons: Vec<String>,
}

/// Arguments for a command (`commands::CommandArgs`).
#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct ShellCommandArgs {
    /// A column number, 1-based.
    #[uniffi(default = None)]
    pub column: Option<u32>,
    /// Open in a new column.
    #[uniffi(default = false)]
    pub new_column: bool,
    /// Free text.
    #[uniffi(default = None)]
    pub text: Option<String>,
    /// A passage.
    #[uniffi(default = None)]
    pub reference: Option<String>,
    /// A workspace id.
    #[uniffi(default = None)]
    pub workspace_id: Option<String>,
    /// A tab id.
    #[uniffi(default = None)]
    pub tab_id: Option<String>,
    /// A link group.
    #[uniffi(default = None)]
    pub group: Option<String>,
    /// A number: a line spacing, a brush width, a column's width share.
    #[uniffi(default = None)]
    pub number: Option<f64>,
    /// A scroll anchor's segment.
    #[uniffi(default = None)]
    pub segment_id: Option<String>,
    /// A scroll anchor's offset in scalar values.
    #[uniffi(default = None)]
    pub scalar: Option<u32>,
    /// Where `search.text` looks: `edition`, `all`, `workspace` or `chapter` (B077).
    #[uniffi(default = None)]
    pub scope: Option<String>,
    /// An edition id: what `tab.open_edition` opens, what a search or copy reads (B077).
    #[uniffi(default = None)]
    pub resource_id: Option<String>,
    /// A slot in a column's tab list, 0-based: where `tab.move` drops a tab (B074).
    #[uniffi(default = None)]
    pub index: Option<u32>,
}

impl From<ShellCommandArgs> for CommandArgs {
    fn from(a: ShellCommandArgs) -> Self {
        CommandArgs {
            column: a.column,
            new_column: a.new_column,
            text: a.text,
            reference: a.reference,
            workspace_id: a.workspace_id,
            tab_id: a.tab_id,
            group: a.group,
            number: a.number,
            segment_id: a.segment_id,
            scalar: a.scalar,
            scope: a.scope,
            resource_id: a.resource_id,
            index: a.index,
        }
    }
}

impl From<CommandArgs> for ShellCommandArgs {
    fn from(a: CommandArgs) -> Self {
        ShellCommandArgs {
            column: a.column,
            new_column: a.new_column,
            text: a.text,
            reference: a.reference,
            workspace_id: a.workspace_id,
            tab_id: a.tab_id,
            group: a.group,
            number: a.number,
            segment_id: a.segment_id,
            scalar: a.scalar,
            scope: a.scope,
            resource_id: a.resource_id,
            index: a.index,
        }
    }
}

/// Default shortcuts per platform; the first is the default, the rest native aliases.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ShellShortcuts {
    /// Web.
    pub web: Vec<String>,
    /// Mac.
    pub mac: Vec<String>,
    /// iPad.
    pub ipad: Vec<String>,
}

/// One registered command, with whether it can run now.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ShellCommandInfo {
    /// Stable id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Other names.
    pub aliases: Vec<String>,
    /// Further words.
    pub keywords: Vec<String>,
    /// `session`, `workspace`, `column`, `tab` or `selection`.
    pub scope: String,
    /// `none`, `column_number`, `text`, `reference`, `workspace`, `tab` or `link_group`.
    pub argument: String,
    /// Shortcuts.
    pub shortcuts: ShellShortcuts,
    /// A bare key, only outside text fields.
    pub bare_key: bool,
    /// Whether it can be undone.
    pub undoable: bool,
    /// Whether the core performs it.
    pub handled_by_core: bool,
    /// Whether it records state without bumping the revision (`tab.record_scroll`).
    pub quiet: bool,
    /// The disabled reason's tag, `None` when it can run.
    pub disabled_kind: Option<String>,
    /// The disabled reason for display.
    pub disabled_reason: Option<String>,
}

fn command_info(spec: &CommandSpec, disabled: Option<DisabledReason>) -> ShellCommandInfo {
    ShellCommandInfo {
        id: spec.id.clone(),
        title: spec.title.clone(),
        aliases: spec.aliases.clone(),
        keywords: spec.keywords.clone(),
        scope: name_of::<Scope>(&spec.scope),
        argument: name_of::<ArgumentShape>(&spec.argument),
        shortcuts: ShellShortcuts {
            web: spec.shortcuts.web.clone(),
            mac: spec.shortcuts.mac.clone(),
            ipad: spec.shortcuts.ipad.clone(),
        },
        bare_key: spec.bare_key,
        undoable: spec.undoable,
        handled_by_core: spec.handled_by_core,
        quiet: spec.quiet,
        disabled_kind: disabled.as_ref().map(kind_of),
        disabled_reason: disabled.as_ref().map(ToString::to_string),
    }
}

/// What a command did.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ShellOutcome {
    /// The command that ran (for undo and redo, the one undone or redone).
    pub command_id: String,
    /// `false`: the host performs it; the session did not change.
    pub handled: bool,
    /// Whether it went on the undo stack.
    pub recorded: bool,
    /// Tabs that followed a navigation.
    pub followed_tab_ids: Vec<String>,
    /// Tabs whose passage moved, driver first (B078): relabel each, scroll each follower.
    pub positions: Vec<ShellTabPosition>,
    /// The revision after it.
    pub revision: u64,
}

/// Where a tab is after a command moved it (`reading::TabPosition`, B078).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ShellTabPosition {
    /// The workspace.
    pub workspace_id: String,
    /// The tab.
    pub tab_id: String,
    /// Whether it drove the move (`false`: it followed).
    pub driver: bool,
    /// Whether the move came from scrolling: relabel in place, scroll a follower without
    /// rebuilding it.
    pub scrolled: bool,
    /// Its reference now.
    pub reference: String,
    /// The segment to bring to the top, if the reference names a verse.
    pub segment_id: Option<String>,
    /// The identity row's label (`Jn 3:16`).
    pub reference_label: Option<String>,
    /// The tab strip's label (`Jn 3`).
    pub chapter_label: Option<String>,
    /// The verses marked on it.
    pub highlight_range: Option<String>,
    /// `same`, `unregistered`, `exact`, `partial`, `nearest` or `unmapped`.
    pub mapping: String,
}

impl From<TabPosition> for ShellTabPosition {
    fn from(p: TabPosition) -> Self {
        ShellTabPosition {
            mapping: name_of(&p.mapping),
            workspace_id: p.workspace_id,
            tab_id: p.tab_id,
            driver: p.driver,
            scrolled: p.scrolled,
            reference: p.reference,
            segment_id: p.segment_id,
            reference_label: p.reference_label,
            chapter_label: p.chapter_label,
            highlight_range: p.highlight_range,
        }
    }
}

impl From<Outcome> for ShellOutcome {
    fn from(o: Outcome) -> Self {
        ShellOutcome {
            command_id: o.command_id,
            handled: o.handled,
            recorded: o.recorded,
            followed_tab_ids: o.effects.followed_tab_ids,
            positions: o.effects.positions.into_iter().map(Into::into).collect(),
            revision: o.revision,
        }
    }
}

/// Whose shortcuts to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ShellPlatform {
    /// A browser.
    Web,
    /// The Mac app.
    Mac,
    /// The iPad app.
    Ipad,
}

/// The palette's sections, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ShellPaletteSection {
    /// A parsed passage.
    Passage,
    /// Commands.
    Commands,
    /// Tabs and workspaces.
    Tabs,
    /// Full-text hits.
    Text,
}

impl From<PaletteSection> for ShellPaletteSection {
    fn from(s: PaletteSection) -> Self {
        match s {
            PaletteSection::Passage => ShellPaletteSection::Passage,
            PaletteSection::Commands => ShellPaletteSection::Commands,
            PaletteSection::Tabs => ShellPaletteSection::Tabs,
            PaletteSection::Text => ShellPaletteSection::Text,
        }
    }
}

impl From<ShellPaletteSection> for PaletteSection {
    fn from(s: ShellPaletteSection) -> Self {
        match s {
            ShellPaletteSection::Passage => PaletteSection::Passage,
            ShellPaletteSection::Commands => PaletteSection::Commands,
            ShellPaletteSection::Tabs => PaletteSection::Tabs,
            ShellPaletteSection::Text => PaletteSection::Text,
        }
    }
}

/// What the host knows for a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ShellPaletteContext {
    /// Whether text is selected.
    pub has_selection: bool,
    /// Whose shortcuts to show.
    pub platform: ShellPlatform,
    /// The expanded section, if any.
    #[uniffi(default = None)]
    pub expanded: Option<ShellPaletteSection>,
    /// Where focus is (B076); `None` means the tab.
    #[uniffi(default = None)]
    pub focus: Option<ShellFocus>,
}

/// Where focus is, as far as the tool well cares (`commands::Focus`, B076).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ShellFocus {
    /// The active column's shown tab.
    Tab,
    /// The notes dock.
    NotesDock,
    /// A column showing no tab.
    Empty,
}

impl From<ShellFocus> for Focus {
    fn from(f: ShellFocus) -> Self {
        match f {
            ShellFocus::Tab => Focus::Tab,
            ShellFocus::NotesDock => Focus::NotesDock,
            ShellFocus::Empty => Focus::Empty,
        }
    }
}

/// What the host knows that the session does not (`commands::CommandContext`, B076).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ShellCommandContext {
    /// Whether text is selected.
    pub has_selection: bool,
    /// Where focus is.
    pub focus: ShellFocus,
}

impl From<ShellCommandContext> for CommandContext {
    fn from(c: ShellCommandContext) -> Self {
        CommandContext {
            has_selection: c.has_selection,
            focus: c.focus.into(),
        }
    }
}

/// One button of the tool well (`well::ToolWellItem`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ShellToolWellItem {
    /// The command a click dispatches (`apply_in`).
    pub command_id: String,
    /// The tool it chooses, for a mode tool.
    pub tool: Option<String>,
    /// Tooltip and accessibility label.
    pub label: String,
    /// The default key (`V`, `Cmd+B`), the same on web and Mac.
    pub key: Option<String>,
    /// A neutral icon name the host maps to an SF Symbol.
    pub icon: String,
    /// Whether it is the tool in effect.
    pub active: bool,
    /// An ink item's value: the swatch `#RRGGBB`, the brush width.
    pub value: Option<String>,
    /// Why it cannot run now, in words.
    pub disabled_text: Option<String>,
}

/// One group of the well.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ShellToolWellGroup {
    /// `tools`, `ink` or `selection`.
    pub kind: String,
    /// Its buttons, in order.
    pub items: Vec<ShellToolWellItem>,
}

/// The tool well for what has focus (specs/22 §4.1).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ShellToolWell {
    /// `translation`, `original`, `notes`, `reference` or `none`.
    pub context: String,
    /// The stored tool.
    pub tool: String,
    /// The tool in effect.
    pub effective_tool: String,
    /// `tools`, then `ink` and `selection` where they apply.
    pub groups: Vec<ShellToolWellGroup>,
}

impl From<ToolWell> for ShellToolWell {
    fn from(w: ToolWell) -> Self {
        ShellToolWell {
            context: w.context.name().to_owned(),
            tool: w.tool.name().to_owned(),
            effective_tool: w.effective_tool.name().to_owned(),
            groups: w
                .groups
                .into_iter()
                .map(|g| ShellToolWellGroup {
                    kind: name_of(&g.kind),
                    items: g
                        .items
                        .into_iter()
                        .map(|i| ShellToolWellItem {
                            command_id: i.command_id,
                            tool: i.tool.map(|t| t.name().to_owned()),
                            label: i.label,
                            key: i.key,
                            icon: i.icon,
                            active: i.active,
                            value: i.value,
                            disabled_text: i.disabled_text,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

/// What was right-clicked (`menu::ContextTarget`, B077). A reading or note target's `tab_id`
/// absent means the active tab, or the notes dock for a note.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ShellContextTarget {
    /// A word in reading text; the host selects it as it opens the menu.
    Word {
        /// The word as shown.
        text: String,
        /// Its passage, OSIS.
        reference: Option<String>,
        /// Its segment.
        segment_id: Option<String>,
        /// Where it starts in the segment, in scalar values.
        scalar_start: Option<u32>,
        /// Where it ends, exclusive.
        scalar_end: Option<u32>,
        /// Its edition; the tab's resource when absent.
        resource_id: Option<String>,
        /// The tab it is in.
        tab_id: Option<String>,
    },
    /// Selected reading text.
    Selection {
        /// The selected text.
        text: String,
        /// The passage it starts in.
        reference: Option<String>,
        /// The passage it ends in, when another.
        end_reference: Option<String>,
        /// Its edition; the tab's resource when absent.
        resource_id: Option<String>,
        /// The tab it is in.
        tab_id: Option<String>,
    },
    /// A spot or selected text in a note.
    NoteText {
        /// The selected text; `None` for a plain spot.
        text: Option<String>,
        /// The Notes tab; the dock when absent.
        tab_id: Option<String>,
    },
    /// A passage link in a note.
    NoteLink {
        /// The linked passage, OSIS.
        reference: String,
        /// The Notes tab; the dock when absent.
        tab_id: Option<String>,
    },
    /// A tab in a strip.
    Tab {
        /// The tab.
        tab_id: String,
    },
    /// A column's identity row or badge.
    Column {
        /// Its number, 1-based.
        column: u32,
    },
    /// The empty area of a column.
    Background {
        /// Its number; the active column when absent.
        column: Option<u32>,
    },
}

impl From<ShellContextTarget> for ContextTarget {
    fn from(t: ShellContextTarget) -> Self {
        match t {
            ShellContextTarget::Word {
                text,
                reference,
                segment_id,
                scalar_start,
                scalar_end,
                resource_id,
                tab_id,
            } => ContextTarget::Word {
                text,
                reference,
                segment_id,
                scalar_start,
                scalar_end,
                resource_id,
                tab_id,
            },
            ShellContextTarget::Selection {
                text,
                reference,
                end_reference,
                resource_id,
                tab_id,
            } => ContextTarget::Selection {
                text,
                reference,
                end_reference,
                resource_id,
                tab_id,
            },
            ShellContextTarget::NoteText { text, tab_id } => {
                ContextTarget::NoteText { text, tab_id }
            }
            ShellContextTarget::NoteLink { reference, tab_id } => {
                ContextTarget::NoteLink { reference, tab_id }
            }
            ShellContextTarget::Tab { tab_id } => ContextTarget::Tab { tab_id },
            ShellContextTarget::Column { column } => ContextTarget::Column { column },
            ShellContextTarget::Background { column } => ContextTarget::Background { column },
        }
    }
}

/// A command with its arguments: the focus change a menu runs first.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShellInvocation {
    /// The command.
    pub command_id: String,
    /// Its arguments.
    pub args: ShellCommandArgs,
}

/// A submenu row (`menu::ContextMenuItem` without a further submenu).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShellContextMenuSubitem {
    /// The command a click dispatches (`apply_in` under the menu's context).
    pub command_id: String,
    /// Its arguments.
    pub args: ShellCommandArgs,
    /// What the row says.
    pub label: String,
    /// The default key (`Alt+W`, `Cmd+C`), the same on web and Mac.
    pub key: Option<String>,
    /// A neutral icon name the host maps to an SF Symbol.
    pub icon: String,
    /// Whether it is the current choice (theme, swatch, link mode).
    pub active: bool,
    /// A value to show: a swatch `#RRGGBB`.
    pub value: Option<String>,
    /// Why it cannot run now, in words.
    pub disabled_text: Option<String>,
}

/// A top-level row; with a submenu it carries its first enabled child's command.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShellContextMenuItem {
    /// The command a click dispatches.
    pub command_id: String,
    /// Its arguments.
    pub args: ShellCommandArgs,
    /// What the row says.
    pub label: String,
    /// The default key, the same on web and Mac.
    pub key: Option<String>,
    /// A neutral icon name.
    pub icon: String,
    /// Whether it is the current choice.
    pub active: bool,
    /// A value to show.
    pub value: Option<String>,
    /// Why it cannot run now, in words.
    pub disabled_text: Option<String>,
    /// Its submenu; empty for a plain row.
    pub submenu: Vec<ShellContextMenuSubitem>,
}

/// A run of rows between separators.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShellContextMenuSection {
    /// A heading a host may show.
    pub label: Option<String>,
    /// Its rows.
    pub items: Vec<ShellContextMenuItem>,
}

/// The context menu for one target (specs/22 §4.2).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShellContextMenu {
    /// `word`, `selection`, `note_text`, `note_link`, `tab`, `column` or `background`.
    pub target_kind: String,
    /// A header; `None` for none.
    pub title: Option<String>,
    /// The focus change to apply before the chosen row.
    pub focus: Option<ShellInvocation>,
    /// The context to dispatch rows under.
    pub context: ShellCommandContext,
    /// The sections, in order.
    pub sections: Vec<ShellContextMenuSection>,
}

impl From<Focus> for ShellFocus {
    fn from(f: Focus) -> Self {
        match f {
            Focus::Tab => ShellFocus::Tab,
            Focus::NotesDock => ShellFocus::NotesDock,
            Focus::Empty => ShellFocus::Empty,
        }
    }
}

fn subitem(i: ContextMenuItem) -> ShellContextMenuSubitem {
    ShellContextMenuSubitem {
        command_id: i.command_id,
        args: i.args.into(),
        label: i.label,
        key: i.key,
        icon: i.icon,
        active: i.active,
        value: i.value,
        disabled_text: i.disabled_text,
    }
}

impl From<ContextMenu> for ShellContextMenu {
    fn from(m: ContextMenu) -> Self {
        ShellContextMenu {
            target_kind: m.target_kind.name().to_owned(),
            title: m.title,
            focus: m.focus.map(|f| ShellInvocation {
                command_id: f.command_id,
                args: f.args.into(),
            }),
            context: ShellCommandContext {
                has_selection: m.context.has_selection,
                focus: m.context.focus.into(),
            },
            sections: m
                .sections
                .into_iter()
                .map(|s| ShellContextMenuSection {
                    label: s.label,
                    items: s
                        .items
                        .into_iter()
                        .map(|i| ShellContextMenuItem {
                            command_id: i.command_id,
                            args: i.args.into(),
                            label: i.label,
                            key: i.key,
                            icon: i.icon,
                            active: i.active,
                            value: i.value,
                            disabled_text: i.disabled_text,
                            submenu: i.submenu.into_iter().map(subitem).collect(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

/// One result row.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShellPaletteItem {
    /// Its section.
    pub section: ShellPaletteSection,
    /// Main line.
    pub title: String,
    /// Second line.
    pub subtitle: Option<String>,
    /// Ranking score.
    pub score: u32,
    /// What Enter dispatches: pass to `apply`.
    pub command_id: String,
    /// Its arguments.
    pub args: ShellCommandArgs,
    /// The disabled reason's tag, if Enter would do nothing.
    pub disabled_kind: Option<String>,
    /// The disabled reason for display.
    pub disabled_reason: Option<String>,
    /// The platform's default shortcut.
    pub shortcut: Option<String>,
}

impl From<PaletteItem> for ShellPaletteItem {
    fn from(item: PaletteItem) -> Self {
        ShellPaletteItem {
            section: item.section.into(),
            title: item.title,
            subtitle: item.subtitle,
            score: item.score,
            command_id: item.invocation.command_id,
            args: item.invocation.args.into(),
            disabled_kind: item.disabled.as_ref().map(kind_of),
            disabled_reason: item.disabled.as_ref().map(ToString::to_string),
            shortcut: item.shortcut,
        }
    }
}

/// One section of results.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShellPaletteGroup {
    /// The section.
    pub section: ShellPaletteSection,
    /// Its rows, capped.
    pub items: Vec<ShellPaletteItem>,
    /// Matches before the cap.
    pub total: u32,
}

/// The answer to one query.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShellPaletteResult {
    /// `all`, `commands`, `tabs`, `text` or `passage`.
    pub mode: String,
    /// The query without prefix or target.
    pub text: String,
    /// ` @N`.
    pub target_column: Option<u32>,
    /// ` @new`.
    pub target_new_column: bool,
    /// Non-empty sections, in order.
    pub groups: Vec<ShellPaletteGroup>,
    /// A message for the field.
    pub notice: Option<String>,
}

/// The core's [`Shell::view`] as UniFFI records: field moves only, so `session()` costs what
/// the core view costs (`just bench-shell`, "session view").
fn view(shell: &Shell) -> ShellSession {
    let v = shell.view();
    let tab = |t: TabView| ShellTab {
        id: t.id,
        kind: t.kind,
        content_id: t.content_id,
        content_revision: t.content_revision,
        reference: t.reference,
        resource_title: t.resource_title,
        reference_label: t.reference_label,
        chapter_label: t.chapter_label,
        scroll_segment_id: t.scroll_segment_id,
        scroll_scalar: t.scroll_scalar,
        link_mode: t.link_mode,
        link_group: t.link_group,
        can_go_back: t.can_go_back,
        can_go_forward: t.can_go_forward,
        lexicon_word: t.lexicon_word,
        lexicon_language: t.lexicon_language,
        highlight_range: t.highlight_range,
    };
    ShellSession {
        workspaces: v
            .workspaces
            .into_iter()
            .map(|w| ShellWorkspace {
                id: w.id,
                title: w.title,
                columns: w
                    .columns
                    .into_iter()
                    .map(|c| ShellColumn {
                        id: c.id,
                        number: c.number,
                        tabs: c.tabs.into_iter().map(tab).collect(),
                        shown_tab_id: c.shown_tab_id,
                        width_fraction: c.width_fraction,
                    })
                    .collect(),
                active_column_number: w.active_column_number,
                notes_dock_note_id: w.notes_dock_note_id,
                notes_dock_open: w.notes_dock_open,
            })
            .collect(),
        active_workspace_id: v.active_workspace_id,
        tool: v.tool,
        effective_tool: v.effective_tool,
        tool_context: v.tool_context,
        revision: v.revision,
        undo_title: v.undo_title,
        redo_title: v.redo_title,
        reading: ShellReading {
            text_size_step: v.reading.text_size_step,
            line_spacing: v.reading.line_spacing,
            theme: v.reading.theme,
        },
        ink: ShellInk {
            width: v.ink.width,
            swatch: v.ink.swatch,
            alternate_swatch: v.ink.alternate_swatch,
        },
    }
}

pub(crate) struct State {
    pub(crate) shell: Shell,
    index: Option<(u64, PaletteIndex)>,
    text: Option<Box<dyn TextProvider + Send>>,
    /// B026: the morphology layers `use_morphology` attached.
    #[cfg(feature = "retrieval")]
    pub(crate) morphology: Option<openlogos_retrieval_sqlite::MorphologyLayers>,
}

/// A session with its undo history, owned by the core. One per window.
#[derive(uniffi::Object)]
pub struct ShellHandle {
    state: Mutex<State>,
}

impl ShellHandle {
    fn from_session(session: Session) -> Result<Arc<ShellHandle>, ShellCommandError> {
        let shell = Shell::new(session).map_err(invalid)?;
        Ok(Arc::new(ShellHandle {
            state: Mutex::new(State {
                shell,
                index: None,
                text: None,
                #[cfg(feature = "retrieval")]
                morphology: None,
            }),
        }))
    }

    pub(crate) fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        // A panic inside a call poisons nothing a later call could misread: every mutation
        // replaces the session whole.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(feature = "retrieval")]
#[uniffi::export]
impl ShellHandle {
    /// Answers the Text section from the search packs in `directory` (`search/` of a preview
    /// pack, B071): the shared tokenizer over each edition's FTS5 index, opened read-only.
    pub fn use_edition_search(&self, directory: String) -> Result<(), ShellCommandError> {
        let search = openlogos_retrieval_sqlite::EditionSearch::open_directory(
            std::path::Path::new(&directory),
        )
        .map_err(invalid)?;
        self.lock().text = Some(Box::new(search));
        Ok(())
    }
}

#[uniffi::export]
impl ShellHandle {
    /// A handle over a session in its in-memory JSON form (a session record with its
    /// workspaces inline, as `sessionJson` returns it).
    #[uniffi::constructor]
    pub fn new(session_json: String) -> Result<Arc<ShellHandle>, ShellCommandError> {
        let session: Session = serde_json::from_str(&session_json).map_err(invalid)?;
        ShellHandle::from_session(session)
    }

    /// A handle over the persisted records. Workspace records at 1.x are migrated to 2.0
    /// first (they are written back at 2.0 only when the host saves).
    #[uniffi::constructor]
    pub fn from_records(
        session_json: String,
        workspace_jsons: Vec<String>,
    ) -> Result<Arc<ShellHandle>, ShellCommandError> {
        let record: SessionRecord = serde_json::from_str(&session_json).map_err(invalid)?;
        let mut workspaces = Vec::with_capacity(workspace_jsons.len());
        for json in workspace_jsons {
            let value: serde_json::Value = serde_json::from_str(&json).map_err(invalid)?;
            let value = migrate_workspace_to_2(value).map_err(invalid)?;
            workspaces.push(serde_json::from_value(value).map_err(invalid)?);
        }
        ShellHandle::from_session(Session::from_records(record, workspaces).map_err(invalid)?)
    }

    /// The `shell-commands` conformance session with the fixture text provider: two
    /// workspaces over the fixture editions, for test hosts and previews.
    #[uniffi::constructor]
    pub fn fixture() -> Arc<ShellHandle> {
        let session = openlogos_conformance::fixture_session();
        let handle = ShellHandle::from_session(session).expect("the fixture session checks");
        {
            let mut state = handle.lock();
            state.text = Some(Box::new(FixtureTextProvider::new()));
            // Set directly, so a fresh fixture handle is still at revision 0.
            state.shell.session.resource_languages =
                openlogos_conformance::fixture_resource_languages();
        }
        handle
    }

    /// Answers the Text section from the conformance fixture texts (until B071's search).
    pub fn use_fixture_text(&self) {
        self.lock().text = Some(Box::new(FixtureTextProvider::new()));
    }

    /// Registers the short names resources are shown under (`bsb` → `BSB`); palette rows
    /// and tab views read them. A resource without one shows its id.
    pub fn set_resource_titles(&self, titles: std::collections::HashMap<String, String>) {
        self.lock()
            .shell
            .set_resource_titles(titles.into_iter().collect());
    }

    /// Registers resource languages as BCP 47 tags (`sblgnt` → `grc`, `wlc` → `hbo`); the
    /// tool well shows the original-language tools on Reading tabs of `grc`, `hbo`, `arc`,
    /// `he` or `heb` resources. A resource without one reads as a translation.
    pub fn set_resource_languages(&self, languages: std::collections::HashMap<String, String>) {
        self.lock()
            .shell
            .set_resource_languages(languages.into_iter().collect());
    }

    /// Registers resource versification schemes (`bsb` → `eng`, `wlc` → `org`; B078): linked
    /// tabs follow through them, and `positions` carries each follower's mapped reference.
    /// `ReadingHandle.schemes()` gives the map for the installed editions.
    pub fn set_resource_schemes(&self, schemes: std::collections::HashMap<String, String>) {
        self.lock()
            .shell
            .set_resource_schemes(schemes.into_iter().collect());
    }

    /// The tool well for what has focus: its context, the stored and effective tool, and
    /// the groups to draw. Call it on every render; it costs microseconds.
    pub fn tool_well(&self, context: ShellCommandContext) -> ShellToolWell {
        self.lock().shell.tool_well(&context.into()).into()
    }

    /// The context menu for what was right-clicked (`context.has_selection` for a tab,
    /// column or background menu). Apply `focus` first when set, then the chosen row with
    /// `apply_in` under the menu's `context`. A tab or column that does not exist fails as
    /// `InvalidDocument`.
    pub fn context_menu(
        &self,
        target: ShellContextTarget,
        context: ShellCommandContext,
    ) -> Result<ShellContextMenu, ShellCommandError> {
        self.lock()
            .shell
            .context_menu(&target.into(), &context.into())
            .map(Into::into)
            .map_err(|reason| invalid(format!("context menu target: {reason}")))
    }

    /// `commands` with where focus is, so tool and note commands carry their context's
    /// reasons.
    pub fn commands_in(&self, context: ShellCommandContext) -> Vec<ShellCommandInfo> {
        let state = self.lock();
        state
            .shell
            .commands(&context.into())
            .into_iter()
            .map(|c| command_info(&c.spec, c.disabled))
            .collect()
    }

    /// `apply` with where focus is: a tool or note action outside its context fails as
    /// `Disabled` with kind `wrong_context`.
    pub fn apply_in(
        &self,
        command_id: String,
        args: ShellCommandArgs,
        context: ShellCommandContext,
    ) -> Result<ShellOutcome, ShellCommandError> {
        let mut state = self.lock();
        let outcome = state
            .shell
            .apply(&Invocation::with(&command_id, args.into()), &context.into())?;
        Ok(outcome.into())
    }

    /// The session as a host renders it.
    pub fn session(&self) -> ShellSession {
        view(&self.lock().shell)
    }

    /// The session in its in-memory JSON form.
    pub fn session_json(&self) -> String {
        serde_json::to_string(&self.lock().shell.session).unwrap_or_default()
    }

    /// The records to persist.
    pub fn records(&self) -> ShellRecords {
        let (record, workspaces) = self.lock().shell.session.to_records();
        ShellRecords {
            session_json: serde_json::to_string(&record).unwrap_or_default(),
            workspace_jsons: workspaces
                .iter()
                .map(|w| serde_json::to_string(w).unwrap_or_default())
                .collect(),
        }
    }

    /// Every registered command with whether it can run now (without arguments).
    pub fn commands(&self, has_selection: bool) -> Vec<ShellCommandInfo> {
        let state = self.lock();
        state
            .shell
            .commands(&CommandContext {
                has_selection,
                ..CommandContext::default()
            })
            .into_iter()
            .map(|c| command_info(&c.spec, c.disabled))
            .collect()
    }

    /// Runs a command. `handled == false` means the host performs it.
    pub fn apply(
        &self,
        command_id: String,
        args: ShellCommandArgs,
        has_selection: bool,
    ) -> Result<ShellOutcome, ShellCommandError> {
        let mut state = self.lock();
        let outcome = state.shell.apply(
            &Invocation::with(&command_id, args.into()),
            &CommandContext {
                has_selection,
                ..CommandContext::default()
            },
        )?;
        Ok(outcome.into())
    }

    /// Takes back the last undoable command.
    pub fn undo(&self) -> Result<ShellOutcome, ShellCommandError> {
        Ok(self.lock().shell.undo()?.into())
    }

    /// Runs the last undone command again.
    pub fn redo(&self) -> Result<ShellOutcome, ShellCommandError> {
        Ok(self.lock().shell.redo()?.into())
    }

    /// The palette's answer for the field's text; cheap to call on every keystroke.
    pub fn palette_query(&self, text: String, context: ShellPaletteContext) -> ShellPaletteResult {
        let mut state = self.lock();
        let revision = state.shell.revision;
        if state.index.as_ref().is_none_or(|(r, _)| *r != revision) {
            state.index = Some((revision, PaletteIndex::new(&state.shell.session)));
        }
        let state = &*state;
        let index = &state.index.as_ref().expect("built above").1;
        let context = PaletteContext {
            has_selection: context.has_selection,
            platform: match context.platform {
                ShellPlatform::Web => Platform::Web,
                ShellPlatform::Mac => Platform::Mac,
                ShellPlatform::Ipad => Platform::Ipad,
            },
            expanded: context.expanded.map(Into::into),
            focus: context.focus.map_or(Focus::Tab, Into::into),
        };
        let provider = state.text.as_ref().map(|p| &**p as &dyn TextProvider);
        let result = palette_query_indexed(&state.shell, index, &text, &context, provider);
        ShellPaletteResult {
            mode: match result.query.mode {
                PaletteMode::All => "all",
                PaletteMode::Commands => "commands",
                PaletteMode::Tabs => "tabs",
                PaletteMode::Text => "text",
                PaletteMode::Passage => "passage",
            }
            .to_owned(),
            text: result.query.text,
            target_column: match result.query.target {
                Some(ColumnTarget::Column { number }) => Some(number),
                _ => None,
            },
            target_new_column: result.query.target == Some(ColumnTarget::New),
            groups: result
                .groups
                .into_iter()
                .map(|g| ShellPaletteGroup {
                    section: g.section.into(),
                    items: g.items.into_iter().map(Into::into).collect(),
                    total: g.total,
                })
                .collect(),
            notice: result.notice,
        }
    }
}

/// Every registered command, without state: what a host binds its menus and shortcuts to.
#[uniffi::export]
#[must_use]
pub fn shell_registry() -> Vec<ShellCommandInfo> {
    registry()
        .iter()
        .map(|spec| command_info(spec, None))
        .collect()
}

/// Migrates a workspace document at any 1.x version to 2.0 (a 2.0 document is returned as
/// is).
#[uniffi::export]
pub fn migrate_workspace_json(json: String) -> Result<String, ShellCommandError> {
    let value: serde_json::Value = serde_json::from_str(&json).map_err(invalid)?;
    let migrated = migrate_workspace_to_2(value).map_err(invalid)?;
    serde_json::to_string(&migrated).map_err(invalid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_session_through_the_handle() {
        let shell = ShellHandle::fixture();
        let before = shell.session();
        assert_eq!(before.workspaces.len(), 2);
        assert_eq!(before.workspaces[0].columns[0].number, 1);
        let outcome = shell
            .apply("column.new".into(), ShellCommandArgs::default(), false)
            .unwrap();
        assert!(outcome.handled && outcome.recorded);
        assert_eq!(shell.session().workspaces[0].columns.len(), 4);
        assert_eq!(shell.session().undo_title.as_deref(), Some("New column"));
        shell.undo().unwrap();
        assert_eq!(shell.session().workspaces[0].columns.len(), 3);
        shell.redo().unwrap();
        let error = shell
            .apply(
                "column.focus".into(),
                ShellCommandArgs {
                    column: Some(9),
                    ..Default::default()
                },
                false,
            )
            .unwrap_err();
        assert!(
            matches!(error, ShellCommandError::Disabled { ref kind, .. } if kind == "no_such_column")
        );
        let host = shell
            .apply("selection.study".into(), ShellCommandArgs::default(), true)
            .unwrap();
        assert!(!host.handled);

        let context = ShellPaletteContext {
            has_selection: false,
            platform: ShellPlatform::Mac,
            expanded: None,
            focus: None,
        };
        let result = shell.palette_query("Jn 3:16 @2".into(), context);
        assert_eq!(result.target_column, Some(2));
        let first = &result.groups[0].items[0];
        assert_eq!(first.section, ShellPaletteSection::Passage);
        assert_eq!(first.command_id, "nav.navigate");
        let navigated = shell
            .apply(first.command_id.clone(), first.args.clone(), false)
            .unwrap();
        assert!(navigated.handled);
        let text = shell.palette_query("#loved".into(), context);
        assert_eq!(text.groups[0].section, ShellPaletteSection::Text);

        let records = shell.records();
        let reopened =
            ShellHandle::from_records(records.session_json, records.workspace_jsons).unwrap();
        assert_eq!(reopened.session().workspaces, shell.session().workspaces);
        assert_eq!(shell_registry().len(), registry().len());
        shell
            .apply(
                "view.line_spacing".into(),
                ShellCommandArgs {
                    number: Some(1.7),
                    ..Default::default()
                },
                false,
            )
            .unwrap();
        assert_eq!(shell.session().reading.line_spacing, 1.7);
        let revision = shell.session().revision;
        shell
            .apply(
                "tab.record_scroll".into(),
                ShellCommandArgs {
                    tab_id: Some("t3".into()),
                    segment_id: Some("John.3.16".into()),
                    scalar: Some(4),
                    ..Default::default()
                },
                false,
            )
            .unwrap();
        assert_eq!(shell.session().revision, revision);
        assert_eq!(layout_constants().measure_cap_standard_pt, 640.0);
        shell.set_resource_titles([("bsb".to_owned(), "BSB".to_owned())].into());
        let tab = &shell.session().workspaces[0].columns[0].tabs[0];
        assert_eq!(tab.resource_title, "BSB");
        assert_eq!(tab.reference_label.as_deref(), Some("Jn 3:16"));
        assert_eq!(tab.chapter_label.as_deref(), Some("Jn 3"));
        let rows = shell.palette_query("@bsb jn".into(), context);
        assert_eq!(rows.groups[0].items[0].title, "BSB · Jn 3:16");
        assert_eq!(
            crate::native::format_short("John 3".into()).unwrap(),
            "Jn 3"
        );
        assert!(
            shell
                .commands(false)
                .iter()
                .any(|c| c.id == "edit.undo" && c.disabled_kind.is_none())
        );
    }

    #[test]
    fn the_tool_well_follows_focus_through_the_handle() {
        let shell = ShellHandle::fixture();
        let tab = ShellCommandContext {
            has_selection: false,
            focus: ShellFocus::Tab,
        };
        let well = shell.tool_well(tab);
        assert_eq!(well.context, "translation");
        assert_eq!(well.groups[0].items[0].icon, "select");
        assert!(well.groups[0].items[0].active);
        assert_eq!(well.groups[1].kind, "ink");
        shell
            .apply_in("tool.pen".into(), ShellCommandArgs::default(), tab)
            .unwrap();
        let dock = ShellCommandContext {
            has_selection: true,
            focus: ShellFocus::NotesDock,
        };
        let notes = shell.tool_well(dock);
        assert_eq!(
            (
                notes.context.as_str(),
                notes.tool.as_str(),
                notes.effective_tool.as_str()
            ),
            ("notes", "pen", "type")
        );
        assert_eq!(notes.groups.last().unwrap().kind, "selection");
        let error = shell
            .apply_in("tool.eraser".into(), ShellCommandArgs::default(), dock)
            .unwrap_err();
        assert!(matches!(
            error,
            ShellCommandError::Disabled { ref kind, ref reason, .. }
                if kind == "wrong_context" && reason == "Eraser works on Bible text"
        ));
        assert!(
            shell
                .commands_in(dock)
                .iter()
                .any(|c| c.id == "note.bold" && c.disabled_kind.is_none())
        );
        shell
            .apply(
                "column.focus".into(),
                ShellCommandArgs {
                    column: Some(2),
                    ..Default::default()
                },
                false,
            )
            .unwrap();
        let session = shell.session();
        assert_eq!(
            (
                session.tool_context.as_str(),
                session.effective_tool.as_str()
            ),
            ("original", "pen"),
            "the fixture registers sblgnt as grc"
        );
        let rows = shell.palette_query(
            ":Jn 3:16".into(),
            ShellPaletteContext {
                has_selection: false,
                platform: ShellPlatform::Mac,
                expanded: None,
                focus: Some(ShellFocus::NotesDock),
            },
        );
        assert_eq!(rows.groups[0].items[1].command_id, "note.link_passage");
        let opened = shell
            .apply(
                "tab.open_lexicon".into(),
                ShellCommandArgs {
                    text: Some("ἀγάπη".into()),
                    ..Default::default()
                },
                false,
            )
            .unwrap();
        assert!(opened.recorded);
        let session = shell.session();
        let column = &session.workspaces[0].columns[2];
        assert_eq!(column.tabs[0].lexicon_word.as_deref(), Some("ἀγάπη"));
        assert_eq!(
            session.workspaces[0].active_column_number, 2,
            "focus stays put"
        );
        assert_eq!(column.shown_tab_id, column.tabs[0].id);
    }

    #[test]
    fn the_context_menu_comes_through_the_handle() {
        let shell = ShellHandle::fixture();
        let none = ShellCommandContext {
            has_selection: false,
            focus: ShellFocus::Tab,
        };
        let menu = shell
            .context_menu(
                ShellContextTarget::Word {
                    text: "ἠγάπησεν".into(),
                    reference: Some("John.3.16".into()),
                    segment_id: None,
                    scalar_start: None,
                    scalar_end: None,
                    resource_id: None,
                    tab_id: Some("t3".into()),
                },
                none,
            )
            .unwrap();
        assert_eq!(menu.target_kind, "word");
        assert!(menu.context.has_selection);
        let focus = menu.focus.clone().unwrap();
        assert_eq!(
            (focus.command_id.as_str(), focus.args.tab_id.as_deref()),
            ("tab.focus", Some("t3"))
        );
        let rows: Vec<&ShellContextMenuItem> =
            menu.sections.iter().flat_map(|s| s.items.iter()).collect();
        let study = rows.iter().find(|i| i.label == "Word study").unwrap();
        assert_eq!(study.disabled_text, None);
        let compare = rows.iter().find(|i| i.label == "Compare in bsb").unwrap();
        assert_eq!(compare.args.resource_id.as_deref(), Some("bsb"));
        let highlight = rows.iter().find(|i| i.label == "Highlight").unwrap();
        assert_eq!(highlight.submenu[0].value.as_deref(), Some("#F5D90A"));
        assert!(highlight.submenu[0].active);
        shell
            .apply_in(focus.command_id, focus.args, menu.context)
            .unwrap();
        let opened = shell
            .apply_in(
                compare.command_id.clone(),
                compare.args.clone(),
                menu.context,
            )
            .unwrap();
        assert!(opened.recorded);
        let session = shell.session();
        assert_eq!(session.workspaces[0].active_column_number, 2);
        let column = &session.workspaces[0].columns[2];
        assert_eq!(column.tabs.last().unwrap().content_id, "bsb");

        let tab = shell
            .context_menu(
                ShellContextTarget::Tab {
                    tab_id: "t2".into(),
                },
                none,
            )
            .unwrap();
        let pin = tab.sections[2].items[0].clone();
        assert_eq!(
            (pin.label.as_str(), pin.active, pin.disabled_text.as_deref()),
            ("Pin", true, Some("The tab is already pinned"))
        );
        assert!(matches!(
            shell.context_menu(ShellContextTarget::Tab { tab_id: "t9".into() }, none),
            Err(ShellCommandError::InvalidDocument { ref detail }) if detail.contains("That tab is not open")
        ));
    }

    #[test]
    fn one_point_x_workspaces_open_through_the_records() {
        let one = include_str!("../../../../examples/archive/workspace-1.0.json");
        let migrated = migrate_workspace_json(one.into()).unwrap();
        assert!(migrated.contains("\"schema_version\":\"2.0\""));
        let record = r#"{"schema_version":"1.0","fixture_only":true,"workspace_ids":["fixture:workspace:1"],"active_workspace_id":"fixture:workspace:1","tool":"select","next_serial":1}"#;
        let handle = ShellHandle::from_records(record.into(), vec![one.into()]).unwrap();
        assert_eq!(handle.session().workspaces[0].columns.len(), 3);
        assert!(matches!(
            ShellHandle::new("{}".into()),
            Err(ShellCommandError::InvalidDocument { .. })
        ));
    }
}

#[cfg(all(test, feature = "retrieval"))]
mod edition_search_tests {
    use super::*;
    use openlogos_retrieval::VerseId;
    use openlogos_retrieval_sqlite::{PackBuilder, PackManifest};

    /// A hand-written search pack, not corpus text.
    fn search_directory() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("openlogos-ffi-search-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = PackManifest {
            edition: "bsb".to_owned(),
            scheme: "eng".to_owned(),
            source: "test".to_owned(),
            license: "test".to_owned(),
            attribution: "test".to_owned(),
            work: None,
            locator_scheme: None,
        };
        let mut builder = PackBuilder::create(&dir.join("bsb.sqlite"), &manifest, &[]).unwrap();
        builder
            .add_verse(VerseId::new(43, 1, 2), "A sentence with a zebra in it.")
            .unwrap();
        builder.finish().unwrap();
        dir
    }

    fn context() -> ShellPaletteContext {
        ShellPaletteContext {
            has_selection: false,
            platform: ShellPlatform::Mac,
            expanded: None,
            focus: None,
        }
    }

    #[test]
    fn the_shell_answers_text_from_the_search_packs_after_use_edition_search() {
        let handle = ShellHandle::fixture();
        let before = handle.palette_query("zebra".to_owned(), context());
        assert!(format!("{before:?}").find("Lk 1:2").is_none());
        handle
            .use_edition_search(search_directory().display().to_string())
            .unwrap();
        let after = handle.palette_query("zebr".to_owned(), context());
        let shown = format!("{after:?}");
        assert!(shown.contains("A sentence with a zebra in it."), "{shown}");
    }

    #[test]
    fn a_directory_without_search_packs_is_an_error_and_keeps_the_provider() {
        let handle = ShellHandle::fixture();
        let empty = std::env::temp_dir().join("openlogos-ffi-search-none");
        assert!(
            handle
                .use_edition_search(empty.display().to_string())
                .is_err()
        );
        let answer = handle.palette_query("loved".to_owned(), context());
        assert!(format!("{answer:?}").contains("loved"));
    }
}
