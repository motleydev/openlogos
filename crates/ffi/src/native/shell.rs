//! The workspace shell over `crates/workspaces` (B070, specs/22): a [`ShellHandle`] object
//! holding a session with its undo and redo stacks, the command registry, `apply`, `undo`,
//! `redo` and the palette query. Additive; nothing existing is renamed.
//!
//! The handle keeps the palette's [`PaletteIndex`] keyed on the shell revision, so a host can
//! call `paletteQuery` on every keystroke and pay for indexing only after a command changed
//! the session. Session state crosses as read-only view records; persistence crosses as the
//! schema JSON of the session record and each workspace record.

use std::sync::{Arc, Mutex};

use openlogos_conformance::FixtureTextProvider;
use openlogos_workspaces::commands::{
    ArgumentShape, CommandArgs, CommandContext, CommandError, CommandSpec, DisabledReason,
    Invocation, Outcome, Platform, Scope, Shell, registry,
};
use openlogos_workspaces::migrate_workspace_to_2;
use openlogos_workspaces::palette::{
    ColumnTarget, PaletteContext, PaletteIndex, PaletteItem, PaletteMode, PaletteSection,
    TextProvider, palette_query_indexed,
};
use openlogos_workspaces::shell::{LinkMode, Session, SessionRecord, Workspace};

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

fn invalid(detail: impl ToString) -> ShellCommandError {
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
    /// The passage in short form for the tab label (`Jn 3:16`), if it parses.
    pub reference_label: Option<String>,
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
    /// The selected tool (`select`, `highlight`, `pen`, `highlighter_pen`, `eraser`, `collect`).
    pub tool: String,
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
    /// Tabs that followed a navigation, to remap through the host's reference mapping.
    pub followed_tab_ids: Vec<String>,
    /// The revision after it.
    pub revision: u64,
}

impl From<Outcome> for ShellOutcome {
    fn from(o: Outcome) -> Self {
        ShellOutcome {
            command_id: o.command_id,
            handled: o.handled,
            recorded: o.recorded,
            followed_tab_ids: o.effects.followed_tab_ids,
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

fn view(shell: &Shell) -> ShellSession {
    let english = openlogos_core::reference::AliasTable::english();
    let tab_view = |t: &openlogos_workspaces::shell::Tab| {
        let (link_mode, link_group) = match t.link_mode() {
            LinkMode::Pinned => ("pinned", None),
            LinkMode::Follows => ("follows", None),
            LinkMode::Group { group } => ("group", Some(group)),
        };
        ShellTab {
            id: t.id.clone(),
            kind: name_of(&t.kind),
            content_id: t.content_id.clone(),
            content_revision: t.content_revision.clone(),
            reference: t.reference.clone(),
            resource_title: shell.session.resource_title(&t.content_id).to_owned(),
            reference_label: t
                .reference
                .as_deref()
                .and_then(|r| openlogos_core::reference::parse(r, &english).ok())
                .map(|set| set.to_short()),
            scroll_segment_id: t.scroll_anchor.as_ref().map(|a| a.segment_id.clone()),
            scroll_scalar: t.scroll_anchor.as_ref().map(|a| a.scalar),
            link_mode: link_mode.to_owned(),
            link_group,
            can_go_back: !t.history.back.is_empty(),
            can_go_forward: !t.history.forward.is_empty(),
        }
    };
    let workspace_view = |w: &Workspace| ShellWorkspace {
        id: w.id.clone(),
        title: w.title.clone(),
        columns: w
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| ShellColumn {
                id: c.id.clone(),
                number: i as u32 + 1,
                tabs: c.tabs.iter().map(tab_view).collect(),
                shown_tab_id: c.shown_tab_id.clone(),
                width_fraction: c.width_fraction,
            })
            .collect(),
        active_column_number: w.active_index() as u32 + 1,
        notes_dock_note_id: w.notes_dock.note_id.clone(),
        notes_dock_open: w.notes_dock.open,
    };
    ShellSession {
        workspaces: shell
            .session
            .workspaces
            .iter()
            .map(workspace_view)
            .collect(),
        active_workspace_id: shell.session.active_workspace_id.clone(),
        tool: shell.session.tool.name().to_owned(),
        revision: shell.revision,
        undo_title: shell.undo_stack.last().map(|r| r.title.clone()),
        redo_title: shell.redo_stack.last().map(|r| r.title.clone()),
        reading: ShellReading {
            text_size_step: shell.session.reading.text_size_step,
            line_spacing: shell.session.reading.line_spacing,
            theme: shell.session.reading.theme.clone(),
        },
        ink: ShellInk {
            width: shell.session.ink.width,
            swatch: shell.session.ink.swatch.clone(),
            alternate_swatch: shell.session.ink.alternate_swatch.clone(),
        },
    }
}

struct State {
    shell: Shell,
    index: Option<(u64, PaletteIndex)>,
    text: Option<FixtureTextProvider>,
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
            }),
        }))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        // A panic inside a call poisons nothing a later call could misread: every mutation
        // replaces the session whole.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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
        handle.lock().text = Some(FixtureTextProvider::new());
        handle
    }

    /// Answers the Text section from the conformance fixture texts (until B071's search).
    pub fn use_fixture_text(&self) {
        self.lock().text = Some(FixtureTextProvider::new());
    }

    /// Registers the short names resources are shown under (`bsb` → `BSB`); palette rows
    /// and tab views read them. A resource without one shows its id.
    pub fn set_resource_titles(&self, titles: std::collections::HashMap<String, String>) {
        self.lock()
            .shell
            .set_resource_titles(titles.into_iter().collect());
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
            .commands(&CommandContext { has_selection })
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
            &CommandContext { has_selection },
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
        };
        let provider = state.text.as_ref().map(|p| p as &dyn TextProvider);
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
