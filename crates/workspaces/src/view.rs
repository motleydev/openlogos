//! What a host renders from a [`Shell`] (B070): every workspace, column (with its number) and
//! tab with the labels the core owns, so the UniFFI and wasm surfaces build the same view and
//! no host formats a reference itself. Tab strips show [`TabView::chapter_label`] (`Jn 3`),
//! identity rows [`TabView::reference_label`] (`Jn 3:16`).
//!
//! [`Shell::view`] runs on every render, so it must stay far below a frame: references go
//! through the palette's OSIS fast path ([`crate::palette::parse_tab_reference`]) and the
//! alias tables are built once per process. `just bench-shell` times it over ten
//! workspaces of 3 columns × 4 tabs.

use openlogos_core::reference::{Reference, ReferenceRange, ReferenceSet};
use serde::{Deserialize, Serialize};

use crate::commands::Shell;
use crate::palette::parse_tab_reference;
use crate::shell::{InkSettings, LinkMode, ReadingSettings, Tab, Workspace};

/// One tab as a host renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TabView {
    /// Stable id within the workspace.
    pub id: String,
    /// `reading`, `notes`, `research` or `lexicon`.
    pub kind: String,
    /// The resource.
    pub content_id: String,
    /// The resource revision.
    pub content_revision: String,
    /// The passage as stored.
    pub reference: Option<String>,
    /// The resource's registered short name, else its id.
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
}

/// One column; `number` is its position, 1-based.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnView {
    /// Stable id.
    pub id: String,
    /// Position, 1-based: what the identity row shows.
    pub number: u32,
    /// Its tabs, in order.
    pub tabs: Vec<TabView>,
    /// The tab shown.
    pub shown_tab_id: String,
    /// Width share.
    pub width_fraction: f64,
}

/// One workspace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceView {
    /// Stable id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Columns 1…n.
    pub columns: Vec<ColumnView>,
    /// The active column's number.
    pub active_column_number: u32,
    /// The note in the dock.
    pub notes_dock_note_id: Option<String>,
    /// Whether the dock is open.
    pub notes_dock_open: bool,
}

/// The whole session as a host renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionView {
    /// Every workspace, in order.
    pub workspaces: Vec<WorkspaceView>,
    /// The active workspace.
    pub active_workspace_id: String,
    /// The selected tool, as stored.
    pub tool: String,
    /// The tool in effect for the active tab (B076): `tool` when the tab's context offers
    /// it, else the context's default. [`Shell::tool_well`] answers for the notes dock.
    pub effective_tool: String,
    /// The active tab's tool-well context: `translation`, `original`, `notes` or
    /// `reference` (B076).
    pub tool_context: String,
    /// Bumped by every change.
    pub revision: u64,
    /// The command Undo would take back, if any.
    pub undo_title: Option<String>,
    /// The command Redo would run again, if any.
    pub redo_title: Option<String>,
    /// Text size, line spacing and theme.
    pub reading: ReadingSettings,
    /// Brush width and swatches.
    pub ink: InkSettings,
}

/// The short labels of a stored reference: `(Jn 3:16, Jn 3)`; both `None` when it does not
/// parse (a fixture id, a commentary locator) or is absent.
#[must_use]
pub fn reference_labels(reference: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(set) = reference.and_then(parse_tab_reference) else {
        return (None, None);
    };
    let start = &set.ranges[0].start;
    let chapter = ReferenceSet {
        ranges: vec![ReferenceRange::single(Reference::chapter(
            start.book,
            start.chapter,
        ))],
        flags: Vec::new(),
    };
    (Some(set.to_short()), Some(chapter.to_short()))
}

