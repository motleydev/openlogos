//! Pane identity, link groups and navigation history (specs/12-workspace-ux.md, "Linking and
//! navigation"; specs/03-data-model.md, Workspace).
//!
//! **Pane ids** are opaque tokens the host mints when a pane is created and never again:
//! not its position (panes reorder, close and reopen), not its resource (two panes may show
//! one edition, and a pane may change edition), and never reused inside a workspace, because
//! layout profiles and histories key on them and a stored profile tolerates ids the
//! workspace no longer has by ignoring them (specs/18 rule 2), which a reused id would
//! silently adopt. [`is_pane_id`] is the shape, [`check_pane_identity`] the invariants.
//!
//! **Link groups** are identified by their label string, compared by equality; a group
//! exists exactly as long as a pane names it, and the pane carries the membership
//! ([`crate::Pane::link_group`]), so no profile can own or lose it. A pinned pane is in no
//! group and never follows. [`link_groups`] lists them; [`followers`] says which panes a
//! deliberate navigation in one pane moves.
//!
//! **Navigation** is a [`NavigationEvent`] with a transaction id: the pane the reader acted in
//! is the driver for that event, its followers receive the same transaction, and a transaction
//! already applied is a no-op, which is how feedback loops end. A programmatic scroll (a
//! follower moving, a restore) is never a driver. Every pane has its own
//! [`PaneHistory`]; the workspace keeps a reference history across panes; Back acts on the
//! active pane's history ([`WorkspaceNavigation::back_target`]). None of this is persisted in
//! the workspace document: specs/12 lists passage and scroll anchor as persistent state, not
//! history, so the workspace schema is unchanged.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::{ScrollAnchor, Workspace};

/// Longest pane id accepted.
pub const PANE_ID_MAX_LEN: usize = 64;

/// Whether a string has the shape of a pane id: 1 to 64 characters of ASCII letters, digits,
/// `-`, `_`, `.` or `:`, starting with a letter or digit. The shape says nothing about how
/// the id was minted; [`check_pane_identity`] checks what can be checked after the fact.
#[must_use]
pub fn is_pane_id(id: &str) -> bool {
    let mut chars = id.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    id.len() <= PANE_ID_MAX_LEN
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
}

/// Why a workspace's pane identities are not acceptable.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PaneIdentityError {
    /// Not the shape of a pane id.
    #[error("`{pane_id}` is not a pane id")]
    BadShape {
        /// The id.
        pane_id: String,
    },
    /// Two panes share an id.
    #[error("duplicate pane id {pane_id}")]
    Duplicate {
        /// The id.
        pane_id: String,
    },
    /// A pane id equal to the pane's resource id: identity derived from content.
    #[error("pane id {pane_id} is its content id; a pane id is never derived from its resource")]
    DerivedFromResource {
        /// The id.
        pane_id: String,
    },
    /// A pinned pane that also names a link group.
    #[error("pane {pane_id} is pinned and in link group {group}")]
    PinnedInGroup {
        /// The pane.
        pane_id: String,
        /// The group it names.
        group: String,
    },
}

/// Checks the identity invariants of a workspace's panes: every id well formed and unique,
/// no id equal to its pane's content id, and no pinned pane inside a link group.
pub fn check_pane_identity(workspace: &Workspace) -> Result<(), PaneIdentityError> {
    let mut seen = BTreeSet::new();
    for pane in &workspace.panes {
        if !is_pane_id(&pane.id) {
            return Err(PaneIdentityError::BadShape {
                pane_id: pane.id.clone(),
            });
        }
        if !seen.insert(pane.id.as_str()) {
            return Err(PaneIdentityError::Duplicate {
                pane_id: pane.id.clone(),
            });
        }
        if pane.id == pane.content_id {
            return Err(PaneIdentityError::DerivedFromResource {
                pane_id: pane.id.clone(),
            });
        }
        if pane.pinned
            && let Some(group) = &pane.link_group
        {
            return Err(PaneIdentityError::PinnedInGroup {
                pane_id: pane.id.clone(),
                group: group.clone(),
            });
        }
    }
    Ok(())
}

/// Every link group of the workspace with its member panes in pane order. A group exists
/// exactly as long as a pane names it.
#[must_use]
pub fn link_groups(workspace: &Workspace) -> BTreeMap<String, Vec<String>> {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for pane in &workspace.panes {
        if let Some(group) = &pane.link_group
            && !pane.pinned
        {
            groups
                .entry(group.clone())
                .or_default()
                .push(pane.id.clone());
        }
    }
    groups
}

