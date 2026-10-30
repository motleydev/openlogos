//! Workspace layout as portable data. Three synchronized reading panes, the pane tree, link
//! groups that keep panes scrolling together, and the layout profiles that adapt to iPad, Mac,
//! phone, web and ultra-wide displays are all represented here so a workspace restores
//! identically on every platform (specs/12-workspace-ux.md,
//! specs/18-platform-capabilities.md). The reference fixture is
//! examples/archive/workspace-1.1.json (schema 1.1), validated by tools/validate.py; the
//! current example, examples/workspace.json, is schema 2.0 ([`shell`]).
//!
//! The one rule everything here serves: a [`Workspace`] owns its panes, and a pane owns its
//! link state and scroll anchor. A [`LayoutProfile`] only says where each pane is shown on
//! one [`DeviceClass`], so no profile can lose a pane, a link group or a scroll position.
//! [`derive_profile`] turns a workspace, a class, a width and a text size into the
//! [`DerivedLayout`] a host renders, following the six derivation rules of specs/18
//! ("Workspace layout profiles"); [`with_profile`] saves one class's arrangement without
//! touching another's. Everything is plain data and pure functions, so the same code runs in
//! Swift, Kotlin and WebAssembly hosts through the conformance suite.
//!
//! B040 adds [`navigation`] (pane identity rules, link groups, per-pane history and
//! transaction-scoped propagation) and [`migration`] (the ADR 0003 schema migrations,
//! starting with 1.0 → 1.1).
//!
//! B070 adds the workspace shell of specs/22: [`shell`] (sessions of parallel workspaces,
//! numbered columns of tabs, workspace schema 2.0 and its migration from 1.x),
//! [`commands`] (the one command registry, `apply` with undo, the [`commands::Shell`] that
//! holds the stacks) and [`palette`] (the sectioned, ranked palette query).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub mod commands;
pub mod migration;
pub mod navigation;
pub mod palette;
pub mod shell;

pub use migration::{
    MigrationError, STEPS, Step, migrate_workspace, migrate_workspace_to_2,
    migrate_workspace_to_current,
};
pub use navigation::{
    NavigationError, NavigationEvent, NavigationKind, PANE_ID_MAX_LEN, PaneHistory,
    PaneIdentityError, Propagation, ReferenceHistoryEntry, Visit, WorkspaceNavigation,
    check_pane_identity, followers, followers_among, is_pane_id, link_groups,
};

/// The crate's package name, as Cargo records it.
#[must_use]
pub fn crate_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

/// The workspace schema versions this crate reads: 1.0 (panes only) and 1.1 (B062: scroll
/// anchors and layout profiles). A 1.0 file opens on every class through the pane order.
pub const SCHEMA_VERSIONS: [&str; 2] = ["1.0", "1.1"];

/// The schema version [`with_profile`] writes, since profiles exist only from 1.1.
pub const CURRENT_SCHEMA_VERSION: &str = "1.1";

/// Width of a reading pane below which text would have to shrink, at standard text sizes.
/// B004 rendered 341-pt panes at 1024 points carrying 31 Latin characters per line
/// (docs/evidence/B004.md); specs/12 gives 300–340 CSS px as the content-derived minimum.
pub const PANE_MINIMUM_STANDARD_PT: f64 = 340.0;

/// The same minimum at accessibility text sizes (B004 decision 3: 1024 points shows two
/// panes plus a tab at accessibility L). A prototype number, not a measured one.
pub const PANE_MINIMUM_ACCESSIBILITY_PT: f64 = 480.0;

/// The prose measure a single column is capped at, at standard text sizes (B004: 640 points
/// is about 65 Latin characters in the serif; specs/16 asks for 55–75).
pub const MEASURE_CAP_STANDARD_PT: f64 = 640.0;

/// The measure cap at accessibility sizes: the standard cap scaled by the same ratio as the
/// pane minimum (640 × 480 ÷ 340, rounded). Not measured; the host's scaled metric wins.
pub const MEASURE_CAP_ACCESSIBILITY_PT: f64 = 900.0;

/// What a pane shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneKind {
    /// A Bible edition, original-language text, commentary or other work.
    Reading,
    /// The notes dock pinned as a pane.
    Notes,
    /// A research thread.
    Research,
    /// A lexicon or word-study pane.
    Lexicon,
}

/// Where a pane is scrolled to: the segment at the top of the viewport and the scalar offset
/// within it. Lives on the pane, never in a profile.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScrollAnchor {
    /// The segment at the top of the viewport.
    pub segment_id: String,
    /// Offset into the segment, in scalar values.
    pub scalar: u32,
}

/// One pane of a workspace, as schemas/workspace.schema.json stores it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pane {
    /// Stable id within the workspace.
    pub id: String,
    /// What the pane shows.
    pub kind: PaneKind,
    /// The resource (edition, work, note or thread id).
    pub content_id: String,
    /// The resource revision the pane was saved against.
    pub content_revision: String,
    /// The local passage in the resource's own versification, when it has one.
    pub reference: Option<String>,
    /// The reference-linked group the pane belongs to, or `None` when pinned or following.
    pub link_group: Option<String>,
    /// Independent of every other pane's navigation.
    pub pinned: bool,
    /// Width share when every pane is visible (schema 1.0); a profile's widths override it.
    pub width_fraction: f64,
    /// Scroll position (schema 1.1); `None` means the top of the passage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll_anchor: Option<ScrollAnchor>,
}

/// The arrangement of a workspace's panes on one device class: which panes are shown, in
/// what order, which are tabs, optional widths, the notes dock and collapsed link groups.
/// Every pane of the workspace appears exactly once in `visible_pane_ids` plus `tab_order`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutProfile {
    /// Panes shown on this class, in visual order; the visible members of a collapsed link
    /// group share one column. At most the class's column limit.
    pub visible_pane_ids: Vec<String>,
    /// The remaining panes, in tab order.
    pub tab_order: Vec<String>,
    /// Width fractions for the shown panes, summing to one; empty means equal columns.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub widths: BTreeMap<String, f64>,
    /// Whether the notes dock is open on this class.
    pub notes_dock_open: bool,
    /// Link groups whose visible members stack in one column.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collapsed_link_groups: Vec<String>,
}

/// One optional profile per device class (the `layout_profiles` object of schema 1.1).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LayoutProfiles {
    /// Under 700 points: phones, Slide Over, narrow Split View.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact: Option<LayoutProfile>,
    /// 700–1399 points: iPads, two-thirds Split View, small laptop windows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regular: Option<LayoutProfile>,
    /// 1400–2399 points: laptops and external displays at 2×.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wide: Option<LayoutProfile>,
    /// 2400 points and above: Studio Display, Pro Display XDR, 3440 and 5120 px at 1×.
    #[serde(
        default,
        rename = "ultra-wide",
        skip_serializing_if = "Option::is_none"
    )]
    pub ultra_wide: Option<LayoutProfile>,
}

