//! The user store fixture (B074, specs/24-persistence.md): a session worked on (a scroll
//! into the next verse, the pen, a lexicon lookup in Greek, a tab dragged, a column resized)
//! with notes, highlights and ink, exported and restored into a fresh shell: the same
//! session, stream positions, tool and annotations, and the exact document every host writes.
//! Then restores of older and damaged stores (a 2.0 store without the B074 fields, a 1.x
//! workspace inside a store, a bad session beside good notes, documents that are not stores)
//! and the placeholder rule for highlights over the placeholder reading library, so Swift,
//! Kotlin and JavaScript hosts assert through [`crate::run_all`] that their saves and
//! restores are the core's.

use std::collections::BTreeMap;

use openlogos_workspaces::commands::{CommandArgs, CommandContext, Invocation, Shell};
use openlogos_workspaces::shell::{ReadingSettings, Session, Tool};
use openlogos_workspaces::store::{Highlight, InkMark, Note, PlaceholderCheck, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Recorder;
use crate::shell::ActiveView;

/// One command of the worked session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoreStep {
    /// The command id.
    pub command: String,
    /// Its arguments.
    #[serde(default)]
    pub args: CommandArgs,
}

/// What the restored shell must show; only the fields present are checked.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionExpectation {
    /// Focus.
    pub active: Option<ActiveView>,
    /// Every workspace as `id: title`, in order.
    pub workspaces: Option<Vec<String>>,
    /// The tool.
    pub tool: Option<Tool>,
    /// Text size, line spacing and theme.
    pub reading: Option<ReadingSettings>,
    /// Scroll anchors in the active workspace as `segment@scalar`, `null` for none.
    pub anchors: Option<BTreeMap<String, Option<String>>>,
    /// Tab passages in the active workspace.
    pub references: Option<BTreeMap<String, Option<String>>>,
    /// The active workspace's column widths in thousandths, rounded.
    pub widths_permille: Option<Vec<u32>>,
    /// Lexicon languages in the active workspace, `null` for none.
    pub lexicon_languages: Option<BTreeMap<String, Option<String>>>,
}

/// The worked session and what its store must be.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoundTrip {
    /// Commands from the `shell-commands` session, with the fixture languages registered.
    pub steps: Vec<StoreStep>,
    /// Notes saved after the commands.
    pub notes: Vec<Note>,
    /// Highlights added (ids are minted).
    pub highlights: Vec<Highlight>,
    /// Ink added (ids are minted).
    pub ink: Vec<InkMark>,
    /// The ids minted for the highlights, then the ink.
    pub ids: Vec<String>,
    /// The session as restored.
    pub session: SessionExpectation,
    /// Highlight and ink ids per `resource_id chapter_key` query on the restored shell.
    pub queries: BTreeMap<String, [Vec<String>; 2]>,
    /// The document written, exactly.
    pub store: Value,
}

/// What a restore must report.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RestoreExpectation {
    /// The error kind when the document is refused whole.
    pub error: Option<String>,
    /// The version the document named.
    pub schema_version: Option<String>,
    /// Whether the session came from the store.
    pub session_restored: Option<bool>,
    /// `[workspaces, tabs, notes, highlights, ink]` after the restore.
    pub counts: Option<[u32; 5]>,
    /// Parts set aside, as `part` or `part:id`.
    pub issues: Option<Vec<String>>,
    /// Whether the next save is due (`changes` moved).
    pub save_due: Option<bool>,
    /// The session afterwards.
    pub session: Option<SessionExpectation>,
}

/// One document restored into the fixture shell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestoreCase {
    /// Unique within the fixture.
    pub name: String,
    /// The document as JSON.
    #[serde(default)]
    pub document: Option<Value>,
    /// Or the file's raw text, for documents that are not JSON.
    #[serde(default)]
    pub input: Option<String>,
    /// What must hold.
    pub expect: RestoreExpectation,
}

/// One highlight offered over the placeholder reading library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaceholderCase {
    /// Unique within the fixture.
    pub name: String,
    /// The highlight.
    pub highlight: Highlight,
    /// The error kind, `null` when it is stored.
    pub error: Option<String>,
}

/// The user store fixture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserStoreFixture {
    /// Fixture id (also the file name).
    pub id: String,
    /// What the fixture shows.
    pub description: String,
    /// License of the data.
    pub license: String,
    /// Where the expectations come from.
    pub source: String,
    /// The worked session.
    pub round_trip: RoundTrip,
    /// Restores.
    pub restores: Vec<RestoreCase>,
    /// The placeholder rule.
    pub placeholders: Vec<PlaceholderCase>,
}