/// The panes a deliberate navigation in `driver` moves: the other, unpinned members of its
/// link group, in pane order. A pinned pane, a pane in no group and an unknown pane drive
/// nothing. Which passage each follower lands on is the host's reference mapping; a
/// follower with no mapping shows "unmapped here", never a guessed passage (specs/12).
#[must_use]
pub fn followers(workspace: &Workspace, driver: &str) -> Vec<String> {
    let Some(pane) = workspace.pane(driver) else {
        return Vec::new();
    };
    if pane.pinned {
        return Vec::new();
    }
    let Some(group) = &pane.link_group else {
        return Vec::new();
    };
    workspace
        .panes
        .iter()
        .filter(|p| p.id != driver && !p.pinned && p.link_group.as_ref() == Some(group))
        .map(|p| p.id.clone())
        .collect()
}

/// Where a pane was: the passage and the scroll anchor at that moment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Visit {
    /// The passage in the pane's own versification, if the resource has one.
    pub reference: Option<String>,
    /// The scroll anchor.
    pub scroll_anchor: Option<ScrollAnchor>,
    /// The transaction that brought the pane here.
    pub transaction_id: String,
    /// Whether this pane was the driver of that transaction (`false`: it followed).
    pub driven: bool,
}

/// One pane's own back and forward stacks. Independent of every other pane's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneHistory {
    /// Earlier visits, oldest first; the last one is where Back goes.
    pub back: Vec<Visit>,
    /// The current visit, once the pane has navigated.
    pub current: Option<Visit>,
    /// Visits undone by Back, most recently undone last; the last one is where Forward goes.
    pub forward: Vec<Visit>,
}

impl PaneHistory {
    /// Records a new visit: the current one moves to the back stack and the forward stack
    /// is cleared, as in a browser.
    pub fn visit(&mut self, visit: Visit) {
        if let Some(current) = self.current.take() {
            self.back.push(current);
        }
        self.current = Some(visit);
        self.forward.clear();
    }

    /// Where Back would go, without going.
    #[must_use]
    pub fn back_target(&self) -> Option<&Visit> {
        self.back.last()
    }

    /// Goes back one visit and returns the visit now current.
    pub fn go_back(&mut self) -> Option<&Visit> {
        let previous = self.back.pop()?;
        if let Some(current) = self.current.take() {
            self.forward.push(current);
        }
        self.current = Some(previous);
        self.current.as_ref()
    }

    /// Goes forward one visit and returns the visit now current.
    pub fn go_forward(&mut self) -> Option<&Visit> {
        let next = self.forward.pop()?;
        if let Some(current) = self.current.take() {
            self.back.push(current);
        }
        self.current = Some(next);
        self.current.as_ref()
    }
}

/// How a navigation came about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigationKind {
    /// The reader navigated or scrolled in this pane: it is the driver for the transaction.
    Deliberate,
    /// The pane was moved by code (a follower landing, a restore, a programmatic scroll):
    /// recorded in its history, never propagated.
    Programmatic,
}

/// One navigation in one pane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationEvent {
    /// Unique per deliberate action; a follower's landing reuses the driver's id.
    pub transaction_id: String,
    /// The pane the event happened in.
    pub pane_id: String,
    /// Deliberate or programmatic.
    pub kind: NavigationKind,
    /// Where the pane is now.
    pub reference: Option<String>,
    /// Its scroll anchor now.
    pub scroll_anchor: Option<ScrollAnchor>,
}

/// What a host does after applying a navigation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Propagation {
    /// The transaction.
    pub transaction_id: String,
    /// The driver.
    pub driver_pane_id: String,
    /// Panes to move under the same transaction id, via the host's reference mapping. Empty
    /// for a programmatic event, a pinned or ungrouped driver, or a repeated transaction.
    pub follower_pane_ids: Vec<String>,
    /// Whether the event was ignored because its transaction was already applied in this
    /// pane (a feedback echo).
    pub suppressed: bool,
}

/// Why a navigation could not be applied.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NavigationError {
    /// The pane is not in the workspace.
    #[error("pane {pane_id} is not in the workspace")]
    UnknownPane {
        /// The pane.
        pane_id: String,
    },
    /// An empty transaction id.
    #[error("navigation without a transaction id")]
    NoTransaction,
}