impl LayoutProfiles {
    /// The stored profile for a class, if any.
    #[must_use]
    pub fn get(&self, class: DeviceClass) -> Option<&LayoutProfile> {
        match class {
            DeviceClass::Compact => self.compact.as_ref(),
            DeviceClass::Regular => self.regular.as_ref(),
            DeviceClass::Wide => self.wide.as_ref(),
            DeviceClass::UltraWide => self.ultra_wide.as_ref(),
        }
    }

    /// Replaces one class's profile and nothing else.
    pub fn set(&mut self, class: DeviceClass, profile: LayoutProfile) {
        let slot = match class {
            DeviceClass::Compact => &mut self.compact,
            DeviceClass::Regular => &mut self.regular,
            DeviceClass::Wide => &mut self.wide,
            DeviceClass::UltraWide => &mut self.ultra_wide,
        };
        *slot = Some(profile);
    }
}

/// A saved reading workspace (schemas/workspace.schema.json, 1.0 or 1.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    /// `"1.0"` or `"1.1"`.
    pub schema_version: String,
    /// Synthetic data, never a user's workspace.
    pub fixture_only: bool,
    /// Stable workspace id.
    pub id: String,
    /// Display title.
    pub title: String,
    /// The panes, in the order they were created or arranged; the fallback priority order.
    pub panes: Vec<Pane>,
    /// The note shown in the notes dock, if any.
    pub notes_dock_id: Option<String>,
    /// The active pane; always shown by every derived layout.
    pub active_pane_id: String,
    /// Per-class profiles; `None` for a 1.0 file, `Some` (possibly empty) for 1.1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_profiles: Option<LayoutProfiles>,
}

impl Workspace {
    /// The stored profile for a class, if the workspace has one.
    #[must_use]
    pub fn profile(&self, class: DeviceClass) -> Option<&LayoutProfile> {
        self.layout_profiles.as_ref().and_then(|p| p.get(class))
    }

    /// The pane with this id.
    #[must_use]
    pub fn pane(&self, id: &str) -> Option<&Pane> {
        self.panes.iter().find(|p| p.id == id)
    }

    /// Every pane id, in pane order.
    #[must_use]
    pub fn pane_ids(&self) -> Vec<String> {
        self.panes.iter().map(|p| p.id.clone()).collect()
    }
}

/// The four layout classes of specs/18. The platform's own size class decides where one
/// exists (an iPhone is compact in every orientation); [`DeviceClass::for_width_pt`] decides
/// elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceClass {
    /// Under 700 points; one column.
    Compact,
    /// 700–1399 points; up to three columns.
    Regular,
    /// 1400–2399 points; up to six columns.
    Wide,
    /// 2400 points and above; up to six columns plus collapsed group columns.
    UltraWide,
}

impl DeviceClass {
    /// Every class, narrowest first.
    pub const ALL: [DeviceClass; 4] = [
        DeviceClass::Compact,
        DeviceClass::Regular,
        DeviceClass::Wide,
        DeviceClass::UltraWide,
    ];

    /// The class a width selects when the platform has no size class of its own. The
    /// thresholds are the smallest widths holding two, four and seven 340-pt panes, rounded
    /// up for chrome: prototype defaults in the sense of specs/12, not immutable breakpoints.
    #[must_use]
    pub fn for_width_pt(width_pt: f64) -> DeviceClass {
        if width_pt < 700.0 {
            DeviceClass::Compact
        } else if width_pt < 1400.0 {
            DeviceClass::Regular
        } else if width_pt < 2400.0 {
            DeviceClass::Wide
        } else {
            DeviceClass::UltraWide
        }
    }

    /// The most columns the class shows; a collapsed link group is one column.
    #[must_use]
    pub fn max_columns(self) -> usize {
        match self {
            DeviceClass::Compact => 1,
            DeviceClass::Regular => 3,
            DeviceClass::Wide | DeviceClass::UltraWide => 6,
        }
    }

    /// Where a class without a stored profile borrows its order from, nearest first
    /// (specs/18 rule 2).
    #[must_use]
    pub fn fallback_order(self) -> [DeviceClass; 3] {
        match self {
            DeviceClass::Compact => [
                DeviceClass::Regular,
                DeviceClass::Wide,
                DeviceClass::UltraWide,
            ],
            DeviceClass::Regular => [
                DeviceClass::Wide,
                DeviceClass::Compact,
                DeviceClass::UltraWide,
            ],
            DeviceClass::Wide => [
                DeviceClass::UltraWide,
                DeviceClass::Regular,
                DeviceClass::Compact,
            ],
            DeviceClass::UltraWide => [
                DeviceClass::Wide,
                DeviceClass::Regular,
                DeviceClass::Compact,
            ],
        }
    }

    /// The class name as the schema spells it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            DeviceClass::Compact => "compact",
            DeviceClass::Regular => "regular",
            DeviceClass::Wide => "wide",
            DeviceClass::UltraWide => "ultra-wide",
        }
    }
}

/// The reader's text size, as far as layout is concerned. Two tiers are measured (B004);
/// the intermediate Dynamic Type categories use the standard minimum until B014 measures
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextSize {
    /// Default through the largest non-accessibility size.
    Standard,
    /// The accessibility sizes.
    Accessibility,
}

impl TextSize {
    /// Narrowest useful reading pane at this size.
    #[must_use]
    pub fn pane_minimum_pt(self) -> f64 {
        match self {
            TextSize::Standard => PANE_MINIMUM_STANDARD_PT,
            TextSize::Accessibility => PANE_MINIMUM_ACCESSIBILITY_PT,
        }
    }

    /// Column width above which the text column is centred at the measure.
    #[must_use]
    pub fn measure_cap_pt(self) -> f64 {
        match self {
            TextSize::Standard => MEASURE_CAP_STANDARD_PT,
            TextSize::Accessibility => MEASURE_CAP_ACCESSIBILITY_PT,
        }
    }
}

/// Where the notes dock is in a derived layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotesDockPlacement {
    /// Closed.
    Hidden,
    /// A column of its own at the trailing edge (regular and wider, more than one slot).
    Pane,
    /// A sheet over the panes (compact, or a single slot).
    Sheet,
}

/// Where a derived layout's order came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfileSource {
    /// The workspace stores a profile for this class.
    Stored,
    /// Borrowed from the nearest stored class.
    DerivedFrom {
        /// The class whose profile supplied the order.
        class: DeviceClass,
    },
    /// No profile anywhere (a 1.0 file): the workspace's pane order.
    PaneOrder,
}

/// One column of a derived layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    /// The pane shown in the column.
    pub pane_id: String,
    /// Members of a collapsed link group stacked behind it as in-column tabs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stacked_pane_ids: Vec<String>,
    /// The shown pane's link group, if any.
    pub link_group: Option<String>,
    /// Column width in points.
    pub width_pt: f64,
    /// Whether the text column is centred at the measure cap because the column is wider.
    pub centered_measure: bool,
}

