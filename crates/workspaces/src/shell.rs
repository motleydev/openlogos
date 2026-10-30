//! The workspace shell of specs/22-workspace-shell.md §2 (B070): a [`Session`] of parallel
//! [`Workspace`]s, each an ordered row of numbered [`Column`]s, each a stack of [`Tab`]s with
//! one shown. A tab is what specs/21 called a pane: one resource at one passage with its own
//! scroll anchor, link state and history ([`PaneHistory`], reused from [`crate::navigation`]).
//!
//! Column numbers are positions, never stored: column `n` is `columns[n - 1]`, so moving a
//! column renumbers it. Ids are opaque tokens with the shape of [`crate::is_pane_id`], minted
//! from a per-record serial and never reused inside the record that minted them.
//!
//! Persistence is two records (ADR 0003): a workspace at schema 2.0
//! (schemas/workspace-2.schema.json), one per workspace so each can be shared or exported
//! alone, and a session at schema 1.0 (schemas/session.schema.json) holding the workspace
//! order, the active workspace and the current tool. [`Session`] is the in-memory union of
//! the two; [`Session::to_records`] and [`Session::from_records`] split and join it.
//! [`Workspace::from_v1`] is the lossless 1.x → 2.0 migration and [`Workspace::to_v1`] the
//! export back to a 1.1 reader.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::navigation::{PaneHistory, is_pane_id};
use crate::{
    DeviceClass, LayoutError, LayoutProfile, LayoutProfiles, Pane, PaneKind, ScrollAnchor,
};

/// The workspace schema version this module reads and writes.
pub const WORKSPACE_SCHEMA_VERSION: &str = "2.0";

/// The session schema version this module reads and writes.
pub const SESSION_SCHEMA_VERSION: &str = "1.0";

/// Closed tabs a workspace remembers for "reopen closed tab".
pub const CLOSED_TABS_KEPT: usize = 20;

/// One tab: specs/21's pane under its new name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tab {
    /// Stable id within the workspace.
    pub id: String,
    /// What the tab shows.
    pub kind: PaneKind,
    /// The resource (edition, work, note or thread id).
    pub content_id: String,
    /// The resource revision the tab was saved against.
    pub content_revision: String,
    /// The local passage in the resource's own versification, when it has one.
    pub reference: Option<String>,
    /// Scroll position; `None` means the top of the passage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll_anchor: Option<ScrollAnchor>,
    /// The reference-linked group, or `None` when pinned or following.
    pub link_group: Option<String>,
    /// Independent of every other tab's navigation.
    pub pinned: bool,
    /// The tab's own back and forward stacks.
    #[serde(default, skip_serializing_if = "PaneHistory::is_empty")]
    pub history: PaneHistory,
}

impl Tab {
    /// The tab's navigation mode in specs/21 §2's words.
    #[must_use]
    pub fn link_mode(&self) -> LinkMode {
        match (&self.link_group, self.pinned) {
            (_, true) => LinkMode::Pinned,
            (Some(group), false) => LinkMode::Group {
                group: group.clone(),
            },
            (None, false) => LinkMode::Follows,
        }
    }
}

/// A tab's navigation mode (specs/21 §2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LinkMode {
    /// Independent.
    Pinned,
    /// A member of a reference-linked group.
    Group {
        /// The group label.
        group: String,
    },
    /// Follows the active tab's passage without driving.
    Follows,
}

/// One column: a stack of tabs with one shown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    /// Stable id within the workspace; the column's number is its position.
    pub id: String,
    /// The tabs, in order; never empty.
    pub tabs: Vec<Tab>,
    /// The tab shown.
    pub shown_tab_id: String,
    /// Width share; a workspace's columns sum to one.
    pub width_fraction: f64,
}

impl Column {
    /// The shown tab.
    #[must_use]
    pub fn shown(&self) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == self.shown_tab_id)
    }

    /// Position of the shown tab.
    #[must_use]
    pub fn shown_index(&self) -> usize {
        self.tabs
            .iter()
            .position(|t| t.id == self.shown_tab_id)
            .unwrap_or(0)
    }
}

/// The notes dock of specs/21 §4.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotesDock {
    /// The note shown, if any.
    pub note_id: Option<String>,
    /// Whether the dock is open.
    pub open: bool,
}

/// Which columns one device class shows (specs/22 §2: profiles keep their meaning per
/// workspace). Columns the profile does not name stay reachable by number; nothing is closed
/// by a narrower window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnProfile {
    /// Columns shown on this class, in visual order.
    pub visible_column_ids: Vec<String>,
    /// Width fractions for the shown columns; empty means the columns' own widths.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub widths: BTreeMap<String, f64>,
    /// Whether the notes dock is open on this class.
    pub notes_dock_open: bool,
}

/// One optional column profile per device class.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ColumnProfiles {
    /// Under 700 points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact: Option<ColumnProfile>,
    /// 700–1399 points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regular: Option<ColumnProfile>,
    /// 1400–2399 points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wide: Option<ColumnProfile>,
    /// 2400 points and above.
    #[serde(
        default,
        rename = "ultra-wide",
        skip_serializing_if = "Option::is_none"
    )]
    pub ultra_wide: Option<ColumnProfile>,
}