/// A reference-history entry across the workspace: which pane went where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceHistoryEntry {
    /// The driver.
    pub pane_id: String,
    /// The passage.
    pub reference: Option<String>,
    /// The transaction.
    pub transaction_id: String,
}

/// The navigation state of one open workspace: a history per pane, the workspace-level
/// reference history and the transactions already seen per pane. Runtime state; not part of
/// the workspace document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceNavigation {
    /// One history per pane id.
    pub histories: BTreeMap<String, PaneHistory>,
    /// Deliberate navigations across the workspace, oldest first.
    pub reference_history: Vec<ReferenceHistoryEntry>,
    /// `(transaction id, pane id)` pairs already applied.
    pub applied: BTreeSet<(String, String)>,
}

impl WorkspaceNavigation {
    /// Applies one navigation: records the visit in the pane's own history and, for a
    /// deliberate event, in the reference history, and names the followers to move. A
    /// transaction already applied to the pane is suppressed: nothing is recorded and
    /// nothing propagates, so a follower's landing can never re-drive the driver.
    pub fn apply(
        &mut self,
        workspace: &Workspace,
        event: &NavigationEvent,
    ) -> Result<Propagation, NavigationError> {
        if event.transaction_id.is_empty() {
            return Err(NavigationError::NoTransaction);
        }
        if workspace.pane(&event.pane_id).is_none() {
            return Err(NavigationError::UnknownPane {
                pane_id: event.pane_id.clone(),
            });
        }
        let key = (event.transaction_id.clone(), event.pane_id.clone());
        if !self.applied.insert(key) {
            return Ok(Propagation {
                transaction_id: event.transaction_id.clone(),
                driver_pane_id: event.pane_id.clone(),
                follower_pane_ids: Vec::new(),
                suppressed: true,
            });
        }
        let driven = event.kind == NavigationKind::Deliberate;
        self.histories
            .entry(event.pane_id.clone())
            .or_default()
            .visit(Visit {
                reference: event.reference.clone(),
                scroll_anchor: event.scroll_anchor.clone(),
                transaction_id: event.transaction_id.clone(),
                driven,
            });
        let follower_pane_ids = if driven {
            self.reference_history.push(ReferenceHistoryEntry {
                pane_id: event.pane_id.clone(),
                reference: event.reference.clone(),
                transaction_id: event.transaction_id.clone(),
            });
            followers(workspace, &event.pane_id)
        } else {
            Vec::new()
        };
        Ok(Propagation {
            transaction_id: event.transaction_id.clone(),
            driver_pane_id: event.pane_id.clone(),
            follower_pane_ids,
            suppressed: false,
        })
    }

    /// The history of one pane, empty if it never navigated.
    #[must_use]
    pub fn history(&self, pane_id: &str) -> PaneHistory {
        self.histories.get(pane_id).cloned().unwrap_or_default()
    }

    /// Back's target: the active pane and the visit it would return to. Back acts on the
    /// active pane only; other panes keep their place.
    #[must_use]
    pub fn back_target(&self, workspace: &Workspace) -> Option<(String, Visit)> {
        let active = workspace.active_pane_id.as_str();
        self.histories
            .get(active)
            .and_then(PaneHistory::back_target)
            .map(|visit| (active.to_owned(), visit.clone()))
    }

    /// Goes back in the active pane and returns the visit now current there. Going back is
    /// a programmatic move: it does not propagate to the pane's group.
    pub fn go_back(&mut self, workspace: &Workspace) -> Option<(String, Visit)> {
        let active = workspace.active_pane_id.clone();
        let visit = self.histories.get_mut(&active)?.go_back()?.clone();
        Some((active, visit))
    }