/// What a host renders for one workspace on one class at one width and text size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivedLayout {
    /// The class the layout was derived for.
    pub device_class: DeviceClass,
    /// The text size it was derived for.
    pub text_size: TextSize,
    /// Columns available to panes after the notes dock took its slot.
    pub slots: usize,
    /// The columns, leading to trailing.
    pub columns: Vec<Column>,
    /// Panes shown as tabs under the identity row, in order.
    pub tabs: Vec<String>,
    /// The notes dock.
    pub notes_dock: NotesDockPlacement,
    /// Width of the dock column when it is a pane, else zero.
    pub notes_dock_width_pt: f64,
    /// The active pane; always a column's shown pane.
    pub active_pane_id: String,
    /// The effective profile for this class: what the host saves if the reader saves.
    pub profile: LayoutProfile,
    /// Where the order came from.
    pub profile_source: ProfileSource,
}

impl DerivedLayout {
    /// Every pane id the layout places: shown, stacked and tabbed, in that order per column.
    #[must_use]
    pub fn placed_pane_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        for column in &self.columns {
            ids.push(column.pane_id.clone());
            ids.extend(column.stacked_pane_ids.iter().cloned());
        }
        ids.extend(self.tabs.iter().cloned());
        ids
    }

    /// The panes shown in columns (not stacked, not tabs), leading to trailing.
    #[must_use]
    pub fn shown_pane_ids(&self) -> Vec<String> {
        self.columns.iter().map(|c| c.pane_id.clone()).collect()
    }
}

/// Why a workspace or profile is rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LayoutError {
    /// A workspace needs at least one pane.
    #[error("workspace has no panes")]
    NoPanes,
    /// Two panes share an id.
    #[error("duplicate pane id {pane_id}")]
    DuplicatePane {
        /// The repeated id.
        pane_id: String,
    },
    /// The active pane is not a pane of the workspace.
    #[error("active pane {pane_id} is not in the workspace")]
    UnknownActivePane {
        /// The id named as active.
        pane_id: String,
    },
    /// A profile names a pane the workspace does not have.
    #[error("profile for {class} names unknown pane {pane_id}")]
    UnknownPane {
        /// The class whose profile is wrong.
        class: DeviceClass,
        /// The id.
        pane_id: String,
    },
    /// A profile leaves a pane out: the one thing a profile must never do.
    #[error("profile for {class} drops pane {pane_id}")]
    MissingPane {
        /// The class whose profile is wrong.
        class: DeviceClass,
        /// The dropped pane.
        pane_id: String,
    },
    /// A profile places a pane twice.
    #[error("profile for {class} places pane {pane_id} twice")]
    PanePlacedTwice {
        /// The class whose profile is wrong.
        class: DeviceClass,
        /// The repeated pane.
        pane_id: String,
    },
    /// A profile shows more columns than its class allows.
    #[error("profile for {class} shows {columns} columns; the class allows {max}")]
    TooManyColumns {
        /// The class whose profile is wrong.
        class: DeviceClass,
        /// Columns the profile would need.
        columns: usize,
        /// The class limit.
        max: usize,
    },
    /// A profile sizes a pane it does not show.
    #[error("profile for {class} sizes pane {pane_id}, which it does not show")]
    WidthForHiddenPane {
        /// The class whose profile is wrong.
        class: DeviceClass,
        /// The pane.
        pane_id: String,
    },
    /// A profile collapses a link group no pane belongs to.
    #[error("profile for {class} collapses link group {group}, which no pane belongs to")]
    UnknownLinkGroup {
        /// The class whose profile is wrong.
        class: DeviceClass,
        /// The group.
        group: String,
    },
}

impl std::fmt::Display for DeviceClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Checks the workspace invariants every function here relies on.
fn check_workspace(workspace: &Workspace) -> Result<(), LayoutError> {
    if workspace.panes.is_empty() {
        return Err(LayoutError::NoPanes);
    }
    let mut seen = std::collections::BTreeSet::new();
    for pane in &workspace.panes {
        if !seen.insert(pane.id.as_str()) {
            return Err(LayoutError::DuplicatePane {
                pane_id: pane.id.clone(),
            });
        }
    }
    if workspace.pane(&workspace.active_pane_id).is_none() {
        return Err(LayoutError::UnknownActivePane {
            pane_id: workspace.active_pane_id.clone(),
        });
    }
    Ok(())
}

/// Counts the columns a visible list needs: a collapsed group's members share one.
fn count_columns(workspace: &Workspace, profile: &LayoutProfile) -> usize {
    let mut seen_groups = std::collections::BTreeSet::new();
    let mut columns = 0;
    for id in &profile.visible_pane_ids {
        let group = workspace.pane(id).and_then(|p| p.link_group.as_deref());
        match group {
            Some(g) if profile.collapsed_link_groups.iter().any(|c| c == g) => {
                if seen_groups.insert(g) {
                    columns += 1;
                }
            }
            _ => columns += 1,
        }
    }
    columns
}

/// Checks that a profile is a complete, lossless arrangement of this workspace on this
/// class: every pane exactly once, within the column limit, widths only for shown panes,
/// collapsed groups that exist.
pub fn check_profile(
    workspace: &Workspace,
    class: DeviceClass,
    profile: &LayoutProfile,
) -> Result<(), LayoutError> {
    check_workspace(workspace)?;
    let mut placed = std::collections::BTreeSet::new();
    for id in profile.visible_pane_ids.iter().chain(&profile.tab_order) {
        if workspace.pane(id).is_none() {
            return Err(LayoutError::UnknownPane {
                class,
                pane_id: id.clone(),
            });
        }
        if !placed.insert(id.as_str()) {
            return Err(LayoutError::PanePlacedTwice {
                class,
                pane_id: id.clone(),
            });
        }
    }
    for pane in &workspace.panes {
        if !placed.contains(pane.id.as_str()) {
            return Err(LayoutError::MissingPane {
                class,
                pane_id: pane.id.clone(),
            });
        }
    }
    let columns = count_columns(workspace, profile);
    if columns > class.max_columns() {
        return Err(LayoutError::TooManyColumns {
            class,
            columns,
            max: class.max_columns(),
        });
    }
    for id in profile.widths.keys() {
        if !profile.visible_pane_ids.contains(id) {
            return Err(LayoutError::WidthForHiddenPane {
                class,
                pane_id: id.clone(),
            });
        }
    }
    for group in &profile.collapsed_link_groups {
        if !workspace
            .panes
            .iter()
            .any(|p| p.link_group.as_deref() == Some(group))
        {
            return Err(LayoutError::UnknownLinkGroup {
                class,
                group: group.clone(),
            });
        }
    }
    Ok(())
}

/// Saves one class's arrangement: a copy of the workspace with only that class's profile
/// replaced. Every pane, link group and scroll anchor, and every other class's profile, is
/// unchanged; the schema version becomes 1.1 because profiles exist only there.
pub fn with_profile(
    workspace: &Workspace,
    class: DeviceClass,
    profile: LayoutProfile,
) -> Result<Workspace, LayoutError> {
    check_profile(workspace, class, &profile)?;
    let mut saved = workspace.clone();
    saved
        .layout_profiles
        .get_or_insert_with(LayoutProfiles::default)
        .set(class, profile);
    saved.schema_version = CURRENT_SCHEMA_VERSION.to_owned();
    Ok(saved)
}