impl ColumnProfiles {
    /// Whether no class has a profile.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        DeviceClass::ALL.iter().all(|c| self.get(*c).is_none())
    }

    /// The stored profile for a class, if any.
    #[must_use]
    pub fn get(&self, class: DeviceClass) -> Option<&ColumnProfile> {
        match class {
            DeviceClass::Compact => self.compact.as_ref(),
            DeviceClass::Regular => self.regular.as_ref(),
            DeviceClass::Wide => self.wide.as_ref(),
            DeviceClass::UltraWide => self.ultra_wide.as_ref(),
        }
    }

    fn slot(&mut self, class: DeviceClass) -> &mut Option<ColumnProfile> {
        match class {
            DeviceClass::Compact => &mut self.compact,
            DeviceClass::Regular => &mut self.regular,
            DeviceClass::Wide => &mut self.wide,
            DeviceClass::UltraWide => &mut self.ultra_wide,
        }
    }

    /// Replaces one class's profile.
    pub fn set(&mut self, class: DeviceClass, profile: ColumnProfile) {
        *self.slot(class) = Some(profile);
    }

    /// Forgets a closed column in every profile.
    pub fn forget_column(&mut self, column_id: &str) {
        for class in DeviceClass::ALL {
            if let Some(profile) = self.slot(class).as_mut() {
                profile.visible_column_ids.retain(|c| c != column_id);
                profile.widths.remove(column_id);
            }
        }
    }
}

/// A tab that was closed, kept so it can be reopened where it was.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClosedTab {
    /// The tab, with its id, passage, anchor, link state and history.
    pub tab: Tab,
    /// The column it was in.
    pub column_id: String,
    /// Its position in that column.
    pub index: usize,
}

/// A workspace at schema 2.0: numbered columns of tabs, an active column and the notes dock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    /// `"2.0"`.
    pub schema_version: String,
    /// Synthetic data, never a user's workspace.
    pub fixture_only: bool,
    /// Stable workspace id.
    pub id: String,
    /// Display title.
    pub title: String,
    /// Columns 1…n, leading to trailing; never empty.
    pub columns: Vec<Column>,
    /// The active column.
    pub active_column_id: String,
    /// The notes dock.
    pub notes_dock: NotesDock,
    /// Per-class column profiles; absent classes show every column that fits.
    #[serde(default, skip_serializing_if = "ColumnProfiles::is_empty")]
    pub layout_profiles: ColumnProfiles,
    /// The next serial [`Workspace::mint_id`] tries, so an id is never minted twice.
    pub next_serial: u64,
    /// Recently closed tabs, most recent last. Runtime state, not persisted.
    #[serde(skip)]
    pub closed_tabs: Vec<ClosedTab>,
}

/// The tool the floating tool palette has selected (specs/22 §4).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    /// Native text selection.
    #[default]
    Select,
    /// A drag highlights with the current swatch.
    Highlight,
    /// Ink in the margin layer.
    Pen,
    /// Ink with the highlighter brush.
    HighlighterPen,
    /// Removes whole strokes.
    Eraser,
    /// A drag collects into the open collection.
    Collect,
}

impl Tool {
    /// Every tool, in palette order.
    pub const ALL: [Tool; 6] = [
        Tool::Select,
        Tool::Highlight,
        Tool::Pen,
        Tool::HighlighterPen,
        Tool::Eraser,
        Tool::Collect,
    ];

    /// The tool's name as serialized.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "select",
            Tool::Highlight => "highlight",
            Tool::Pen => "pen",
            Tool::HighlighterPen => "highlighter_pen",
            Tool::Eraser => "eraser",
            Tool::Collect => "collect",
        }
    }
}

/// Text size steps a reader can take below and above the default.
pub const TEXT_SIZE_STEPS: std::ops::RangeInclusive<i32> = -3..=5;

/// The line spacings offered (B072): compact, default and open.
pub const LINE_SPACINGS: [f64; 3] = [1.35, 1.5, 1.7];

/// The themes a host offers; `system` follows the platform's appearance.
pub const THEMES: [&str; 4] = ["system", "light", "dark", "sepia"];

/// Ink brush widths in coordinate units, smallest first; the pen default is 4 and the
/// highlighter's 12 (crates/ink brushes).
pub const INK_WIDTHS: [f64; 7] = [1.0, 2.0, 4.0, 6.0, 8.0, 12.0, 16.0];

/// How the reader has set up the text: one place for every host to read and the commands
/// `view.text_size_*`, `view.line_spacing*` and `view.theme` to change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReadingSettings {
    /// Steps from the platform's default text size, within [`TEXT_SIZE_STEPS`].
    pub text_size_step: i32,
    /// Line height as a multiple of the font size, one of [`LINE_SPACINGS`].
    pub line_spacing: f64,
    /// One of [`THEMES`].
    pub theme: String,
}

impl Default for ReadingSettings {
    fn default() -> Self {
        ReadingSettings {
            text_size_step: 0,
            line_spacing: 1.5,
            theme: "system".to_owned(),
        }
    }
}

/// The tool palette's wells and brush width (specs/22 §4), kept with the tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InkSettings {
    /// Brush width, one of [`INK_WIDTHS`].
    pub width: f64,
    /// The current swatch, `#RRGGBB`.
    pub swatch: String,
    /// The alternate swatch `X` swaps in.
    pub alternate_swatch: String,
}

impl Default for InkSettings {
    fn default() -> Self {
        InkSettings {
            width: 4.0,
            swatch: "#FFE066".to_owned(),
            alternate_swatch: "#7FD1FF".to_owned(),
        }
    }
}