    /// Forgets the history of a pane that was closed. Its id is not reused.
    pub fn forget_pane(&mut self, pane_id: &str) {
        self.histories.remove(pane_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Pane, PaneKind};

    fn pane(id: &str, group: Option<&str>, pinned: bool) -> Pane {
        Pane {
            id: id.to_owned(),
            kind: PaneKind::Reading,
            content_id: format!("fixture:edition-{id}"),
            content_revision: "0".to_owned(),
            reference: Some("fixture:passage:1".to_owned()),
            link_group: group.map(str::to_owned),
            pinned,
            width_fraction: 1.0 / 3.0,
            scroll_anchor: None,
        }
    }

    /// A + B linked, C pinned: the specs/12 example.
    fn workspace() -> Workspace {
        Workspace {
            schema_version: "1.1".to_owned(),
            fixture_only: true,
            id: "fixture:workspace:nav".to_owned(),
            title: "A+B linked, C fixed".to_owned(),
            panes: vec![
                pane("a", Some("1"), false),
                pane("b", Some("1"), false),
                pane("c", None, true),
            ],
            notes_dock_id: None,
            active_pane_id: "a".to_owned(),
            layout_profiles: Some(Default::default()),
        }
    }

    fn event(tx: &str, pane_id: &str, kind: NavigationKind, reference: &str) -> NavigationEvent {
        NavigationEvent {
            transaction_id: tx.to_owned(),
            pane_id: pane_id.to_owned(),
            kind,
            reference: Some(reference.to_owned()),
            scroll_anchor: None,
        }
    }

    #[test]
    fn pane_id_shape() {
        for ok in ["p1", "a", "pane-7f3a", "01HZX.Y:Z_1", "A"] {
            assert!(is_pane_id(ok), "{ok}");
        }
        for bad in ["", "-p", "p 1", "p/1", "pané", &"x".repeat(65)] {
            assert!(!is_pane_id(bad), "{bad}");
        }
    }

    #[test]
    fn identity_invariants() {
        assert_eq!(check_pane_identity(&workspace()), Ok(()));
        let mut w = workspace();
        w.panes[1].id = "a".to_owned();
        assert_eq!(
            check_pane_identity(&w),
            Err(PaneIdentityError::Duplicate {
                pane_id: "a".into()
            })
        );
        let mut w = workspace();
        w.panes[0].id = w.panes[0].content_id.clone();
        assert!(matches!(
            check_pane_identity(&w),
            Err(PaneIdentityError::DerivedFromResource { .. })
        ));
        let mut w = workspace();
        w.panes[2].link_group = Some("1".to_owned());
        assert!(matches!(
            check_pane_identity(&w),
            Err(PaneIdentityError::PinnedInGroup { .. })
        ));
        let mut w = workspace();
        w.panes[0].id = "a b".to_owned();
        assert!(matches!(
            check_pane_identity(&w),
            Err(PaneIdentityError::BadShape { .. })
        ));
    }

    #[test]
    fn ids_survive_reordering_and_closing() {
        // Identity is the id, not the position: reversing the panes changes no id, and
        // closing one leaves the others' ids as they were.
        let mut w = workspace();
        let before = w.pane_ids();
        w.panes.reverse();
        let mut after = w.pane_ids();
        after.reverse();
        assert_eq!(before, after);
        w.panes.remove(0);
        assert_eq!(w.pane_ids(), vec!["b", "a"]);
        assert_eq!(check_pane_identity(&w), Ok(()));
    }

    #[test]
    fn link_groups_and_followers() {
        let w = workspace();
        let groups = link_groups(&w);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups["1"], vec!["a", "b"]);
        assert_eq!(followers(&w, "a"), vec!["b"]);
        assert_eq!(followers(&w, "b"), vec!["a"]);
        assert!(followers(&w, "c").is_empty(), "pinned drives nothing");
        assert!(followers(&w, "zz").is_empty());
        // A group identified by its label: renaming the label on one pane leaves it alone.
        let mut w = workspace();
        w.panes[1].link_group = Some("2".to_owned());
        assert!(followers(&w, "a").is_empty());
        assert_eq!(link_groups(&w).len(), 2);
        // A pinned pane that still carries a label is not a member.
        let mut w = workspace();
        w.panes[1].pinned = true;
        assert!(followers(&w, "a").is_empty());
        assert_eq!(link_groups(&w)["1"], vec!["a"]);
    }

    #[test]
    fn a_deliberate_navigation_names_followers_and_programmatic_does_not() {
        let w = workspace();
        let mut nav = WorkspaceNavigation::default();
        let p = nav
            .apply(&w, &event("t1", "a", NavigationKind::Deliberate, "John.3"))
            .unwrap();
        assert_eq!(p.follower_pane_ids, vec!["b"]);
        assert!(!p.suppressed);
        // The follower lands under the same transaction, programmatically.
        let q = nav
            .apply(
                &w,
                &event("t1", "b", NavigationKind::Programmatic, "John.3"),
            )
            .unwrap();
        assert!(q.follower_pane_ids.is_empty());
        assert!(!nav.history("b").current.unwrap().driven);
        assert!(nav.history("a").current.unwrap().driven);
        assert!(
            nav.history("c").current.is_none(),
            "the pinned pane did not move"
        );
        assert_eq!(nav.reference_history.len(), 1);
        assert_eq!(nav.reference_history[0].pane_id, "a");
    }

