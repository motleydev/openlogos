//! The B040 contract fixtures: the workspace schema migration under ADR 0003
//! (`workspace-migration-1-0-to-1-1`) and the note-authority state machine
//! (`note-authority-transitions`), so Swift, Kotlin and JavaScript hosts can assert through
//! [`crate::run_all`] that a 1.0 workspace migrates to the 1.1 document the fixture expects
//! and derives the same layout before and after, and that every binding transition yields the
//! effect and the binding the fixture expects (specs/12-workspace-ux.md,
//! specs/13-obsidian-and-ink.md, docs/adr/0003-identity-and-schema-versioning.md).

use openlogos_notes::{BindingEvent, Effect, NoteBinding, apply};
use openlogos_workspaces::{DeviceClass, TextSize, Workspace, derive_profile, migrate_workspace};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Recorder;
use crate::layout::ExpectedLayout;

/// A layout both the original and the migrated document must derive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MigrationLayout {
    /// The class.
    pub device_class: DeviceClass,
    /// The width available to panes.
    pub width_pt: f64,
    /// The text size.
    pub text_size: TextSize,
    /// What both must derive.
    pub expected: ExpectedLayout,
}

/// One migration step over one document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MigrationCase {
    /// What the case shows.
    pub name: String,
    /// The version the input is at.
    pub from: String,
    /// The version wanted.
    pub to: String,
    /// The document, as its schema version stores it.
    pub input: Value,
    /// The document the migration must produce, byte for byte as JSON.
    pub expected: Value,
    /// Layouts the input and the output must both derive.
    #[serde(default)]
    pub layouts: Vec<MigrationLayout>,
}

/// A workspace migration fixture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceMigrationFixture {
    /// Fixture id (also the file name).
    pub id: String,
    /// What the fixture shows.
    pub description: String,
    /// License of the data.
    pub license: String,
    /// Where the expectations come from.
    pub source: String,
    /// The steps.
    pub cases: Vec<MigrationCase>,
}

/// One event applied to the binding a scenario has reached, and what must come of it:
/// either an effect and the binding after, or the kind of error that refuses it (the binding
/// is then unchanged).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthorityStep {
    /// What the step shows.
    pub name: String,
    /// The event.
    pub event: BindingEvent,
    /// The effect, when the event applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<Effect>,
    /// The binding after, when the event applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<NoteBinding>,
    /// The error kind (as `BindingError` serializes its `kind`), when the event is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One binding taken through a sequence of events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthorityScenario {
    /// What the scenario shows.
    pub name: String,
    /// The binding at the start.
    pub initial: NoteBinding,
    /// The events, in order.
    pub steps: Vec<AuthorityStep>,
}

/// A note-authority fixture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteAuthorityFixture {
    /// Fixture id (also the file name).
    pub id: String,
    /// What the fixture shows.
    pub description: String,
    /// License of the data.
    pub license: String,
    /// Where the expectations come from.
    pub source: String,
    /// The scenarios.
    pub scenarios: Vec<AuthorityScenario>,
}

/// The `kind` tag of a serialized error enum, as fixtures name errors.
fn error_kind<E: Serialize + std::fmt::Display>(error: &E) -> String {
    serde_json::to_value(error)
        .ok()
        .and_then(|v| v.get("kind").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| error.to_string())
}

pub(crate) fn run_workspace_migration(fixture: &WorkspaceMigrationFixture, r: &mut Recorder) {
    for case in &fixture.cases {
        let migrated = match migrate_workspace(case.input.clone(), &case.from, &case.to) {
            Ok(value) => value,
            Err(error) => {
                r.fail(
                    &format!("{}: migrates", case.name),
                    "a migrated document",
                    &error.to_string(),
                );
                continue;
            }
        };
        r.check(
            &format!("{}: migrates", case.name),
            &case.expected,
            &migrated,
        );
        let again =
            migrate_workspace(migrated.clone(), &case.to, &case.to).map_err(|e| e.to_string());
        r.check(
            &format!("{}: migrating again changes nothing", case.name),
            &Ok(case.expected.clone()),
            &again,
        );
        let backwards = match migrate_workspace(migrated.clone(), &case.to, &case.from) {
            Ok(_) => "migrated".to_owned(),
            Err(error) => error_kind(&error),
        };
        r.check(
            &format!("{}: backwards is refused", case.name),
            &"backwards".to_owned(),
            &backwards,
        );
        let before: Result<Workspace, _> = serde_json::from_value(case.input.clone());
        let after: Result<Workspace, _> = serde_json::from_value(migrated);
        let (before, after) = match (before, after) {
            (Ok(b), Ok(a)) => (b, a),
            (b, a) => {
                r.fail(
                    &format!("{}: both documents parse", case.name),
                    "two workspaces",
                    &format!("{:?} / {:?}", b.err(), a.err()),
                );
                continue;
            }
        };
        r.check(
            &format!("{}: pane ids are untouched", case.name),
            &before.pane_ids(),
            &after.pane_ids(),
        );
        r.check(
            &format!("{}: panes are untouched", case.name),
            &before.panes,
            &after.panes,
        );
        for layout in &case.layouts {
            let name = format!(
                "{}: {} at {} pt {:?}",
                case.name, layout.device_class, layout.width_pt, layout.text_size
            );
            let derived_before = derive_profile(
                &before,
                layout.device_class,
                layout.width_pt,
                layout.text_size,
            )
            .map(|l| ExpectedLayout::from(&l))
            .map_err(|e| e.to_string());
            let derived_after = derive_profile(
                &after,
                layout.device_class,
                layout.width_pt,
                layout.text_size,
            )
            .map(|l| ExpectedLayout::from(&l))
            .map_err(|e| e.to_string());
            r.check(
                &format!("{name}: the migrated document derives the expected layout"),
                &Ok(layout.expected.clone()),
                &derived_after,
            );
            r.check(
                &format!("{name}: the original derives the same layout"),
                &derived_before,
                &derived_after,
            );
        }
    }
}

pub(crate) fn run_note_authority(fixture: &NoteAuthorityFixture, r: &mut Recorder) {
    for scenario in &fixture.scenarios {
        let mut current = scenario.initial.clone();
        r.check(
            &format!("{}: the initial binding is well formed", scenario.name),
            &Ok(()),
            &current.validate().map_err(|e| e.to_string()),
        );
        for step in &scenario.steps {
            let name = format!("{}: {}", scenario.name, step.name);
            match apply(&current, &step.event) {
                Ok(transition) => {
                    r.check(
                        &format!("{name}: effect"),
                        &step.effect.clone().map(Ok::<Effect, String>),
                        &Some(Ok(transition.effect.clone())),
                    );
                    r.check(
                        &format!("{name}: binding"),
                        &step.binding.clone().map(Ok::<NoteBinding, String>),
                        &Some(Ok(transition.binding.clone())),
                    );
                    if step.error.is_some() {
                        r.fail(
                            &format!("{name}: refused"),
                            step.error.as_deref().unwrap_or(""),
                            "applied",
                        );
                    }
                    current = transition.binding;
                }
                Err(error) => {
                    let kind = error_kind(&error);
                    r.check(
                        &format!("{name}: refused"),
                        &step.error.clone(),
                        &Some(kind),
                    );
                    if step.effect.is_some() || step.binding.is_some() {
                        r.fail(&format!("{name}: effect"), "applied", &error.to_string());
                    }
                }
            }
            r.check(
                &format!("{name}: the binding stays well formed"),
                &Ok(()),
                &current.validate().map_err(|e| e.to_string()),
            );
        }
    }
}