/// A stored profile reconciled with the current pane set: ids the workspace no longer has
/// are ignored, panes the profile does not know are appended as tabs. Returns the priority
/// order and how many leading entries the profile wants visible.
fn reconcile(workspace: &Workspace, profile: &LayoutProfile) -> (Vec<String>, usize) {
    let mut order: Vec<String> = Vec::with_capacity(workspace.panes.len());
    let push = |id: &str, order: &mut Vec<String>| {
        if workspace.pane(id).is_some() && !order.iter().any(|o| o == id) {
            order.push(id.to_owned());
        }
    };
    for id in &profile.visible_pane_ids {
        push(id, &mut order);
    }
    let visible = order.len();
    for id in &profile.tab_order {
        push(id, &mut order);
    }
    for pane in &workspace.panes {
        push(&pane.id, &mut order);
    }
    (order, visible)
}

/// Derives the layout for one class at one width and text size (specs/18, "Derivation
/// rules"). `width_pt` is the width available to panes: the window minus any side chrome.
/// Pure: the workspace is read, never changed; the result's `profile` is what a host saves
/// through [`with_profile`] if the reader saves.
pub fn derive_profile(
    workspace: &Workspace,
    device_class: DeviceClass,
    width_pt: f64,
    text_size: TextSize,
) -> Result<DerivedLayout, LayoutError> {
    check_workspace(workspace)?;

    // Rule 2: the priority order and how many of its leading panes may take a slot.
    let (order, visible_cap, source, template) = match workspace.profile(device_class) {
        Some(profile) => {
            let (order, visible) = reconcile(workspace, profile);
            // A profile whose shown panes were all closed elsewhere shows what fits.
            let cap = if visible == 0 { usize::MAX } else { visible };
            (order, cap, ProfileSource::Stored, Some(profile))
        }
        None => {
            let nearest = device_class
                .fallback_order()
                .into_iter()
                .find_map(|class| workspace.profile(class).map(|p| (class, p)));
            match nearest {
                Some((class, profile)) => {
                    let (order, _) = reconcile(workspace, profile);
                    (
                        order,
                        usize::MAX,
                        ProfileSource::DerivedFrom { class },
                        Some(profile),
                    )
                }
                None => (
                    workspace.pane_ids(),
                    usize::MAX,
                    ProfileSource::PaneOrder,
                    None,
                ),
            }
        }
    };
    let notes_dock_open = template.is_some_and(|p| p.notes_dock_open);
    let collapsed: Vec<String> = template
        .map(|p| p.collapsed_link_groups.clone())
        .unwrap_or_default();

    // Rule 1: slots from the width, the text size and the class; the dock takes one.
    let width = if width_pt.is_finite() {
        width_pt.max(0.0)
    } else {
        0.0
    };
    let fitting = (width / text_size.pane_minimum_pt()).floor();
    let fitting = if fitting >= 1.0 { fitting as usize } else { 1 };
    let total_slots = fitting.min(device_class.max_columns());
    let (notes_dock, notes_dock_width_pt, slots) = if !notes_dock_open {
        (NotesDockPlacement::Hidden, 0.0, total_slots)
    } else if device_class == DeviceClass::Compact || total_slots == 1 {
        (NotesDockPlacement::Sheet, 0.0, total_slots)
    } else {
        (
            NotesDockPlacement::Pane,
            width / total_slots as f64,
            total_slots - 1,
        )
    };
    let available = width - notes_dock_width_pt;

    // Rule 3: columns, stacking collapsed groups, the rest as tabs.
    let mut columns: Vec<Column> = Vec::new();
    let mut tabs: Vec<String> = Vec::new();
    for (index, id) in order.iter().enumerate() {
        let group = workspace.pane(id).and_then(|p| p.link_group.clone());
        if index >= visible_cap {
            tabs.push(id.clone());
            continue;
        }
        if let Some(g) = group
            .as_deref()
            .filter(|g| collapsed.iter().any(|c| c == g))
            && let Some(column) = columns
                .iter_mut()
                .find(|c| c.link_group.as_deref() == Some(g))
        {
            column.stacked_pane_ids.push(id.clone());
            continue;
        }
        if columns.len() < slots {
            columns.push(Column {
                pane_id: id.clone(),
                stacked_pane_ids: Vec::new(),
                link_group: group,
                width_pt: 0.0,
                centered_measure: false,
            });
        } else {
            tabs.push(id.clone());
        }
    }

    // Rule 4: the active pane is always a column's shown pane.
    let active = workspace.active_pane_id.clone();
    let active_group = workspace.pane(&active).and_then(|p| p.link_group.clone());
    if !columns.iter().any(|c| c.pane_id == active) {
        if let Some(column) = columns
            .iter_mut()
            .find(|c| c.stacked_pane_ids.contains(&active))
        {
            column.stacked_pane_ids.retain(|s| *s != active);
            let shown = std::mem::replace(&mut column.pane_id, active.clone());
            column.stacked_pane_ids.insert(0, shown);
        } else {
            tabs.retain(|t| *t != active);
            let group_column = active_group
                .as_deref()
                .filter(|g| collapsed.iter().any(|c| c == g))
                .and_then(|g| {
                    columns
                        .iter()
                        .position(|c| c.link_group.as_deref() == Some(g))
                });
            match group_column {
                // Its collapsed group already has a column: the active pane is shown there
                // and the pane it replaces stays in the stack.
                Some(position) => {
                    let column = &mut columns[position];
                    let shown = std::mem::replace(&mut column.pane_id, active.clone());
                    column.stacked_pane_ids.insert(0, shown);
                }
                // Otherwise it takes the last column without a stack (the last column if
                // every column has one, whose stack then stays put) and the displaced pane
                // becomes the first tab.
                None => {
                    let position = columns
                        .iter()
                        .rposition(|c| c.stacked_pane_ids.is_empty())
                        .unwrap_or(columns.len() - 1);
                    let column = &mut columns[position];
                    let shown = std::mem::replace(&mut column.pane_id, active.clone());
                    column.link_group = active_group;
                    tabs.insert(0, shown);
                }
            }
        }
    }

    // Rule 5: widths from the profile when it sizes every shown pane, else equal.
    let shown: Vec<String> = columns.iter().map(|c| c.pane_id.clone()).collect();
    let stored_widths = template.map(|p| &p.widths);
    let fractions: Vec<f64> = match stored_widths {
        Some(widths) if !shown.is_empty() && shown.iter().all(|id| widths.contains_key(id)) => {
            let total: f64 = shown.iter().map(|id| widths[id]).sum();
            shown.iter().map(|id| widths[id] / total).collect()
        }
        _ => vec![1.0 / shown.len().max(1) as f64; shown.len()],
    };
    let cap = text_size.measure_cap_pt();
    let mut widths = BTreeMap::new();
    for (column, fraction) in columns.iter_mut().zip(fractions) {
        column.width_pt = available * fraction;
        column.centered_measure = column.width_pt > cap;
        widths.insert(column.pane_id.clone(), fraction);
    }

    // Rule 6: the effective profile for this class.
    let mut visible_pane_ids = Vec::new();
    for column in &columns {
        visible_pane_ids.push(column.pane_id.clone());
        visible_pane_ids.extend(column.stacked_pane_ids.iter().cloned());
    }
    let profile = LayoutProfile {
        visible_pane_ids,
        tab_order: tabs.clone(),
        widths,
        notes_dock_open,
        collapsed_link_groups: collapsed,
    };
    debug_assert!(check_profile(workspace, device_class, &profile).is_ok());

    Ok(DerivedLayout {
        device_class,
        text_size,
        slots,
        columns,
        tabs,
        notes_dock,
        notes_dock_width_pt,
        active_pane_id: active,
        profile,
        profile_source: source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: &str, kind: PaneKind, group: Option<&str>, anchor: Option<(&str, u32)>) -> Pane {
        Pane {
            id: id.to_owned(),
            kind,
            content_id: format!("fixture:{id}"),
            content_revision: "0".to_owned(),
            reference: Some("fixture:passage:1".to_owned()),
            link_group: group.map(str::to_owned),
            pinned: group.is_none(),
            width_fraction: 0.2,
            scroll_anchor: anchor.map(|(segment, scalar)| ScrollAnchor {
                segment_id: segment.to_owned(),
                scalar,
            }),
        }
    }

    fn ids(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    /// The first acceptance addition of specs/18: five panes in two link groups as a Mac
    /// saved them, with the wide profile only.
    fn mac_workspace() -> Workspace {
        let mut widths = BTreeMap::new();
        for (id, w) in [
            ("p1", 0.22),
            ("p2", 0.22),
            ("p3", 0.22),
            ("p4", 0.2),
            ("p5", 0.14),
        ] {
            widths.insert(id.to_owned(), w);
        }
        Workspace {
            schema_version: "1.1".to_owned(),
            fixture_only: true,
            id: "fixture:workspace:mac".to_owned(),
            title: "Five panes, two groups".to_owned(),
            panes: vec![
                pane("p1", PaneKind::Reading, Some("1"), Some(("seg:1", 0))),
                pane("p2", PaneKind::Reading, Some("1"), Some(("seg:1", 12))),
                pane("p3", PaneKind::Reading, Some("1"), Some(("seg:1", 7))),
                pane("p4", PaneKind::Reading, Some("2"), Some(("seg:1-greek", 3))),
                pane("p5", PaneKind::Lexicon, Some("2"), None),
            ],
            notes_dock_id: Some("fixture:note:1".to_owned()),
            active_pane_id: "p1".to_owned(),
            layout_profiles: Some(LayoutProfiles {
                wide: Some(LayoutProfile {
                    visible_pane_ids: ids(&["p1", "p2", "p3", "p4", "p5"]),
                    tab_order: Vec::new(),
                    widths,
                    notes_dock_open: false,
                    collapsed_link_groups: Vec::new(),
                }),
                ..LayoutProfiles::default()
            }),
        }
    }

    /// Nothing is lost: every pane placed once, and the panes (with their link groups and
    /// scroll anchors) are exactly what the workspace holds.
    fn assert_lossless(workspace: &Workspace, before: &Workspace, layout: &DerivedLayout) {
        let mut placed = layout.placed_pane_ids();
        placed.sort();
        let mut expected = workspace.pane_ids();
        expected.sort();
        assert_eq!(placed, expected, "every pane exactly once");
        assert_eq!(
            workspace, before,
            "derivation reads the workspace and never changes it"
        );
        for id in layout.placed_pane_ids() {
            let now = workspace.pane(&id).expect("placed pane exists");
            let then = before.pane(&id).expect("pane existed");
            assert_eq!(now.link_group, then.link_group);
            assert_eq!(now.scroll_anchor, then.scroll_anchor);
        }
        assert!(
            layout
                .columns
                .iter()
                .any(|c| c.pane_id == layout.active_pane_id),
            "the active pane is shown"
        );
        check_profile(workspace, layout.device_class, &layout.profile)
            .expect("effective profile is valid");
    }

    #[test]
    fn crate_is_part_of_the_openlogos_workspace() {
        assert!(crate_name().starts_with("openlogos-"));
    }

    #[test]
    fn device_classes_follow_the_width_thresholds() {
        assert_eq!(DeviceClass::for_width_pt(390.0), DeviceClass::Compact);
        assert_eq!(DeviceClass::for_width_pt(699.9), DeviceClass::Compact);
        assert_eq!(DeviceClass::for_width_pt(700.0), DeviceClass::Regular);
        assert_eq!(DeviceClass::for_width_pt(1376.0), DeviceClass::Regular);
        assert_eq!(DeviceClass::for_width_pt(1440.0), DeviceClass::Wide);
        assert_eq!(DeviceClass::for_width_pt(2399.0), DeviceClass::Wide);
        assert_eq!(DeviceClass::for_width_pt(2560.0), DeviceClass::UltraWide);
        assert_eq!(DeviceClass::for_width_pt(5120.0), DeviceClass::UltraWide);
        assert_eq!(
            serde_json::to_string(&DeviceClass::UltraWide).unwrap(),
            "\"ultra-wide\""
        );
    }

    #[test]
    fn mac_workspace_on_its_own_display_shows_all_five() {
        let ws = mac_workspace();
        let before = ws.clone();
        let layout = derive_profile(&ws, DeviceClass::Wide, 1710.0, TextSize::Standard).unwrap();
        assert_eq!(layout.slots, 5);
        assert_eq!(
            layout.shown_pane_ids(),
            ids(&["p1", "p2", "p3", "p4", "p5"])
        );
        assert!(layout.tabs.is_empty());
        assert_eq!(layout.profile_source, ProfileSource::Stored);
        // Stored widths are honoured and sum to the available width.
        let total: f64 = layout.columns.iter().map(|c| c.width_pt).sum();
        assert!((total - 1710.0).abs() < 1e-6);
        assert!((layout.columns[0].width_pt - 1710.0 * 0.22).abs() < 1e-6);
        assert_lossless(&ws, &before, &layout);
    }

    #[test]
    fn a_narrower_wide_display_turns_the_trailing_pane_into_the_first_tab() {
        let ws = mac_workspace();
        let layout = derive_profile(&ws, DeviceClass::Wide, 1440.0, TextSize::Standard).unwrap();
        assert_eq!(layout.slots, 4);
        assert_eq!(layout.shown_pane_ids(), ids(&["p1", "p2", "p3", "p4"]));
        assert_eq!(layout.tabs, ids(&["p5"]));
        // The stored widths cover every shown pane, so they are normalised over the four.
        assert!((layout.columns[0].width_pt - 1440.0 * 0.22 / 0.86).abs() < 1e-6);
        let total: f64 = layout.columns.iter().map(|c| c.width_pt).sum();
        assert!((total - 1440.0).abs() < 1e-6);
        assert_lossless(&ws, &mac_workspace(), &layout);
    }

    #[test]
    fn mac_workspace_opens_on_an_ipad_without_lost_panes_groups_or_anchors() {
        let ws = mac_workspace();
        let before = ws.clone();
        let layout = derive_profile(&ws, DeviceClass::Regular, 1024.0, TextSize::Standard).unwrap();
        assert_eq!(layout.slots, 3);
        assert_eq!(layout.shown_pane_ids(), ids(&["p1", "p2", "p3"]));
        assert_eq!(layout.tabs, ids(&["p4", "p5"]));
        assert_eq!(
            layout.profile_source,
            ProfileSource::DerivedFrom {
                class: DeviceClass::Wide
            }
        );
        assert_eq!(layout.notes_dock, NotesDockPlacement::Hidden);
        // The tabbed Greek text and lexicon keep group 2 and their scroll anchors.
        assert_eq!(ws.pane("p4").unwrap().link_group.as_deref(), Some("2"));
        assert_eq!(
            ws.pane("p4")
                .unwrap()
                .scroll_anchor
                .as_ref()
                .unwrap()
                .scalar,
            3
        );
        assert_eq!(ws.pane("p5").unwrap().link_group.as_deref(), Some("2"));
        assert!((layout.columns[1].width_pt - 1024.0 / 3.0).abs() < 1e-6);
        assert!(!layout.columns[1].centered_measure);
        assert_lossless(&ws, &before, &layout);
    }

    #[test]
    fn at_accessibility_sizes_the_ipad_shows_two_panes_plus_tabs() {
        let ws = mac_workspace();
        let layout =
            derive_profile(&ws, DeviceClass::Regular, 1024.0, TextSize::Accessibility).unwrap();
        assert_eq!(layout.slots, 2);
        assert_eq!(layout.shown_pane_ids(), ids(&["p1", "p2"]));
        assert_eq!(layout.tabs, ids(&["p3", "p4", "p5"]));
        assert_lossless(&ws, &mac_workspace(), &layout);
    }

    #[test]
    fn a_portrait_ipad_shows_two_panes() {
        let ws = mac_workspace();
        let layout = derive_profile(&ws, DeviceClass::Regular, 768.0, TextSize::Standard).unwrap();
        assert_eq!(layout.slots, 2);
        assert_eq!(layout.shown_pane_ids(), ids(&["p1", "p2"]));
        assert_lossless(&ws, &mac_workspace(), &layout);
    }

    #[test]
    fn a_thirteen_inch_ipad_in_landscape_is_capped_at_three_columns() {
        let ws = mac_workspace();
        let layout = derive_profile(&ws, DeviceClass::Regular, 1376.0, TextSize::Standard).unwrap();
        assert_eq!(layout.slots, 3);
        assert_eq!(layout.tabs, ids(&["p4", "p5"]));
    }

    #[test]
    fn mac_workspace_opens_on_an_iphone_as_the_active_pane_plus_four_tabs() {
        let ws = mac_workspace();
        let before = ws.clone();
        let layout = derive_profile(&ws, DeviceClass::Compact, 390.0, TextSize::Standard).unwrap();
        assert_eq!(layout.slots, 1);
        assert_eq!(layout.shown_pane_ids(), ids(&["p1"]));
        assert_eq!(layout.tabs, ids(&["p2", "p3", "p4", "p5"]));
        assert_eq!(
            layout.profile_source,
            ProfileSource::DerivedFrom {
                class: DeviceClass::Wide
            }
        );
        assert!((layout.columns[0].width_pt - 390.0).abs() < 1e-6);
        assert_lossless(&ws, &before, &layout);
    }

    #[test]
    fn the_active_pane_is_shown_even_when_the_order_makes_it_a_tab() {
        let mut ws = mac_workspace();
        ws.active_pane_id = "p4".to_owned();
        let layout = derive_profile(&ws, DeviceClass::Compact, 390.0, TextSize::Standard).unwrap();
        assert_eq!(layout.shown_pane_ids(), ids(&["p4"]));
        assert_eq!(
            layout.tabs,
            ids(&["p1", "p2", "p3", "p5"]),
            "the displaced pane leads the tabs"
        );
        assert_lossless(&ws, &ws.clone(), &layout);

        let layout = derive_profile(&ws, DeviceClass::Regular, 1024.0, TextSize::Standard).unwrap();
        assert_eq!(layout.shown_pane_ids(), ids(&["p1", "p2", "p4"]));
        assert_eq!(layout.tabs, ids(&["p3", "p5"]));
        assert_lossless(&ws, &ws.clone(), &layout);
    }

    #[test]
    fn an_ultra_wide_display_centres_the_measure() {
        let ws = mac_workspace();
        let layout =
            derive_profile(&ws, DeviceClass::UltraWide, 3440.0, TextSize::Standard).unwrap();
        assert_eq!(layout.slots, 6);
        assert_eq!(layout.columns.len(), 5, "only five panes exist");
        assert!(layout.tabs.is_empty());
        // 3440 × 0.22 = 757 pt is past the 640-pt measure; the 14% lexicon column (482 pt) is not.
        assert!((layout.columns[0].width_pt - 3440.0 * 0.22).abs() < 1e-6);
        assert!(layout.columns[0].centered_measure);
        assert!(!layout.columns[4].centered_measure);
        assert_lossless(&ws, &mac_workspace(), &layout);
    }

    #[test]
    fn a_collapsed_link_group_takes_one_column() {
        let mut ws = mac_workspace();
        let profile = ws.layout_profiles.as_mut().unwrap().wide.as_mut().unwrap();
        profile.collapsed_link_groups = ids(&["2"]);
        profile.widths.clear();
        let layout = derive_profile(&ws, DeviceClass::Wide, 1440.0, TextSize::Standard).unwrap();
        assert_eq!(layout.slots, 4);
        assert_eq!(layout.shown_pane_ids(), ids(&["p1", "p2", "p3", "p4"]));
        assert_eq!(layout.columns[3].stacked_pane_ids, ids(&["p5"]));
        assert!(
            layout.tabs.is_empty(),
            "all five panes are visible in four columns"
        );
        assert_eq!(
            layout.profile.visible_pane_ids,
            ids(&["p1", "p2", "p3", "p4", "p5"])
        );
        assert_lossless(&ws, &ws.clone(), &layout);

        // The active pane inside the stack becomes the column's shown pane.
        ws.active_pane_id = "p5".to_owned();
        let layout = derive_profile(&ws, DeviceClass::Wide, 1440.0, TextSize::Standard).unwrap();
        assert_eq!(layout.columns[3].pane_id, "p5");
        assert_eq!(layout.columns[3].stacked_pane_ids, ids(&["p4"]));
        assert_lossless(&ws, &ws.clone(), &layout);
    }

    #[test]
    fn the_notes_dock_takes_a_slot_on_regular_and_is_a_sheet_on_compact() {
        let mut ws = mac_workspace();
        ws.layout_profiles
            .as_mut()
            .unwrap()
            .wide
            .as_mut()
            .unwrap()
            .notes_dock_open = true;
        let layout = derive_profile(&ws, DeviceClass::Regular, 1024.0, TextSize::Standard).unwrap();
        assert_eq!(layout.notes_dock, NotesDockPlacement::Pane);
        assert_eq!(layout.slots, 2);
        assert_eq!(layout.shown_pane_ids(), ids(&["p1", "p2"]));
        assert!((layout.notes_dock_width_pt - 1024.0 / 3.0).abs() < 1e-6);
        assert!((layout.columns[0].width_pt - 1024.0 / 3.0).abs() < 1e-6);
        assert_lossless(&ws, &ws.clone(), &layout);

        let layout = derive_profile(&ws, DeviceClass::Compact, 390.0, TextSize::Standard).unwrap();
        assert_eq!(layout.notes_dock, NotesDockPlacement::Sheet);
        assert_eq!(layout.slots, 1);
        assert!((layout.notes_dock_width_pt).abs() < 1e-9);

        // A single slot on a regular class stays with the panes; the dock is a sheet.
        let layout =
            derive_profile(&ws, DeviceClass::Regular, 700.0, TextSize::Accessibility).unwrap();
        assert_eq!(layout.notes_dock, NotesDockPlacement::Sheet);
        assert_eq!(layout.slots, 1);
    }

    #[test]
    fn a_stored_profile_is_honoured_even_when_the_width_would_hold_more() {
        let ws = mac_workspace();
        let regular = LayoutProfile {
            visible_pane_ids: ids(&["p1", "p4"]),
            tab_order: ids(&["p2", "p3", "p5"]),
            widths: BTreeMap::new(),
            notes_dock_open: false,
            collapsed_link_groups: Vec::new(),
        };
        let saved = with_profile(&ws, DeviceClass::Regular, regular).unwrap();
        let layout =
            derive_profile(&saved, DeviceClass::Regular, 1210.0, TextSize::Standard).unwrap();
        assert_eq!(layout.slots, 3);
        assert_eq!(layout.shown_pane_ids(), ids(&["p1", "p4"]));
        assert_eq!(layout.tabs, ids(&["p2", "p3", "p5"]));
        assert_eq!(layout.profile_source, ProfileSource::Stored);
        assert_lossless(&saved, &saved.clone(), &layout);
    }

    #[test]
    fn saving_the_ipad_arrangement_leaves_the_mac_profile_and_the_panes_untouched() {
        let ws = mac_workspace();
        let ipad = derive_profile(&ws, DeviceClass::Regular, 1024.0, TextSize::Standard).unwrap();
        let saved = with_profile(&ws, DeviceClass::Regular, ipad.profile.clone()).unwrap();
        assert_eq!(saved.panes, ws.panes);
        assert_eq!(saved.active_pane_id, ws.active_pane_id);
        let before = ws.layout_profiles.as_ref().unwrap();
        let after = saved.layout_profiles.as_ref().unwrap();
        assert_eq!(
            after.wide, before.wide,
            "the Mac's profile is byte for byte what it was"
        );
        assert_eq!(after.compact, None);
        assert_eq!(after.ultra_wide, None);
        assert_eq!(
            after.regular.as_ref().unwrap().visible_pane_ids,
            ids(&["p1", "p2", "p3"])
        );
        assert_eq!(
            after.regular.as_ref().unwrap().tab_order,
            ids(&["p4", "p5"])
        );
        // And the saved arrangement reopens identically.
        let again =
            derive_profile(&saved, DeviceClass::Regular, 1024.0, TextSize::Standard).unwrap();
        assert_eq!(again.shown_pane_ids(), ipad.shown_pane_ids());
        assert_eq!(again.tabs, ipad.tabs);
        assert_eq!(again.profile_source, ProfileSource::Stored);
        // Saving on the phone afterwards leaves both the Mac and the iPad entries alone.
        let phone =
            derive_profile(&saved, DeviceClass::Compact, 390.0, TextSize::Standard).unwrap();
        let saved_twice = with_profile(&saved, DeviceClass::Compact, phone.profile).unwrap();
        let after_twice = saved_twice.layout_profiles.as_ref().unwrap();
        assert_eq!(after_twice.wide, before.wide);
        assert_eq!(after_twice.regular, after.regular);
        assert_eq!(saved_twice.panes, ws.panes);
    }

    #[test]
    fn a_profile_that_loses_a_pane_is_rejected() {
        let ws = mac_workspace();
        let dropped = LayoutProfile {
            visible_pane_ids: ids(&["p1", "p2", "p3"]),
            tab_order: ids(&["p4"]),
            widths: BTreeMap::new(),
            notes_dock_open: false,
            collapsed_link_groups: Vec::new(),
        };
        assert_eq!(
            with_profile(&ws, DeviceClass::Regular, dropped).unwrap_err(),
            LayoutError::MissingPane {
                class: DeviceClass::Regular,
                pane_id: "p5".to_owned()
            }
        );
        let twice = LayoutProfile {
            visible_pane_ids: ids(&["p1", "p2", "p3"]),
            tab_order: ids(&["p4", "p5", "p1"]),
            widths: BTreeMap::new(),
            notes_dock_open: false,
            collapsed_link_groups: Vec::new(),
        };
        assert_eq!(
            with_profile(&ws, DeviceClass::Regular, twice).unwrap_err(),
            LayoutError::PanePlacedTwice {
                class: DeviceClass::Regular,
                pane_id: "p1".to_owned()
            }
        );
        let unknown = LayoutProfile {
            visible_pane_ids: ids(&["p1", "p2", "p9"]),
            tab_order: ids(&["p3", "p4", "p5"]),
            widths: BTreeMap::new(),
            notes_dock_open: false,
            collapsed_link_groups: Vec::new(),
        };
        assert_eq!(
            with_profile(&ws, DeviceClass::Regular, unknown).unwrap_err(),
            LayoutError::UnknownPane {
                class: DeviceClass::Regular,
                pane_id: "p9".to_owned()
            }
        );
        let crowded = LayoutProfile {
            visible_pane_ids: ids(&["p1", "p2"]),
            tab_order: ids(&["p3", "p4", "p5"]),
            widths: BTreeMap::new(),
            notes_dock_open: false,
            collapsed_link_groups: Vec::new(),
        };
        assert_eq!(
            with_profile(&ws, DeviceClass::Compact, crowded.clone()).unwrap_err(),
            LayoutError::TooManyColumns {
                class: DeviceClass::Compact,
                columns: 2,
                max: 1
            }
        );
        // The same two panes in one collapsed group are one column.
        let stacked = LayoutProfile {
            collapsed_link_groups: ids(&["1"]),
            ..crowded
        };
        assert!(with_profile(&ws, DeviceClass::Compact, stacked).is_ok());
        let mut sized = LayoutProfile {
            visible_pane_ids: ids(&["p1"]),
            tab_order: ids(&["p2", "p3", "p4", "p5"]),
            widths: BTreeMap::new(),
            notes_dock_open: false,
            collapsed_link_groups: Vec::new(),
        };
        sized.widths.insert("p2".to_owned(), 1.0);
        assert_eq!(
            with_profile(&ws, DeviceClass::Compact, sized).unwrap_err(),
            LayoutError::WidthForHiddenPane {
                class: DeviceClass::Compact,
                pane_id: "p2".to_owned()
            }
        );
        let no_group = LayoutProfile {
            visible_pane_ids: ids(&["p1"]),
            tab_order: ids(&["p2", "p3", "p4", "p5"]),
            widths: BTreeMap::new(),
            notes_dock_open: false,
            collapsed_link_groups: ids(&["9"]),
        };
        assert_eq!(
            with_profile(&ws, DeviceClass::Compact, no_group).unwrap_err(),
            LayoutError::UnknownLinkGroup {
                class: DeviceClass::Compact,
                group: "9".to_owned()
            }
        );
    }

    #[test]
    fn a_workspace_without_profiles_derives_from_its_pane_order() {
        let mut ws = mac_workspace();
        ws.schema_version = "1.0".to_owned();
        ws.layout_profiles = None;
        for pane in &mut ws.panes {
            pane.scroll_anchor = None;
        }
        let layout = derive_profile(&ws, DeviceClass::Regular, 1024.0, TextSize::Standard).unwrap();
        assert_eq!(layout.profile_source, ProfileSource::PaneOrder);
        assert_eq!(layout.shown_pane_ids(), ids(&["p1", "p2", "p3"]));
        assert_eq!(layout.tabs, ids(&["p4", "p5"]));
        assert_lossless(&ws, &ws.clone(), &layout);
        // Saving once writes the regular entry and moves the file to 1.1.
        let saved = with_profile(&ws, DeviceClass::Regular, layout.profile).unwrap();
        assert_eq!(saved.schema_version, "1.1");
        assert!(saved.layout_profiles.as_ref().unwrap().regular.is_some());
        assert!(saved.layout_profiles.as_ref().unwrap().wide.is_none());
    }

    #[test]
    fn a_stored_profile_is_reconciled_with_panes_added_or_closed_elsewhere() {
        let mut ws = mac_workspace();
        // A pane the Mac profile does not know, added on another device, becomes a tab.
        ws.panes.push(pane("p6", PaneKind::Reading, None, None));
        // A pane the Mac profile names but the workspace no longer has is ignored.
        ws.panes.retain(|p| p.id != "p3");
        let layout = derive_profile(&ws, DeviceClass::Wide, 1710.0, TextSize::Standard).unwrap();
        assert_eq!(layout.shown_pane_ids(), ids(&["p1", "p2", "p4", "p5"]));
        assert_eq!(layout.tabs, ids(&["p6"]));
        assert_lossless(&ws, &ws.clone(), &layout);
    }

    #[test]
    fn malformed_workspaces_are_rejected() {
        let mut ws = mac_workspace();
        ws.active_pane_id = "nope".to_owned();
        assert_eq!(
            derive_profile(&ws, DeviceClass::Regular, 1024.0, TextSize::Standard).unwrap_err(),
            LayoutError::UnknownActivePane {
                pane_id: "nope".to_owned()
            }
        );
        let mut ws = mac_workspace();
        ws.panes.clear();
        assert_eq!(
            derive_profile(&ws, DeviceClass::Regular, 1024.0, TextSize::Standard).unwrap_err(),
            LayoutError::NoPanes
        );
        let mut ws = mac_workspace();
        ws.panes[1].id = "p1".to_owned();
        assert_eq!(
            derive_profile(&ws, DeviceClass::Regular, 1024.0, TextSize::Standard).unwrap_err(),
            LayoutError::DuplicatePane {
                pane_id: "p1".to_owned()
            }
        );
        // Odd widths never panic: a zero or NaN width still shows the active pane.
        let ws = mac_workspace();
        let layout =
            derive_profile(&ws, DeviceClass::Regular, f64::NAN, TextSize::Standard).unwrap();
        assert_eq!(layout.slots, 1);
        assert_eq!(layout.shown_pane_ids(), ids(&["p1"]));
    }

    #[test]
    fn errors_and_layouts_serialize_with_a_kind_tag() {
        let error = LayoutError::MissingPane {
            class: DeviceClass::Regular,
            pane_id: "p5".to_owned(),
        };
        let json = serde_json::to_value(&error).unwrap();
        assert_eq!(json["kind"], "missing_pane");
        assert_eq!(json["class"], "regular");
        assert_eq!(error.to_string(), "profile for regular drops pane p5");
        let layout = derive_profile(
            &mac_workspace(),
            DeviceClass::Compact,
            390.0,
            TextSize::Standard,
        )
        .unwrap();
        let json = serde_json::to_value(&layout).unwrap();
        assert_eq!(json["profile_source"]["kind"], "derived_from");
        assert_eq!(json["profile_source"]["class"], "wide");
        assert_eq!(json["notes_dock"], "hidden");
        let back: DerivedLayout = serde_json::from_value(json).unwrap();
        assert_eq!(back, layout);
    }

    /// The example under `examples/` at the repository root, or `None` when the tree is
    /// filtered (the nix checks build from `crates/` alone).
    fn example(name: &str) -> Option<String> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/").to_owned() + name;
        match std::fs::read_to_string(&path) {
            Ok(text) => Some(text),
            Err(_) => {
                eprintln!("skipping {path}: not present in this source tree");
                None
            }
        }
    }

    #[test]
    fn the_schema_examples_parse_and_derive() {
        let (Some(current), Some(archived)) = (
            example("archive/workspace-1.1.json"),
            example("archive/workspace-1.0.json"),
        ) else {
            return;
        };
        let ws: Workspace = serde_json::from_str(&current).unwrap();
        assert_eq!(ws.schema_version, "1.1");
        assert_eq!(ws.panes.len(), 5);
        assert!(
            ws.profile(DeviceClass::Wide).is_some() && ws.profile(DeviceClass::Regular).is_some()
        );
        assert!(ws.profile(DeviceClass::Compact).is_none());
        let ipad = derive_profile(&ws, DeviceClass::Regular, 1024.0, TextSize::Standard).unwrap();
        assert_eq!(ipad.profile_source, ProfileSource::Stored);
        assert_eq!(ipad.shown_pane_ids(), ids(&["p1", "p2", "p3"]));
        let phone = derive_profile(&ws, DeviceClass::Compact, 390.0, TextSize::Standard).unwrap();
        assert_eq!(
            phone.profile_source,
            ProfileSource::DerivedFrom {
                class: DeviceClass::Regular
            }
        );
        assert_lossless(&ws, &ws.clone(), &phone);
        // Round trip keeps the example's shape (absent optional fields stay absent).
        let value: serde_json::Value = serde_json::from_str(&current).unwrap();
        assert_eq!(serde_json::to_value(&ws).unwrap(), value);

        let old: Workspace = serde_json::from_str(&archived).unwrap();
        assert_eq!(old.schema_version, "1.0");
        assert!(old.layout_profiles.is_none());
        assert!(old.panes.iter().all(|p| p.scroll_anchor.is_none()));
        let layout = derive_profile(&old, DeviceClass::Compact, 390.0, TextSize::Standard).unwrap();
        assert_eq!(layout.profile_source, ProfileSource::PaneOrder);
        assert_lossless(&old, &old.clone(), &layout);
        let value: serde_json::Value = serde_json::from_str(&archived).unwrap();
        assert_eq!(serde_json::to_value(&old).unwrap(), value);
    }
}