/// The `shell-commands` session with the fixture editions' languages, as
/// `ShellHandle.fixture()` opens it.
#[must_use]
pub fn fixture_shell() -> Shell {
    let mut shell =
        Shell::new(crate::shell::fixture_session()).expect("the fixture session checks");
    shell.session.resource_languages = crate::well::fixture_resource_languages();
    shell
}

/// The placeholder rule over [`crate::reading::reading_library`]'s editions.
#[must_use]
pub fn placeholder_check() -> impl PlaceholderCheck {
    let library = crate::reading::reading_library();
    move |resource: &str, segment: &str| {
        library.segment_is_placeholder(resource, segment) == Some(true)
    }
}

fn kind_of(error: &StoreError) -> String {
    serde_json::to_value(error)
        .ok()
        .and_then(|v| v.get("kind").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_default()
}

fn check_session(r: &mut Recorder, name: &str, expect: &SessionExpectation, session: &Session) {
    let ws = session.active();
    if let Some(active) = &expect.active {
        let actual = ActiveView {
            workspace: ws.id.clone(),
            column: ws.active_index() as u32 + 1,
            tab: ws.active_tab().id.clone(),
        };
        r.check(&format!("{name}: active"), active, &actual);
    }
    if let Some(workspaces) = &expect.workspaces {
        let actual: Vec<String> = session
            .workspaces
            .iter()
            .map(|w| format!("{}: {}", w.id, w.title))
            .collect();
        r.check(&format!("{name}: workspaces"), workspaces, &actual);
    }
    if let Some(tool) = &expect.tool {
        r.check(&format!("{name}: tool"), tool, &session.tool);
    }
    if let Some(reading) = &expect.reading {
        r.check(&format!("{name}: reading"), reading, &session.reading);
    }
    if let Some(anchors) = &expect.anchors {
        let actual: BTreeMap<String, Option<String>> = ws
            .tabs()
            .map(|t| {
                (
                    t.id.clone(),
                    t.scroll_anchor
                        .as_ref()
                        .map(|a| format!("{}@{}", a.segment_id, a.scalar)),
                )
            })
            .collect();
        r.check(&format!("{name}: anchors"), anchors, &actual);
    }
    if let Some(references) = &expect.references {
        let actual: BTreeMap<String, Option<String>> = ws
            .tabs()
            .map(|t| (t.id.clone(), t.reference.clone()))
            .collect();
        r.check(&format!("{name}: references"), references, &actual);
    }
    if let Some(widths) = &expect.widths_permille {
        let actual: Vec<u32> = ws
            .columns
            .iter()
            .map(|c| (c.width_fraction * 1000.0).round() as u32)
            .collect();
        r.check(&format!("{name}: widths"), widths, &actual);
    }
    if let Some(languages) = &expect.lexicon_languages {
        let actual: BTreeMap<String, Option<String>> = ws
            .tabs()
            .map(|t| (t.id.clone(), t.lexicon_language.clone()))
            .collect();
        r.check(&format!("{name}: lexicon languages"), languages, &actual);
    }
}

fn run_round_trip(trip: &RoundTrip, r: &mut Recorder) {
    let context = CommandContext::default();
    let mut shell = fixture_shell();
    for (i, step) in trip.steps.iter().enumerate() {
        let invocation = Invocation::with(&step.command, step.args.clone());
        if let Err(error) = shell.apply(&invocation, &context) {
            r.fail(
                &format!("round trip: step {i} {}", step.command),
                "runs",
                &error.to_string(),
            );
            return;
        }
    }
    let check = placeholder_check();
    let mut ids = Vec::new();
    for note in &trip.notes {
        if let Err(error) = shell.put_note(note.clone()) {
            r.fail("round trip: note saved", &note.id, &error.to_string());
        }
    }
    for h in &trip.highlights {
        match shell.add_highlight(h.clone(), Some(&check)) {
            Ok(id) => ids.push(id),
            Err(error) => ids.push(kind_of(&error)),
        }
    }
    for k in &trip.ink {
        match shell.add_ink(k.clone()) {
            Ok(id) => ids.push(id),
            Err(error) => ids.push(kind_of(&error)),
        }
    }
    r.check("round trip: minted ids", &trip.ids, &ids);

    let json = shell.export_store_json();
    let mut written: Value = serde_json::from_str(&json).unwrap_or(Value::Null);
    r.check(
        "round trip: written by this core",
        &openlogos_core::api_version(),
        &written["core_api_version"].as_str().unwrap_or_default(),
    );
    // The fixture pins the form, not the API version that wrote it.
    if let (Some(doc), Some(version)) =
        (written.as_object_mut(), trip.store.get("core_api_version"))
    {
        doc.insert("core_api_version".to_owned(), version.clone());
    }
    r.check("round trip: the document written", &trip.store, &written);
    r.check(
        "round trip: exportStore equals exportStoreJson",
        &serde_json::from_str::<Value>(&json).unwrap_or(Value::Null),
        &serde_json::to_value(shell.export_store()).unwrap_or(Value::Null),
    );

    let mut restored = fixture_shell();
    match restored.import_store_json(&json) {
        Ok(report) => {
            r.check(
                "round trip: nothing set aside",
                &Vec::<String>::new(),
                &report.issues.iter().map(|i| i.detail.clone()).collect(),
            );
            r.check(
                "round trip: the session is the one saved",
                &true,
                &(restored.session == shell.session),
            );
            r.check(
                "round trip: the annotations are the ones saved",
                &true,
                &(restored.annotations == shell.annotations),
            );
            r.check(
                "round trip: saving again writes the same bytes",
                &json,
                &restored.export_store_json(),
            );
            r.check(
                "round trip: the restored shell has nothing to undo",
                &(true, true),
                &(
                    restored.undo_stack.is_empty(),
                    restored.redo_stack.is_empty(),
                ),
            );
            check_session(r, "round trip", &trip.session, &restored.session);
            for (query, expected) in &trip.queries {
                let (resource, chapter) = query.split_once(' ').unwrap_or((query, ""));
                let actual = [
                    restored
                        .highlights_in(resource, chapter)
                        .iter()
                        .map(|h| h.id.clone())
                        .collect(),
                    restored
                        .ink_in(resource, chapter)
                        .iter()
                        .map(|k| k.id.clone())
                        .collect(),
                ];
                r.check(&format!("round trip: in {query}"), expected, &actual);
            }
        }
        Err(error) => r.fail("round trip: restores", "a report", &error.to_string()),
    }
}

fn run_restore(case: &RestoreCase, r: &mut Recorder) {
    let name = &case.name;
    let input = match (&case.document, &case.input) {
        (Some(document), _) => document.to_string(),
        (None, Some(input)) => input.clone(),
        (None, None) => String::new(),
    };
    let mut shell = fixture_shell();
    let before = shell.clone();
    let changes = shell.changes;
    let result = shell.import_store_json(&input);
    let expect = &case.expect;
    match result {
        Err(error) => {
            r.check(
                &format!("{name}: error"),
                &expect.error,
                &Some(kind_of(&error)),
            );
            r.check(
                &format!("{name}: the shell is unchanged"),
                &true,
                &(shell == before),
            );
        }
        Ok(report) => {
            r.check(&format!("{name}: error"), &expect.error, &None);
            if let Some(version) = &expect.schema_version {
                r.check(&format!("{name}: version"), version, &report.schema_version);
            }
            if let Some(restored) = &expect.session_restored {
                r.check(
                    &format!("{name}: session restored"),
                    restored,
                    &report.session_restored,
                );
            }
            if let Some(counts) = &expect.counts {
                let actual = [
                    report.workspaces,
                    report.tabs,
                    report.notes,
                    report.highlights,
                    report.ink,
                ];
                r.check(&format!("{name}: counts"), counts, &actual);
            }
            if let Some(issues) = &expect.issues {
                let actual: Vec<String> = report
                    .issues
                    .iter()
                    .map(|i| {
                        let part = serde_json::to_value(i.part)
                            .ok()
                            .and_then(|v| v.as_str().map(str::to_owned))
                            .unwrap_or_default();
                        match &i.id {
                            Some(id) => format!("{part}:{id}"),
                            None => part,
                        }
                    })
                    .collect();
                r.check(&format!("{name}: issues"), issues, &actual);
            }
            if let Some(due) = &expect.save_due {
                r.check(
                    &format!("{name}: save due"),
                    due,
                    &(shell.changes != changes),
                );
            }
            if let Some(session) = &expect.session {
                check_session(r, name, session, &shell.session);
            }
        }
    }
}

/// Runs the user store fixture.
pub(crate) fn run_user_store(fixture: &UserStoreFixture, r: &mut Recorder) {
    run_round_trip(&fixture.round_trip, r);
    for case in &fixture.restores {
        run_restore(case, r);
    }
    let check = placeholder_check();
    for case in &fixture.placeholders {
        let mut shell = fixture_shell();
        let actual = shell
            .add_highlight(case.highlight.clone(), Some(&check))
            .err()
            .map(|e| kind_of(&e));
        r.check(&format!("placeholder: {}", case.name), &case.error, &actual);
    }
}