/// Whether a string is a `#RRGGBB` swatch.
#[must_use]
pub fn is_swatch(text: &str) -> bool {
    text.len() == 7 && text.starts_with('#') && text[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// The session record as stored (schemas/session.schema.json 1.0): workspace order and the
/// active workspace; each workspace is its own record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionRecord {
    /// `"1.0"`.
    pub schema_version: String,
    /// Synthetic data, never a user's session.
    pub fixture_only: bool,
    /// Workspace ids in order.
    pub workspace_ids: Vec<String>,
    /// The active workspace.
    pub active_workspace_id: String,
    /// The selected tool.
    #[serde(default)]
    pub tool: Tool,
    /// The next serial for workspace ids.
    pub next_serial: u64,
    /// Text size, line spacing and theme.
    #[serde(default)]
    pub reading: ReadingSettings,
    /// Brush width and swatches.
    #[serde(default)]
    pub ink: InkSettings,
}

/// Every open workspace, in order, with the active one and the tool: the in-memory union of
/// a [`SessionRecord`] and its workspaces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    /// `"1.0"`, the session record's version.
    pub schema_version: String,
    /// Synthetic data.
    pub fixture_only: bool,
    /// The workspaces, in order; never empty.
    pub workspaces: Vec<Workspace>,
    /// The active workspace.
    pub active_workspace_id: String,
    /// The selected tool.
    #[serde(default)]
    pub tool: Tool,
    /// The next serial for workspace ids.
    pub next_serial: u64,
    /// Text size, line spacing and theme.
    #[serde(default)]
    pub reading: ReadingSettings,
    /// Brush width and swatches.
    #[serde(default)]
    pub ink: InkSettings,
}

/// Why a shell document is rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ShellError {
    /// A version this module does not read.
    #[error("unsupported {record} schema version {version}")]
    UnsupportedVersion {
        /// `workspace` or `session`.
        record: String,
        /// The version found.
        version: String,
    },
    /// A session without workspaces, a workspace without columns or a column without tabs.
    #[error("{what} is empty")]
    Empty {
        /// What is empty.
        what: String,
    },
    /// An id without the shape of a pane id.
    #[error("`{id}` is not a valid id")]
    BadId {
        /// The id.
        id: String,
    },
    /// Two things of one kind share an id.
    #[error("duplicate id {id}")]
    DuplicateId {
        /// The id.
        id: String,
    },
    /// A reference to an id that does not exist.
    #[error("{what} {id} does not exist")]
    Dangling {
        /// What kind of id.
        what: String,
        /// The id.
        id: String,
    },
    /// A pinned tab that also names a link group.
    #[error("tab {tab_id} is pinned and in link group {group}")]
    PinnedInGroup {
        /// The tab.
        tab_id: String,
        /// The group.
        group: String,
    },
    /// Column widths that are not positive or do not sum to one.
    #[error("column widths of workspace {workspace_id} do not sum to one")]
    BadWidths {
        /// The workspace.
        workspace_id: String,
    },
    /// The 1.x workspace was malformed.
    #[error("1.x workspace: {detail}")]
    Layout {
        /// What [`LayoutError`] said.
        detail: String,
    },
    /// The 2.0 workspace cannot be expressed in 1.1 (more columns than a class allows, or a
    /// column whose tabs are not one link group).
    #[error("cannot export to 1.1: {detail}")]
    NotExpressible {
        /// Why.
        detail: String,
    },
}

impl From<LayoutError> for ShellError {
    fn from(error: LayoutError) -> Self {
        ShellError::Layout {
            detail: error.to_string(),
        }
    }
}

fn check_id(id: &str) -> Result<(), ShellError> {
    if is_pane_id(id) {
        Ok(())
    } else {
        Err(ShellError::BadId { id: id.to_owned() })
    }
}

impl Workspace {
    /// Column `number` (1-based), if it exists.
    #[must_use]
    pub fn column(&self, number: usize) -> Option<&Column> {
        number.checked_sub(1).and_then(|i| self.columns.get(i))
    }

