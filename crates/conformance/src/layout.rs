//! The workspace layout fixture (B062): the five-pane two-group Mac workspace of specs/18
//! opened on every device class, so Swift, Kotlin and JavaScript hosts can assert through
//! [`crate::run_all`] that the core places every pane, keeps link groups and scroll anchors,
//! shows the active pane and saves one class's arrangement without touching another
//! (specs/18-platform-capabilities.md, "Workspace layout profiles").

use openlogos_workspaces::{
    DerivedLayout, DeviceClass, LayoutProfile, NotesDockPlacement, Pane, ProfileSource,
    ScrollAnchor, TextSize, Workspace, derive_profile, with_profile,
};
use serde::{Deserialize, Serialize};

use crate::Recorder;

/// What a host must see for one class, width and text size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedLayout {
    /// Columns available to panes.
    pub slots: usize,
    /// Each column as its shown pane followed by any stacked panes.
    pub columns: Vec<Vec<String>>,
    /// Tabs, in order.
    pub tabs: Vec<String>,
    /// The notes dock.
    pub notes_dock: NotesDockPlacement,
    /// Where the order came from.
    pub profile_source: ProfileSource,
    /// Column widths rounded to whole points.
    pub column_widths_pt: Vec<i64>,
    /// Which columns centre their text at the measure cap.
    pub centered_measure: Vec<bool>,
}

impl From<&DerivedLayout> for ExpectedLayout {
    fn from(layout: &DerivedLayout) -> Self {
        ExpectedLayout {
            slots: layout.slots,
            columns: layout
                .columns
                .iter()
                .map(|c| {
                    std::iter::once(c.pane_id.clone())
                        .chain(c.stacked_pane_ids.iter().cloned())
                        .collect()
                })
                .collect(),
            tabs: layout.tabs.clone(),
            notes_dock: layout.notes_dock,
            profile_source: layout.profile_source,
            column_widths_pt: layout
                .columns
                .iter()
                .map(|c| c.width_pt.round() as i64)
                .collect(),
            centered_measure: layout.columns.iter().map(|c| c.centered_measure).collect(),
        }
    }
}

/// One opening of the fixture workspace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutCase {
    /// What the case shows.
    pub name: String,
    /// The class the host reports.
    pub device_class: DeviceClass,
    /// The width available to panes, in points.
    pub width_pt: f64,
    /// The text size.
    pub text_size: TextSize,
    /// Overrides the workspace's active pane for this case.
    #[serde(default)]
    pub active_pane_id: Option<String>,
    /// A pane appended to the workspace before deriving, as if added on another device
    /// after the profile was saved; the profile does not know it.
    #[serde(default)]
    pub extra_pane: Option<Pane>,
    /// The layout the core must derive.
    pub expected: ExpectedLayout,
}

/// A workspace layout fixture: one workspace and the layouts it must derive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceLayoutFixture {
    /// Fixture id (also the file name).
    pub id: String,
    /// What the fixture shows.
    pub description: String,
    /// License of the data.
    pub license: String,
    /// Where the expectations come from.
    pub source: String,
    /// The workspace, as schemas/workspace.schema.json stores it.
    pub workspace: Workspace,
    /// The openings.
    pub cases: Vec<LayoutCase>,
}

/// A pane's state that no layout may change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneState {
    /// The pane.
    pub id: String,
    /// Its link group.
    pub link_group: Option<String>,
    /// Whether it is pinned.
    pub pinned: bool,
    /// Its scroll anchor.
    pub scroll_anchor: Option<ScrollAnchor>,
}

fn pane_states(workspace: &Workspace) -> Vec<PaneState> {
    workspace
        .panes
        .iter()
        .map(|p| PaneState {
            id: p.id.clone(),
            link_group: p.link_group.clone(),
            pinned: p.pinned,
            scroll_anchor: p.scroll_anchor.clone(),
        })
        .collect()
}

fn other_profiles(
    workspace: &Workspace,
    class: DeviceClass,
) -> Vec<(String, Option<LayoutProfile>)> {
    DeviceClass::ALL
        .iter()
        .filter(|c| **c != class)
        .map(|c| (c.name().to_owned(), workspace.profile(*c).cloned()))
        .collect()
}

pub(crate) fn run_workspace_layout(fixture: &WorkspaceLayoutFixture, r: &mut Recorder) {
    for case in &fixture.cases {
        let mut workspace = fixture.workspace.clone();
        if let Some(extra) = &case.extra_pane {
            workspace.panes.push(extra.clone());
        }
        if let Some(active) = &case.active_pane_id {
            workspace.active_pane_id = active.clone();
        }
        let before = workspace.clone();
        let layout =
            match derive_profile(&workspace, case.device_class, case.width_pt, case.text_size) {
                Ok(layout) => layout,
                Err(error) => {
                    r.fail(
                        &format!("{}: layout", case.name),
                        "a derived layout",
                        &error.to_string(),
                    );
                    continue;
                }
            };
        let derived = ExpectedLayout::from(&layout);
        r.check(&format!("{}: layout", case.name), &case.expected, &derived);

        let mut placed = layout.placed_pane_ids();
        placed.sort();
        let mut all = workspace.pane_ids();
        all.sort();
        r.check(
            &format!("{}: every pane placed exactly once", case.name),
            &all,
            &placed,
        );
        r.check(
            &format!("{}: link groups and scroll anchors survive", case.name),
            &pane_states(&before),
            &pane_states(&workspace),
        );
        r.check(
            &format!("{}: the active pane is shown", case.name),
            &true,
            &layout
                .columns
                .iter()
                .any(|c| c.pane_id == layout.active_pane_id),
        );

        match with_profile(&workspace, case.device_class, layout.profile.clone()) {
            Ok(saved) => {
                r.check(
                    &format!("{}: saving touches no other class", case.name),
                    &other_profiles(&workspace, case.device_class),
                    &other_profiles(&saved, case.device_class),
                );
                r.check(
                    &format!("{}: saving keeps every pane", case.name),
                    &pane_states(&workspace),
                    &pane_states(&saved),
                );
                match derive_profile(&saved, case.device_class, case.width_pt, case.text_size) {
                    Ok(again) => {
                        // The saved arrangement is now the stored profile; everything but the
                        // source must be what the first opening produced.
                        let mut reopened = ExpectedLayout::from(&again);
                        reopened.profile_source = derived.profile_source;
                        r.check(
                            &format!("{}: the saved arrangement reopens identically", case.name),
                            &derived,
                            &reopened,
                        );
                        r.check(
                            &format!("{}: the saved arrangement is the stored profile", case.name),
                            &ProfileSource::Stored,
                            &again.profile_source,
                        );
                    }
                    Err(error) => r.fail(
                        &format!("{}: the saved arrangement reopens identically", case.name),
                        "a derived layout",
                        &error.to_string(),
                    ),
                }
            }
            Err(error) => r.fail(
                &format!("{}: saving touches no other class", case.name),
                "a saved workspace",
                &error.to_string(),
            ),
        }
    }
}