fn tab_view(shell: &Shell, tab: &Tab) -> TabView {
    let (link_mode, link_group) = match tab.link_mode() {
        LinkMode::Pinned => ("pinned", None),
        LinkMode::Follows => ("follows", None),
        LinkMode::Group { group } => ("group", Some(group)),
    };
    let (reference_label, chapter_label) = reference_labels(tab.reference.as_deref());
    TabView {
        id: tab.id.clone(),
        kind: crate::shell::kind_name(tab.kind).to_owned(),
        content_id: tab.content_id.clone(),
        content_revision: tab.content_revision.clone(),
        reference: tab.reference.clone(),
        resource_title: shell.session.resource_title(&tab.content_id).to_owned(),
        reference_label,
        chapter_label,
        scroll_segment_id: tab.scroll_anchor.as_ref().map(|a| a.segment_id.clone()),
        scroll_scalar: tab.scroll_anchor.as_ref().map(|a| a.scalar),
        link_mode: link_mode.to_owned(),
        link_group,
        can_go_back: !tab.history.back.is_empty(),
        can_go_forward: !tab.history.forward.is_empty(),
    }
}

fn workspace_view(shell: &Shell, workspace: &Workspace) -> WorkspaceView {
    WorkspaceView {
        id: workspace.id.clone(),
        title: workspace.title.clone(),
        columns: workspace
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| ColumnView {
                id: c.id.clone(),
                number: i as u32 + 1,
                tabs: c.tabs.iter().map(|t| tab_view(shell, t)).collect(),
                shown_tab_id: c.shown_tab_id.clone(),
                width_fraction: c.width_fraction,
            })
            .collect(),
        active_column_number: workspace.active_index() as u32 + 1,
        notes_dock_note_id: workspace.notes_dock.note_id.clone(),
        notes_dock_open: workspace.notes_dock.open,
    }
}

impl Shell {
    /// The session as a host renders it, with the core's labels.
    #[must_use]
    pub fn view(&self) -> SessionView {
        SessionView {
            workspaces: self
                .session
                .workspaces
                .iter()
                .map(|w| workspace_view(self, w))
                .collect(),
            active_workspace_id: self.session.active_workspace_id.clone(),
            tool: self.session.tool.name().to_owned(),
            effective_tool: crate::well::effective_tool(&self.session, &Default::default())
                .name()
                .to_owned(),
            tool_context: crate::well::tool_context(&self.session, &Default::default())
                .name()
                .to_owned(),
            revision: self.revision,
            undo_title: self.undo_stack.last().map(|r| r.title.clone()),
            redo_title: self.redo_stack.last().map(|r| r.title.clone()),
            reading: self.session.reading.clone(),
            ink: self.session.ink.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::tests::fixture_session;

    #[test]
    fn labels_are_short_and_chapter_labels_drop_the_verse() {
        assert_eq!(
            reference_labels(Some("John.3.16")),
            (Some("Jn 3:16".into()), Some("Jn 3".into()))
        );
        assert_eq!(
            reference_labels(Some("Gen 1:1-2")),
            (Some("Gen 1:1–2".into()), Some("Gen 1".into()))
        );
        assert_eq!(
            reference_labels(Some("Ps.23")),
            (Some("Ps 23".into()), Some("Ps 23".into()))
        );
        assert_eq!(reference_labels(Some("fixture:passage:1")), (None, None));
        assert_eq!(reference_labels(None), (None, None));
    }

    #[test]
    fn the_view_numbers_columns_and_labels_tabs() {
        let mut shell = Shell::new(fixture_session()).unwrap();
        shell.set_resource_titles([("bsb".to_owned(), "BSB".to_owned())].into());
        let view = shell.view();
        assert_eq!(view.workspaces.len(), 2);
        let column = &view.workspaces[0].columns[1];
        assert_eq!(column.number, 2);
        let tab = &view.workspaces[0].columns[0].tabs[0];
        assert_eq!(tab.resource_title, "BSB");
        assert_eq!(tab.reference_label.as_deref(), Some("Jn 3:16"));
        assert_eq!(tab.chapter_label.as_deref(), Some("Jn 3"));
        assert_eq!(tab.kind, "reading");
        assert_eq!(tab.link_mode, "group");
    }
}