    #[test]
    fn a_repeated_transaction_is_suppressed() {
        // The follower's landing scrolls b; if the host reports that scroll as b's own event
        // under the same transaction, it must not drive a again.
        let w = workspace();
        let mut nav = WorkspaceNavigation::default();
        nav.apply(&w, &event("t1", "a", NavigationKind::Deliberate, "John.3"))
            .unwrap();
        nav.apply(
            &w,
            &event("t1", "b", NavigationKind::Programmatic, "John.3"),
        )
        .unwrap();
        let echo = nav
            .apply(&w, &event("t1", "b", NavigationKind::Deliberate, "John.3"))
            .unwrap();
        assert!(echo.suppressed);
        assert!(echo.follower_pane_ids.is_empty());
        assert_eq!(nav.history("b").back.len(), 0);
        assert_eq!(nav.reference_history.len(), 1);
        // A new transaction from b does drive a.
        let next = nav
            .apply(&w, &event("t2", "b", NavigationKind::Deliberate, "John.4"))
            .unwrap();
        assert_eq!(next.follower_pane_ids, vec!["a"]);
    }

    #[test]
    fn histories_are_independent_and_back_targets_the_active_pane() {
        let mut w = workspace();
        let mut nav = WorkspaceNavigation::default();
        for (tx, reference) in [("t1", "John.3"), ("t2", "John.4"), ("t3", "John.5")] {
            nav.apply(&w, &event(tx, "a", NavigationKind::Deliberate, reference))
                .unwrap();
            nav.apply(&w, &event(tx, "b", NavigationKind::Programmatic, reference))
                .unwrap();
        }
        nav.apply(&w, &event("t4", "c", NavigationKind::Deliberate, "Ps.23"))
            .unwrap();
        assert_eq!(nav.history("a").back.len(), 2);
        assert_eq!(nav.history("b").back.len(), 2);
        assert_eq!(nav.history("c").back.len(), 0);
        // Back in a goes to John.4 and leaves b and c where they are.
        let (pane_id, visit) = nav.back_target(&w).unwrap();
        assert_eq!(pane_id, "a");
        assert_eq!(visit.reference.as_deref(), Some("John.4"));
        let (_, visit) = nav.go_back(&w).unwrap();
        assert_eq!(visit.reference.as_deref(), Some("John.4"));
        assert_eq!(
            nav.history("b").current.unwrap().reference.as_deref(),
            Some("John.5")
        );
        assert_eq!(nav.history("a").forward.len(), 1);
        // Back with c active has nowhere to go.
        w.active_pane_id = "c".to_owned();
        assert!(nav.back_target(&w).is_none());
        assert!(nav.go_back(&w).is_none());
        // Forward in a returns to John.5.
        let mut a = nav.history("a");
        assert_eq!(a.go_forward().unwrap().reference.as_deref(), Some("John.5"));
        assert!(a.go_forward().is_none());
    }

    #[test]
    fn a_new_visit_clears_forward() {
        let mut h = PaneHistory::default();
        let v = |tx: &str| Visit {
            reference: Some(tx.to_owned()),
            scroll_anchor: None,
            transaction_id: tx.to_owned(),
            driven: true,
        };
        h.visit(v("1"));
        h.visit(v("2"));
        h.go_back();
        assert_eq!(h.forward.len(), 1);
        h.visit(v("3"));
        assert!(h.forward.is_empty());
        assert_eq!(h.back.len(), 1);
        assert_eq!(h.current.unwrap().transaction_id, "3");
    }

    #[test]
    fn unknown_pane_and_empty_transaction_are_errors() {
        let w = workspace();
        let mut nav = WorkspaceNavigation::default();
        assert_eq!(
            nav.apply(&w, &event("t1", "zz", NavigationKind::Deliberate, "x")),
            Err(NavigationError::UnknownPane {
                pane_id: "zz".into()
            })
        );
        assert_eq!(
            nav.apply(&w, &event("", "a", NavigationKind::Deliberate, "x")),
            Err(NavigationError::NoTransaction)
        );
        nav.apply(&w, &event("t1", "a", NavigationKind::Deliberate, "x"))
            .unwrap();
        nav.forget_pane("a");
        assert!(nav.history("a").current.is_none());
    }
}