    /// The number of the column with this id.
    #[must_use]
    pub fn column_number(&self, column_id: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|c| c.id == column_id)
            .map(|i| i + 1)
    }

    /// The active column's position (0-based).
    #[must_use]
    pub fn active_index(&self) -> usize {
        self.columns
            .iter()
            .position(|c| c.id == self.active_column_id)
            .unwrap_or(0)
    }

    /// The active column.
    #[must_use]
    pub fn active_column(&self) -> &Column {
        &self.columns[self.active_index()]
    }

    /// The active tab: the active column's shown tab.
    #[must_use]
    pub fn active_tab(&self) -> &Tab {
        let column = self.active_column();
        &column.tabs[column.shown_index()]
    }

    /// Every tab, column by column.
    pub fn tabs(&self) -> impl Iterator<Item = &Tab> {
        self.columns.iter().flat_map(|c| c.tabs.iter())
    }

    /// The tab with this id and the position of its column.
    #[must_use]
    pub fn find_tab(&self, tab_id: &str) -> Option<(usize, &Tab)> {
        self.columns
            .iter()
            .enumerate()
            .find_map(|(i, c)| c.tabs.iter().find(|t| t.id == tab_id).map(|t| (i, t)))
    }

    fn id_in_use(&self, id: &str) -> bool {
        self.columns
            .iter()
            .any(|c| c.id == id || c.tabs.iter().any(|t| t.id == id))
            || self.closed_tabs.iter().any(|c| c.tab.id == id)
    }

    /// Mints a fresh id `prefix{serial}` no column, tab or closed tab of the workspace has.
    pub fn mint_id(&mut self, prefix: &str) -> String {
        loop {
            let id = format!("{prefix}{}", self.next_serial);
            self.next_serial += 1;
            if !self.id_in_use(&id) {
                return id;
            }
        }
    }

    /// Checks the invariants every command relies on.
    pub fn check(&self) -> Result<(), ShellError> {
        if self.schema_version != WORKSPACE_SCHEMA_VERSION {
            return Err(ShellError::UnsupportedVersion {
                record: "workspace".to_owned(),
                version: self.schema_version.clone(),
            });
        }
        check_id(&self.id)?;
        if self.columns.is_empty() {
            return Err(ShellError::Empty {
                what: format!("workspace {}", self.id),
            });
        }
        let mut columns = BTreeSet::new();
        let mut tabs = BTreeSet::new();
        for column in &self.columns {
            check_id(&column.id)?;
            if !columns.insert(column.id.as_str()) {
                return Err(ShellError::DuplicateId {
                    id: column.id.clone(),
                });
            }
            if column.tabs.is_empty() {
                return Err(ShellError::Empty {
                    what: format!("column {}", column.id),
                });
            }
            for tab in &column.tabs {
                check_id(&tab.id)?;
                if !tabs.insert(tab.id.as_str()) || columns.contains(tab.id.as_str()) {
                    return Err(ShellError::DuplicateId { id: tab.id.clone() });
                }
                if tab.pinned
                    && let Some(group) = &tab.link_group
                {
                    return Err(ShellError::PinnedInGroup {
                        tab_id: tab.id.clone(),
                        group: group.clone(),
                    });
                }
            }
            if column.shown().is_none() {
                return Err(ShellError::Dangling {
                    what: "shown tab".to_owned(),
                    id: column.shown_tab_id.clone(),
                });
            }
        }
        if !columns.contains(self.active_column_id.as_str()) {
            return Err(ShellError::Dangling {
                what: "active column".to_owned(),
                id: self.active_column_id.clone(),
            });
        }
        let total: f64 = self.columns.iter().map(|c| c.width_fraction).sum();
        if self.columns.iter().any(|c| c.width_fraction <= 0.0) || (total - 1.0).abs() > 1e-6 {
            return Err(ShellError::BadWidths {
                workspace_id: self.id.clone(),
            });
        }
        for class in DeviceClass::ALL {
            if let Some(profile) = self.layout_profiles.get(class) {
                for id in profile
                    .visible_column_ids
                    .iter()
                    .chain(profile.widths.keys())
                {
                    if !columns.contains(id.as_str()) {
                        return Err(ShellError::Dangling {
                            what: format!("column in the {class} profile"),
                            id: id.clone(),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    /// Scales the column widths to sum to one, keeping their proportions.
    pub fn normalize_widths(&mut self) {
        let total: f64 = self.columns.iter().map(|c| c.width_fraction).sum();
        if total > 0.0 && (total - 1.0).abs() > 1e-12 {
            for column in &mut self.columns {
                column.width_fraction /= total;
            }
        }
    }

    /// Migrates a 1.x workspace to 2.0 without losing a pane, link group, pinned state,
    /// scroll anchor, the active pane, the notes dock or any class's set of visible panes.
    ///
    /// The column structure comes from the widest stored profile (ultra-wide, wide, regular,
    /// compact), or from the pane order when there is none (a 1.0 file, or 1.1 with empty
    /// profiles): each visible pane becomes a column with one tab, the visible members of a
    /// collapsed link group share one column, and the tab-order panes become tabs of the last
    /// column. The active pane is its column's shown tab and that column is active; every
    /// other column shows its first tab. Column ids are `col-` plus the first tab's pane id;
    /// tab ids are the pane ids. Every stored class profile becomes a column profile naming
    /// the columns of its visible panes. Widths come from the structure profile when it sizes
    /// every visible pane, else from the panes' own `width_fraction`.
    pub fn from_v1(v1: &crate::Workspace) -> Result<Workspace, ShellError> {
        crate::check_workspace(v1)?;
        let pane = |id: &str| v1.pane(id).expect("reconciled ids are panes");
        let to_tab = |p: &Pane| Tab {
            id: p.id.clone(),
            kind: p.kind,
            content_id: p.content_id.clone(),
            content_revision: p.content_revision.clone(),
            reference: p.reference.clone(),
            scroll_anchor: p.scroll_anchor.clone(),
            link_group: p.link_group.clone(),
            pinned: p.pinned,
            history: PaneHistory::default(),
        };
        let structure = [
            DeviceClass::UltraWide,
            DeviceClass::Wide,
            DeviceClass::Regular,
            DeviceClass::Compact,
        ]
        .into_iter()
        .find_map(|class| v1.profile(class));
        let (order, visible, collapsed, widths, dock_open) = match structure {
            Some(profile) => {
                let (order, visible) = crate::reconcile(v1, profile);
                (
                    order,
                    visible.max(1),
                    profile.collapsed_link_groups.clone(),
                    Some(&profile.widths),
                    profile.notes_dock_open,
                )
            }
            None => {
                let order = v1.pane_ids();
                let visible = order.len();
                (order, visible, Vec::new(), None, false)
            }
        };

        // Visible panes become columns; collapsed groups share one.
        let mut columns: Vec<(Option<String>, Vec<String>)> = Vec::new();
        for id in &order[..visible] {
            let group = pane(id)
                .link_group
                .clone()
                .filter(|g| collapsed.contains(g));
            match group
                .as_ref()
                .and_then(|g| columns.iter_mut().find(|(cg, _)| cg.as_ref() == Some(g)))
            {
                Some((_, members)) => members.push(id.clone()),
                None => columns.push((group, vec![id.clone()])),
            }
        }
        let visible_members: Vec<Vec<String>> = columns.iter().map(|(_, m)| m.clone()).collect();
        if let Some((_, last)) = columns.last_mut() {
            last.extend(order[visible..].iter().cloned());
        }

        let sized = widths.filter(|w| order[..visible].iter().all(|id| w.contains_key(id)));
        let share = |members: &[String]| -> f64 {
            match sized {
                Some(w) => members.iter().map(|id| w[id]).sum(),
                None if structure.is_some() => 1.0,
                None => members.iter().map(|id| pane(id).width_fraction).sum(),
            }
        };
        let column_id = |members: &[String]| format!("col-{}", members[0]);
        let mut out_columns: Vec<Column> = columns
            .iter()
            .zip(&visible_members)
            .map(|((_, members), shown_members)| {
                let shown = if members.contains(&v1.active_pane_id) {
                    v1.active_pane_id.clone()
                } else {
                    members[0].clone()
                };
                Column {
                    id: column_id(members),
                    tabs: members.iter().map(|id| to_tab(pane(id))).collect(),
                    shown_tab_id: shown,
                    width_fraction: share(shown_members),
                }
            })
            .collect();
        let active_column_id = out_columns
            .iter()
            .find(|c| c.tabs.iter().any(|t| t.id == v1.active_pane_id))
            .map(|c| c.id.clone())
            .expect("the active pane is placed");

        let column_of = |pane_id: &str| -> String {
            out_columns
                .iter()
                .find(|c| c.tabs.iter().any(|t| t.id == pane_id))
                .map(|c| c.id.clone())
                .expect("every pane is placed")
        };
        let mut layout_profiles = ColumnProfiles::default();
        for class in DeviceClass::ALL {
            let Some(profile) = v1.profile(class) else {
                continue;
            };
            let (order, visible) = crate::reconcile(v1, profile);
            let mut visible_column_ids: Vec<String> = Vec::new();
            for id in &order[..visible] {
                let column = column_of(id);
                if !visible_column_ids.contains(&column) {
                    visible_column_ids.push(column);
                }
            }
            let mut widths = BTreeMap::new();
            if !profile.widths.is_empty()
                && order[..visible]
                    .iter()
                    .all(|id| profile.widths.contains_key(id))
            {
                for id in &order[..visible] {
                    *widths.entry(column_of(id)).or_insert(0.0) += profile.widths[id];
                }
            }
            layout_profiles.set(
                class,
                ColumnProfile {
                    visible_column_ids,
                    widths,
                    notes_dock_open: profile.notes_dock_open,
                },
            );
        }

        let mut workspace = Workspace {
            schema_version: WORKSPACE_SCHEMA_VERSION.to_owned(),
            fixture_only: v1.fixture_only,
            id: v1.id.clone(),
            title: v1.title.clone(),
            columns: std::mem::take(&mut out_columns),
            active_column_id,
            notes_dock: NotesDock {
                note_id: v1.notes_dock_id.clone(),
                open: dock_open,
            },
            layout_profiles,
            next_serial: 1,
            closed_tabs: Vec::new(),
        };
        workspace.normalize_widths();
        workspace.check()?;
        Ok(workspace)
    }

    /// Exports to a 1.1 workspace for a reader that predates 2.0. Panes are the tabs in
    /// column and tab order; a pane's `width_fraction` is its column's share divided among
    /// the column's tabs. When every column has one tab and no class has a profile the
    /// export has empty profiles, so the 1.1 reader derives from the pane order; otherwise
    /// each stored class profile (or, with none, a wide profile of every column) shows each
    /// visible column's shown tab and lists the other tabs in tab order. Fails when a class
    /// would show more columns than 1.1 allows.
    pub fn to_v1(&self) -> Result<crate::Workspace, ShellError> {
        self.check()?;
        let panes: Vec<Pane> = self
            .columns
            .iter()
            .flat_map(|c| {
                let share = c.width_fraction / c.tabs.len() as f64;
                c.tabs.iter().map(move |t| Pane {
                    id: t.id.clone(),
                    kind: t.kind,
                    content_id: t.content_id.clone(),
                    content_revision: t.content_revision.clone(),
                    reference: t.reference.clone(),
                    link_group: t.link_group.clone(),
                    pinned: t.pinned,
                    width_fraction: share,
                    scroll_anchor: t.scroll_anchor.clone(),
                })
            })
            .collect();
        let mut profiles = LayoutProfiles::default();
        let single_tabs = self.columns.iter().all(|c| c.tabs.len() == 1);
        let mut stored: Vec<(DeviceClass, ColumnProfile)> = DeviceClass::ALL
            .iter()
            .filter_map(|class| {
                self.layout_profiles
                    .get(*class)
                    .map(|p| (*class, p.clone()))
            })
            .collect();
        if stored.is_empty() && !single_tabs {
            stored.push((
                DeviceClass::Wide,
                ColumnProfile {
                    visible_column_ids: self.columns.iter().map(|c| c.id.clone()).collect(),
                    widths: BTreeMap::new(),
                    notes_dock_open: self.notes_dock.open,
                },
            ));
        }
        for (class, profile) in stored {
            let mut visible_pane_ids = Vec::new();
            let mut widths = BTreeMap::new();
            for column_id in &profile.visible_column_ids {
                let column = self
                    .columns
                    .iter()
                    .find(|c| &c.id == column_id)
                    .expect("checked");
                visible_pane_ids.push(column.shown_tab_id.clone());
                if let Some(w) = profile.widths.get(column_id) {
                    widths.insert(column.shown_tab_id.clone(), *w);
                }
            }
            let tab_order: Vec<String> = panes
                .iter()
                .map(|p| p.id.clone())
                .filter(|id| !visible_pane_ids.contains(id))
                .collect();
            if visible_pane_ids.len() > class.max_columns() {
                return Err(ShellError::NotExpressible {
                    detail: format!(
                        "the {class} profile shows {} columns; 1.1 allows {}",
                        visible_pane_ids.len(),
                        class.max_columns()
                    ),
                });
            }
            profiles.set(
                class,
                LayoutProfile {
                    visible_pane_ids,
                    tab_order,
                    widths,
                    notes_dock_open: profile.notes_dock_open,
                    collapsed_link_groups: Vec::new(),
                },
            );
        }
        let workspace = crate::Workspace {
            schema_version: crate::CURRENT_SCHEMA_VERSION.to_owned(),
            fixture_only: self.fixture_only,
            id: self.id.clone(),
            title: self.title.clone(),
            panes,
            notes_dock_id: self.notes_dock.note_id.clone(),
            active_pane_id: self.active_tab().id.clone(),
            layout_profiles: Some(profiles),
        };
        for class in DeviceClass::ALL {
            if let Some(profile) = workspace.profile(class) {
                crate::check_profile(&workspace, class, profile)?;
            }
        }
        Ok(workspace)
    }
}

impl Session {
    /// A session holding one workspace.
    #[must_use]
    pub fn single(workspace: Workspace) -> Session {
        Session {
            schema_version: SESSION_SCHEMA_VERSION.to_owned(),
            fixture_only: workspace.fixture_only,
            active_workspace_id: workspace.id.clone(),
            workspaces: vec![workspace],
            tool: Tool::Select,
            next_serial: 1,
            reading: ReadingSettings::default(),
            ink: InkSettings::default(),
        }
    }

    /// The active workspace's position.
    #[must_use]
    pub fn active_index(&self) -> usize {
        self.workspaces
            .iter()
            .position(|w| w.id == self.active_workspace_id)
            .unwrap_or(0)
    }

    /// The active workspace.
    #[must_use]
    pub fn active(&self) -> &Workspace {
        &self.workspaces[self.active_index()]
    }

    /// The active workspace, mutably.
    pub fn active_mut(&mut self) -> &mut Workspace {
        let index = self.active_index();
        &mut self.workspaces[index]
    }

    /// The workspace with this id.
    #[must_use]
    pub fn workspace(&self, id: &str) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.id == id)
    }

    /// Mints a workspace id no open workspace has.
    pub fn mint_workspace_id(&mut self) -> String {
        loop {
            let id = format!("w{}", self.next_serial);
            self.next_serial += 1;
            if self.workspace(&id).is_none() {
                return id;
            }
        }
    }

    /// Checks the session and every workspace.
    pub fn check(&self) -> Result<(), ShellError> {
        if self.schema_version != SESSION_SCHEMA_VERSION {
            return Err(ShellError::UnsupportedVersion {
                record: "session".to_owned(),
                version: self.schema_version.clone(),
            });
        }
        if self.workspaces.is_empty() {
            return Err(ShellError::Empty {
                what: "session".to_owned(),
            });
        }
        let mut seen = BTreeSet::new();
        for workspace in &self.workspaces {
            workspace.check()?;
            if !seen.insert(workspace.id.as_str()) {
                return Err(ShellError::DuplicateId {
                    id: workspace.id.clone(),
                });
            }
        }
        if !seen.contains(self.active_workspace_id.as_str()) {
            return Err(ShellError::Dangling {
                what: "active workspace".to_owned(),
                id: self.active_workspace_id.clone(),
            });
        }
        Ok(())
    }

    /// Splits the session into the session record and one record per workspace.
    #[must_use]
    pub fn to_records(&self) -> (SessionRecord, Vec<Workspace>) {
        (
            SessionRecord {
                schema_version: self.schema_version.clone(),
                fixture_only: self.fixture_only,
                workspace_ids: self.workspaces.iter().map(|w| w.id.clone()).collect(),
                active_workspace_id: self.active_workspace_id.clone(),
                tool: self.tool,
                next_serial: self.next_serial,
                reading: self.reading.clone(),
                ink: self.ink.clone(),
            },
            self.workspaces.clone(),
        )
    }

    /// Joins a session record and its workspaces, in the record's order.
    pub fn from_records(
        record: SessionRecord,
        workspaces: Vec<Workspace>,
    ) -> Result<Session, ShellError> {
        let mut by_id: BTreeMap<String, Workspace> =
            workspaces.into_iter().map(|w| (w.id.clone(), w)).collect();
        let mut ordered = Vec::with_capacity(record.workspace_ids.len());
        for id in &record.workspace_ids {
            match by_id.remove(id) {
                Some(w) => ordered.push(w),
                None => {
                    return Err(ShellError::Dangling {
                        what: "workspace".to_owned(),
                        id: id.clone(),
                    });
                }
            }
        }
        let session = Session {
            schema_version: record.schema_version,
            fixture_only: record.fixture_only,
            workspaces: ordered,
            active_workspace_id: record.active_workspace_id,
            tool: record.tool,
            next_serial: record.next_serial,
            reading: record.reading,
            ink: record.ink,
        };
        session.check()?;
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TextSize, derive_profile, migrate_workspace_to_current};

    /// The example under `examples/`, or `None` when the tree is filtered.
    fn example(name: &str) -> Option<String> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/").to_owned() + name;
        std::fs::read_to_string(&path).ok()
    }

    fn v1(name: &str) -> Option<crate::Workspace> {
        let value: serde_json::Value = serde_json::from_str(&example(name)?).unwrap();
        Some(serde_json::from_value(migrate_workspace_to_current(value).unwrap()).unwrap())
    }

    fn column_tabs(ws: &Workspace) -> Vec<Vec<&str>> {
        ws.columns
            .iter()
            .map(|c| c.tabs.iter().map(|t| t.id.as_str()).collect())
            .collect()
    }

    /// Everything a 1.x pane says that a reader acts on, keyed by pane id.
    /// Id, kind, resource, passage, link group, pinned and anchor.
    type PaneFacts = (
        String,
        PaneKind,
        String,
        Option<String>,
        Option<String>,
        bool,
        Option<ScrollAnchor>,
    );

    fn pane_facts(ws: &crate::Workspace) -> Vec<PaneFacts> {
        ws.panes
            .iter()
            .map(|p| {
                (
                    p.id.clone(),
                    p.kind,
                    p.content_id.clone(),
                    p.reference.clone(),
                    p.link_group.clone(),
                    p.pinned,
                    p.scroll_anchor.clone(),
                )
            })
            .collect()
    }

    /// Migrating forward then exporting back keeps every pane fact, the active pane, the
    /// notes dock and every derived layout.
    fn assert_round_trip(original: &crate::Workspace) {
        let migrated = Workspace::from_v1(original).unwrap();
        let back = migrated.to_v1().unwrap();
        assert_eq!(pane_facts(&back), pane_facts(original));
        assert_eq!(back.active_pane_id, original.active_pane_id);
        assert_eq!(back.notes_dock_id, original.notes_dock_id);
        for class in DeviceClass::ALL {
            for (width, size) in [
                (2560.0, TextSize::Standard),
                (1440.0, TextSize::Standard),
                (1024.0, TextSize::Standard),
                (1024.0, TextSize::Accessibility),
                (390.0, TextSize::Standard),
            ] {
                let a = derive_profile(original, class, width, size).unwrap();
                let b = derive_profile(&back, class, width, size).unwrap();
                assert_eq!(a.shown_pane_ids(), b.shown_pane_ids(), "{class} at {width}");
                assert_eq!(a.tabs, b.tabs, "{class} at {width}");
                assert_eq!(a.notes_dock, b.notes_dock, "{class} at {width}");
                for (x, y) in a.columns.iter().zip(&b.columns) {
                    assert!((x.width_pt - y.width_pt).abs() < 1e-6, "{class} at {width}");
                }
            }
        }
        // And forward again is the same 2.0 workspace.
        assert_eq!(
            Workspace::from_v1(&back).unwrap().columns.len(),
            migrated.columns.len()
        );
    }

    #[test]
    fn the_one_point_one_example_migrates_to_five_columns_with_both_profiles() {
        let Some(original) = v1("archive/workspace-1.1.json") else {
            return;
        };
        let ws = Workspace::from_v1(&original).unwrap();
        assert_eq!(
            column_tabs(&ws),
            vec![vec!["p1"], vec!["p2"], vec!["p3"], vec!["p4"], vec!["p5"]]
        );
        assert_eq!(ws.active_column_id, "col-p1");
        assert_eq!(ws.active_tab().id, "p1");
        let widths: Vec<f64> = ws.columns.iter().map(|c| c.width_fraction).collect();
        assert_eq!(widths, vec![0.22, 0.22, 0.22, 0.2, 0.14]);
        let regular = ws.layout_profiles.get(DeviceClass::Regular).unwrap();
        assert_eq!(
            regular.visible_column_ids,
            vec!["col-p1", "col-p2", "col-p3"]
        );
        assert!(regular.widths.is_empty());
        assert_eq!(
            ws.layout_profiles
                .get(DeviceClass::Wide)
                .unwrap()
                .visible_column_ids
                .len(),
            5
        );
        assert!(ws.layout_profiles.get(DeviceClass::Compact).is_none());
        assert_eq!(ws.notes_dock.note_id.as_deref(), Some("fixture:note:1"));
        assert!(!ws.notes_dock.open);
        let anchors: Vec<_> = ws.tabs().map(|t| t.scroll_anchor.clone()).collect();
        let before: Vec<_> = original
            .panes
            .iter()
            .map(|p| p.scroll_anchor.clone())
            .collect();
        assert_eq!(anchors, before);
        assert_round_trip(&original);
    }

    #[test]
    fn the_one_point_zero_example_migrates_from_its_pane_order() {
        let Some(original) = v1("archive/workspace-1.0.json") else {
            return;
        };
        let ws = Workspace::from_v1(&original).unwrap();
        assert_eq!(column_tabs(&ws), vec![vec!["p1"], vec!["p2"], vec!["p3"]]);
        assert!(ws.layout_profiles.is_empty());
        assert_round_trip(&original);
        // With no profiles and one tab per column the export is the 1.0 → 1.1 migration.
        let back = ws.to_v1().unwrap();
        let expected: crate::Workspace = serde_json::from_value(
            migrate_workspace_to_current(
                serde_json::from_str(&example("archive/workspace-1.0.json").unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(back, expected);
    }

    #[test]
    fn tab_order_panes_become_tabs_of_the_last_column_and_collapsed_groups_share_one() {
        let Some(mut original) = v1("archive/workspace-1.1.json") else {
            return;
        };
        // Only the regular profile: three visible, p4 and p5 as tabs.
        original.layout_profiles.as_mut().unwrap().wide = None;
        original.active_pane_id = "p5".to_owned();
        let ws = Workspace::from_v1(&original).unwrap();
        assert_eq!(
            column_tabs(&ws),
            vec![vec!["p1"], vec!["p2"], vec!["p3", "p4", "p5"]]
        );
        assert_eq!(ws.active_column_id, "col-p3");
        assert_eq!(ws.active_tab().id, "p5");
        assert_eq!(ws.columns[2].shown_tab_id, "p5");
        // Exported back, the regular profile shows p5 where p3 was, as the 1.1 reader's
        // rule 4 already did on regular; a class without a stored profile then borrows that
        // order, so the exact round trip holds when the active pane was visible.
        original.active_pane_id = "p1".to_owned();
        assert_round_trip(&original);

        // A compact profile that collapses group 1 into one column.
        let mut collapsed = v1("archive/workspace-1.1.json").unwrap();
        collapsed.layout_profiles = Some(LayoutProfiles {
            compact: Some(LayoutProfile {
                visible_pane_ids: vec!["p1".into(), "p2".into()],
                tab_order: vec!["p3".into(), "p4".into(), "p5".into()],
                widths: BTreeMap::new(),
                notes_dock_open: true,
                collapsed_link_groups: vec!["1".into()],
            }),
            ..LayoutProfiles::default()
        });
        let ws = Workspace::from_v1(&collapsed).unwrap();
        assert_eq!(column_tabs(&ws), vec![vec!["p1", "p2", "p3", "p4", "p5"]]);
        assert!(ws.notes_dock.open);
        assert_eq!(ws.columns[0].width_fraction, 1.0);
    }

    #[test]
    fn invariants_are_checked() {
        let Some(original) = v1("archive/workspace-1.1.json") else {
            return;
        };
        let good = Workspace::from_v1(&original).unwrap();
        let mut bad = good.clone();
        bad.columns[0].tabs.clear();
        assert!(matches!(bad.check(), Err(ShellError::Empty { .. })));
        let mut bad = good.clone();
        bad.columns[1].tabs[0].id = "p1".into();
        assert_eq!(
            bad.check(),
            Err(ShellError::DuplicateId { id: "p1".into() })
        );
        let mut bad = good.clone();
        bad.columns[0].width_fraction = 0.5;
        assert!(matches!(bad.check(), Err(ShellError::BadWidths { .. })));
        let mut bad = good.clone();
        bad.active_column_id = "col-nope".into();
        assert!(matches!(bad.check(), Err(ShellError::Dangling { .. })));
        let mut bad = good.clone();
        bad.columns[0].tabs[0].pinned = true;
        assert!(matches!(bad.check(), Err(ShellError::PinnedInGroup { .. })));
        let mut bad = good.clone();
        bad.columns[0].shown_tab_id = "p2".into();
        assert!(matches!(bad.check(), Err(ShellError::Dangling { .. })));
        let mut bad = good.clone();
        bad.schema_version = "1.1".into();
        assert!(matches!(
            bad.check(),
            Err(ShellError::UnsupportedVersion { .. })
        ));
        let mut bad = good.clone();
        bad.columns[0].id = "has space".into();
        assert!(matches!(bad.check(), Err(ShellError::BadId { .. })));
        let mut seven = good.clone();
        for i in 0..2 {
            let mut column = seven.columns[0].clone();
            column.id = format!("extra{i}");
            column.tabs[0].id = format!("extra-tab{i}");
            column.shown_tab_id = column.tabs[0].id.clone();
            seven.columns.push(column);
        }
        seven.normalize_widths();
        seven.layout_profiles = ColumnProfiles::default();
        seven.layout_profiles.set(
            DeviceClass::Wide,
            ColumnProfile {
                visible_column_ids: seven.columns.iter().map(|c| c.id.clone()).collect(),
                widths: BTreeMap::new(),
                notes_dock_open: false,
            },
        );
        assert!(matches!(
            seven.to_v1(),
            Err(ShellError::NotExpressible { .. })
        ));
    }

    /// `OPENLOGOS_WRITE_EXAMPLES=1 cargo test -p openlogos-workspaces write_examples`
    /// regenerates examples/workspace.json (the archived 1.1 example migrated) and
    /// examples/session.json (a session of that one workspace).
    #[test]
    fn write_examples() {
        if std::env::var("OPENLOGOS_WRITE_EXAMPLES").as_deref() != Ok("1") {
            return;
        }
        let original = v1("archive/workspace-1.1.json").unwrap();
        let ws = Workspace::from_v1(&original).unwrap();
        let session = Session::single(ws.clone());
        let (record, _) = session.to_records();
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/");
        for (name, value) in [
            ("workspace.json", serde_json::to_string_pretty(&ws).unwrap()),
            (
                "session.json",
                serde_json::to_string_pretty(&record).unwrap(),
            ),
        ] {
            std::fs::write(format!("{dir}{name}"), value + "\n").unwrap();
        }
    }

    #[test]
    fn the_examples_are_the_migration_and_round_trip_through_serde() {
        let (Some(current), Some(session), Some(original)) = (
            example("workspace.json"),
            example("session.json"),
            v1("archive/workspace-1.1.json"),
        ) else {
            return;
        };
        let ws: Workspace = serde_json::from_str(&current).unwrap();
        ws.check().unwrap();
        assert_eq!(ws, Workspace::from_v1(&original).unwrap());
        let value: serde_json::Value = serde_json::from_str(&current).unwrap();
        assert_eq!(serde_json::to_value(&ws).unwrap(), value);

        let record: SessionRecord = serde_json::from_str(&session).unwrap();
        let joined = Session::from_records(record.clone(), vec![ws.clone()]).unwrap();
        assert_eq!(joined.active().id, ws.id);
        assert_eq!(joined.to_records(), (record.clone(), vec![ws]));
        let value: serde_json::Value = serde_json::from_str(&session).unwrap();
        assert_eq!(serde_json::to_value(&record).unwrap(), value);
        assert!(matches!(
            Session::from_records(record, Vec::new()),
            Err(ShellError::Dangling { .. })
        ));
    }

    #[test]
    fn minted_ids_skip_ids_in_use() {
        let Some(original) = v1("archive/workspace-1.1.json") else {
            return;
        };
        let mut ws = Workspace::from_v1(&original).unwrap();
        ws.columns[0].tabs[0].id = "t1".into();
        ws.columns[0].shown_tab_id = "t1".into();
        assert_eq!(ws.mint_id("t"), "t2");
        assert_eq!(ws.mint_id("c"), "c3");
        assert_eq!(ws.next_serial, 4);
    }
}
