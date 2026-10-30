//! The workspace shell fixture (B070): command sequences through the one registry with undo
//! and redo, palette queries with their ranked sections, the registry's ids and the 1.x →
//! 2.0 migration of both workspace examples, so Swift, Kotlin and JavaScript hosts assert
//! through [`crate::run_all`] that the core behaves the same everywhere
//! (specs/22-workspace-shell.md §2, §3).
//!
//! [`FixtureTextProvider`] answers the palette's Text section from the conformance fixture
//! texts (BSB and SBLGNT John 3, WLC Genesis 1) until B071's edition search exists.

use std::collections::BTreeMap;

use openlogos_workspaces::commands::{
    CommandArgs, CommandContext, CommandError, Invocation, Shell, registry,
};
use openlogos_workspaces::migrate_workspace_to_2;
use openlogos_workspaces::palette::{
    PaletteContext, PaletteSection, TextHit, TextProvider, palette_query,
};
use openlogos_workspaces::shell::{
    InkSettings, LinkMode, ReadingSettings, Session, Tool, Workspace,
};
use openlogos_workspaces::{LayoutConstants, layout_constants};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{FIXTURE_FILES, Fixture, Recorder};

/// Editions of the text fixtures the provider searches (the bidi note is not an edition).
const EDITIONS: [&str; 3] = ["bsb", "sblgnt", "wlc"];

/// Lower case without combining marks or Hebrew points and cantillation, so `loved` finds
/// `Loved` and `ברא` finds the pointed word. Greek precomposed accents are not folded; the
/// project tokenizer (B064) does that for the real search.
fn fold(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(*c as u32, 0x0300..=0x036F | 0x0591..=0x05BD | 0x05BF | 0x05C1..=0x05C2 | 0x05C4..=0x05C5 | 0x05C7 | 0x2066..=0x2069))
        .flat_map(char::to_lowercase)
        .collect()
}

/// The palette's Text section over the conformance fixture texts: every segment whose
/// folded text contains the folded query, in fixture and segment order, with the segment's
/// first 80 characters as the snippet.
pub struct FixtureTextProvider {
    segments: Vec<(String, String, String, String)>,
}

impl FixtureTextProvider {
    /// Loads the fixture texts.
    #[must_use]
    pub fn new() -> FixtureTextProvider {
        let mut segments = Vec::new();
        for (_, json) in FIXTURE_FILES {
            if let Ok(Fixture::Text(text)) = serde_json::from_str::<Fixture>(json)
                && EDITIONS.contains(&text.edition_id.as_str())
            {
                for segment in text.segments {
                    let snippet: String = segment.text.chars().take(80).collect();
                    let snippet = if segment.text.chars().count() > 80 {
                        snippet + "…"
                    } else {
                        snippet
                    };
                    segments.push((
                        text.edition_id.clone(),
                        segment.segment_id.clone(),
                        fold(&segment.text),
                        snippet,
                    ));
                }
            }
        }
        FixtureTextProvider { segments }
    }
}

impl Default for FixtureTextProvider {
    fn default() -> Self {
        FixtureTextProvider::new()
    }
}

impl TextProvider for FixtureTextProvider {
    fn search(&self, query: &str, limit: usize) -> Vec<TextHit> {
        let query = fold(query.trim());
        if query.is_empty() {
            return Vec::new();
        }
        self.segments
            .iter()
            .filter(|(_, _, folded, _)| folded.contains(&query))
            .take(limit)
            .map(|(edition, reference, _, snippet)| TextHit {
                edition_id: edition.clone(),
                reference: reference.clone(),
                snippet: snippet.clone(),
            })
            .collect()
    }
}

/// The session of the `shell-commands` fixture: what the bindings' `ShellHandle.fixture()`
/// opens, read from the copy [`FIXTURE_FILES`] already embeds.
#[must_use]
pub fn fixture_session() -> Session {
    let json = FIXTURE_FILES
        .iter()
        .find(|(id, _)| *id == "shell-commands")
        .map(|(_, json)| *json)
        .expect("the shell fixture is compiled in");
    match serde_json::from_str::<Fixture>(json) {
        Ok(Fixture::ShellCommands(fixture)) => fixture.session,
        _ => panic!("the shell fixture parses"),
    }
}

/// Where focus is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveView {
    /// The active workspace id.
    pub workspace: String,
    /// The active column's number.
    pub column: u32,
    /// The active tab id.
    pub tab: String,
}

/// What a step must leave behind; only the fields present are checked.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StepExpectation {
    /// The error kind: a [`openlogos_workspaces::commands::DisabledReason`] kind for a
    /// disabled command, else the [`CommandError`] kind.
    pub error: Option<String>,
    /// Whether the core performed the command.
    pub handled: Option<bool>,
    /// Whether it went on the undo stack.
    pub recorded: Option<bool>,
    /// Focus.
    pub active: Option<ActiveView>,
    /// The active workspace's columns as tab ids, the shown one prefixed `*`.
    pub columns: Option<Vec<Vec<String>>>,
    /// Every workspace as `id: title`, in order.
    pub workspaces: Option<Vec<String>>,
    /// Tab passages in the active workspace.
    pub references: Option<BTreeMap<String, Option<String>>>,
    /// Link modes in the active workspace: `pinned`, `follows` or `group N`.
    pub links: Option<BTreeMap<String, String>>,
    /// Tabs that followed the navigation.
    pub followed: Option<Vec<String>>,
    /// The tool.
    pub tool: Option<Tool>,
    /// The active workspace's notes dock.
    pub notes_dock_open: Option<bool>,
    /// Whether undo is possible.
    pub can_undo: Option<bool>,
    /// Whether redo is possible.
    pub can_redo: Option<bool>,
    /// Text size, line spacing and theme.
    pub reading: Option<ReadingSettings>,
    /// Brush width and swatches.
    pub ink: Option<InkSettings>,
    /// The active workspace's column widths in thousandths, rounded.
    pub widths_permille: Option<Vec<u32>>,
    /// Scroll anchors in the active workspace as `segment@scalar`, `null` for none.
    pub anchors: Option<BTreeMap<String, Option<String>>>,
    /// The shell revision.
    pub revision: Option<u64>,
    /// Tab labels in the active workspace from `Shell::view`: `[reference_label,
    /// chapter_label]`.
    pub labels: Option<BTreeMap<String, [Option<String>; 2]>>,
}

/// One command of a scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShellStep {
    /// The command id (`edit.undo` and `edit.redo` included).
    pub command: String,
    /// Its arguments.
    #[serde(default)]
    pub args: CommandArgs,
    /// The host's context.
    #[serde(default)]
    pub context: CommandContext,
    /// What must hold afterwards.
    pub expect: StepExpectation,
}

/// Commands run in order from the fixture session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShellScenario {
    /// Unique within the fixture.
    pub name: String,
    /// The steps.
    pub steps: Vec<ShellStep>,
}

/// One expected palette row; `args` and `disabled` are checked when present.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpectedItem {
    /// Its section.
    pub section: PaletteSection,
    /// Its title.
    pub title: String,
    /// The command Enter dispatches.
    pub command: String,
    /// Its arguments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<CommandArgs>,
    /// The disabled reason kind, or `null` for enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<Option<String>>,
    /// The disabled reason in words, or `null` for enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_text: Option<Option<String>>,
    /// The second line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<Option<String>>,
    /// The shortcut shown, `{n}` filled; `null` for none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<Option<String>>,
}

/// One palette query against the fixture session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaletteCase {
    /// Unique within the fixture.
    pub name: String,
    /// The field's text.
    pub query: String,
    /// The host's context.
    #[serde(default)]
    pub context: PaletteContext,
    /// The leading rows, in display order.
    pub expect: Vec<ExpectedItem>,
    /// Exactly this many rows in all, when present.
    #[serde(default)]
    pub count: Option<usize>,
    /// Per-section totals before capping, when present.
    #[serde(default)]
    pub totals: Option<BTreeMap<String, u32>>,
    /// The notice, when present.
    #[serde(default)]
    pub notice: Option<Option<String>>,
}

/// A 1.x workspace and the 2.0 document it migrates to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShellMigrationCase {
    /// Unique within the fixture.
    pub name: String,
    /// The 1.x document.
    pub input: Value,
    /// The 2.0 document.
    pub expected: Value,
}

/// The shell fixture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShellFixture {
    /// Fixture id (also the file name).
    pub id: String,
    /// What the fixture shows.
    pub description: String,
    /// License of the data.
    pub license: String,
    /// Where the expectations come from.
    pub source: String,
    /// The session every scenario and query starts from.
    pub session: Session,
    /// Every registered command id, in registry order.
    pub registry_ids: Vec<String>,
    /// Command sequences.
    pub scenarios: Vec<ShellScenario>,
    /// Palette queries.
    pub palette: Vec<PaletteCase>,
    /// Workspace migrations to 2.0.
    pub migrations: Vec<ShellMigrationCase>,
    /// The pane minimums and measure caps every host reads from the core.
    pub layout_constants: LayoutConstants,
    /// Resource names the palette queries run with, as a host registers them.
    #[serde(default)]
    pub resource_titles: BTreeMap<String, String>,
    /// Citations and their short form (`formatShort`).
    #[serde(default)]
    pub short_references: Vec<ShortReferenceCase>,
    /// What a menu shows for a disabled command on the fixture session, by command id.
    #[serde(default)]
    pub reason_texts: BTreeMap<String, String>,
}

/// One citation and its short form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShortReferenceCase {
    /// The citation as typed.
    pub input: String,
    /// `ReferenceSet::to_short` of its parse.
    pub expected: String,
}

/// A reason text a reader would see that names a field instead of saying something.
fn not_words(text: &str) -> bool {
    text.contains('_')
}

pub(crate) fn error_kind(error: &CommandError) -> String {
    match error {
        CommandError::Disabled { reason, .. } => kind_of(reason),
        other => kind_of(other),
    }
}

fn kind_of<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.get("kind").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_default()
}

fn columns_view(ws: &Workspace) -> Vec<Vec<String>> {
    ws.columns
        .iter()
        .map(|c| {
            c.tabs
                .iter()
                .map(|t| {
                    if t.id == c.shown_tab_id {
                        format!("*{}", t.id)
                    } else {
                        t.id.clone()
                    }
                })
                .collect()
        })
        .collect()
}

fn link_name(mode: LinkMode) -> String {
    match mode {
        LinkMode::Pinned => "pinned".to_owned(),
        LinkMode::Follows => "follows".to_owned(),
        LinkMode::Group { group } => format!("group {group}"),
    }
}

fn check_step(
    r: &mut Recorder,
    name: &str,
    shell: &Shell,
    result: &Result<openlogos_workspaces::commands::Outcome, CommandError>,
    expect: &StepExpectation,
) {
    let session = &shell.session;
    let ws = session.active();
    match (&expect.error, result) {
        (Some(kind), Err(error)) => r.check(&format!("{name} error"), kind, &error_kind(error)),
        (Some(kind), Ok(_)) => r.fail(&format!("{name} error"), kind, "ok"),
        (None, Err(error)) => r.fail(&format!("{name} runs"), "ok", &error.to_string()),
        (None, Ok(_)) => {}
    }
    if let Ok(outcome) = result {
        if let Some(handled) = expect.handled {
            r.check(&format!("{name} handled"), &handled, &outcome.handled);
        }
        if let Some(recorded) = expect.recorded {
            r.check(&format!("{name} recorded"), &recorded, &outcome.recorded);
        }
        if let Some(followed) = &expect.followed {
            r.check(
                &format!("{name} followed"),
                followed,
                &outcome.effects.followed_tab_ids,
            );
        }
    }
    if let Some(active) = &expect.active {
        let actual = ActiveView {
            workspace: ws.id.clone(),
            column: ws.active_index() as u32 + 1,
            tab: ws.active_tab().id.clone(),
        };
        r.check(&format!("{name} active"), active, &actual);
    }
    if let Some(columns) = &expect.columns {
        r.check(&format!("{name} columns"), columns, &columns_view(ws));
    }
    if let Some(workspaces) = &expect.workspaces {
        let actual: Vec<String> = session
            .workspaces
            .iter()
            .map(|w| format!("{}: {}", w.id, w.title))
            .collect();
        r.check(&format!("{name} workspaces"), workspaces, &actual);
    }
    if let Some(references) = &expect.references {
        let actual: BTreeMap<String, Option<String>> = references
            .keys()
            .map(|id| {
                let reference = ws.find_tab(id).and_then(|(_, t)| t.reference.clone());
                (id.clone(), reference)
            })
            .collect();
        r.check(&format!("{name} references"), references, &actual);
    }
    if let Some(links) = &expect.links {
        let actual: BTreeMap<String, String> = links
            .keys()
            .map(|id| {
                let mode = ws
                    .find_tab(id)
                    .map_or_else(|| "missing".to_owned(), |(_, t)| link_name(t.link_mode()));
                (id.clone(), mode)
            })
            .collect();
        r.check(&format!("{name} links"), links, &actual);
    }
    if let Some(tool) = &expect.tool {
        r.check(&format!("{name} tool"), tool, &session.tool);
    }
    if let Some(open) = &expect.notes_dock_open {
        r.check(&format!("{name} notes dock"), open, &ws.notes_dock.open);
    }
    if let Some(can_undo) = &expect.can_undo {
        r.check(
            &format!("{name} can undo"),
            can_undo,
            &!shell.undo_stack.is_empty(),
        );
    }
    if let Some(can_redo) = &expect.can_redo {
        r.check(
            &format!("{name} can redo"),
            can_redo,
            &!shell.redo_stack.is_empty(),
        );
    }
    if let Some(reading) = &expect.reading {
        r.check(&format!("{name} reading"), reading, &session.reading);
    }
    if let Some(ink) = &expect.ink {
        r.check(&format!("{name} ink"), ink, &session.ink);
    }
    if let Some(widths) = &expect.widths_permille {
        let actual: Vec<u32> = ws
            .columns
            .iter()
            .map(|c| (c.width_fraction * 1000.0).round() as u32)
            .collect();
        r.check(&format!("{name} widths"), widths, &actual);
    }
    if let Some(anchors) = &expect.anchors {
        let actual: BTreeMap<String, Option<String>> = anchors
            .keys()
            .map(|id| {
                let anchor = ws.find_tab(id).and_then(|(_, t)| {
                    t.scroll_anchor
                        .as_ref()
                        .map(|a| format!("{}@{}", a.segment_id, a.scalar))
                });
                (id.clone(), anchor)
            })
            .collect();
        r.check(&format!("{name} anchors"), anchors, &actual);
    }
    if let Some(labels) = &expect.labels {
        let view = shell.view();
        let active = view
            .workspaces
            .iter()
            .find(|w| w.id == view.active_workspace_id);
        let actual: BTreeMap<String, [Option<String>; 2]> = labels
            .keys()
            .map(|id| {
                let tab = active
                    .into_iter()
                    .flat_map(|w| w.columns.iter().flat_map(|c| c.tabs.iter()))
                    .find(|t| &t.id == id);
                let pair = tab.map_or([None, None], |t| {
                    [t.reference_label.clone(), t.chapter_label.clone()]
                });
                (id.clone(), pair)
            })
            .collect();
        r.check(&format!("{name} labels"), labels, &actual);
    }
    if let Some(revision) = &expect.revision {
        r.check(&format!("{name} revision"), revision, &shell.revision);
    }
    match session.check() {
        Ok(()) => {}
        Err(error) => r.fail(
            &format!("{name} session valid"),
            "valid",
            &error.to_string(),
        ),
    }
}

pub(crate) fn actual_item(
    item: &openlogos_workspaces::palette::PaletteItem,
    expected: &ExpectedItem,
) -> ExpectedItem {
    ExpectedItem {
        section: item.section,
        title: item.title.clone(),
        command: item.invocation.command_id.clone(),
        args: expected.args.as_ref().map(|_| item.invocation.args.clone()),
        disabled: expected
            .disabled
            .as_ref()
            .map(|_| item.disabled.as_ref().map(kind_of)),
        disabled_text: expected
            .disabled_text
            .as_ref()
            .map(|_| item.disabled_text.clone()),
        subtitle: expected.subtitle.as_ref().map(|_| item.subtitle.clone()),
        shortcut: expected.shortcut.as_ref().map(|_| item.shortcut.clone()),
    }
}

pub(crate) fn run_shell(fixture: &ShellFixture, r: &mut Recorder) {
    let ids: Vec<String> = registry().iter().map(|c| c.id.clone()).collect();
    r.check("registry ids", &fixture.registry_ids, &ids);
    r.check(
        "layout constants",
        &fixture.layout_constants,
        &layout_constants(),
    );

    for case in &fixture.migrations {
        match migrate_workspace_to_2(case.input.clone()) {
            Ok(migrated) => {
                r.check(
                    &format!("{} migrates", case.name),
                    &case.expected,
                    &migrated,
                );
                let parsed: Result<Workspace, _> = serde_json::from_value(migrated);
                match parsed.map_err(|e| e.to_string()).and_then(|w| {
                    w.to_v1().map_err(|e| e.to_string())?;
                    w.check().map_err(|e| e.to_string())
                }) {
                    Ok(()) => r.check(&format!("{} exports to 1.1", case.name), &true, &true),
                    Err(error) => r.fail(&format!("{} exports to 1.1", case.name), "ok", &error),
                }
            }
            Err(error) => r.fail(
                &format!("{} migrates", case.name),
                "2.0",
                &error.to_string(),
            ),
        }
    }

    // Every reason a reader could see: the registry on the fixture session with and
    // without a selection, every disabled step and every disabled palette row.
    let mut shown: Vec<String> = Vec::new();
    for scenario in &fixture.scenarios {
        let mut shell = match Shell::new(fixture.session.clone()) {
            Ok(shell) => shell,
            Err(error) => {
                r.fail(
                    &format!("{} session", scenario.name),
                    "valid",
                    &error.to_string(),
                );
                continue;
            }
        };
        for (i, step) in scenario.steps.iter().enumerate() {
            let name = format!("{} {} {}", scenario.name, i + 1, step.command);
            let result = shell.apply(
                &Invocation::with(&step.command, step.args.clone()),
                &step.context,
            );
            if let Err(CommandError::Disabled { reason, .. }) = &result {
                shown.push(reason.to_string());
            }
            check_step(r, &name, &shell, &result, &step.expect);
        }
    }

    for case in &fixture.short_references {
        let actual = openlogos_core::reference::parse(
            &case.input,
            &openlogos_core::reference::AliasTable::english(),
        )
        .map_or_else(|e| e.to_string(), |set| set.to_short());
        r.check(&format!("short {}", case.input), &case.expected, &actual);
    }

    let provider = FixtureTextProvider::new();
    let Ok(mut shell) = Shell::new(fixture.session.clone()) else {
        r.fail("palette session", "valid", "invalid");
        return;
    };
    let mut listed: Vec<String> = Vec::new();
    for has_selection in [false, true] {
        for state in shell.commands(&CommandContext {
            has_selection,
            ..CommandContext::default()
        }) {
            if let Some(reason) = state.disabled {
                let text = reason.to_string();
                if !has_selection && let Some(expected) = fixture.reason_texts.get(&state.spec.id) {
                    r.check(&format!("reason {}", state.spec.id), expected, &text);
                }
                if not_words(&text) {
                    listed.push(format!("{}: {text}", state.spec.id));
                }
            }
        }
    }
    r.check("registry reasons are words", &Vec::<String>::new(), &listed);
    shell.set_resource_titles(fixture.resource_titles.clone());
    for case in &fixture.palette {
        let result = palette_query(&shell, &case.query, &case.context, Some(&provider));
        let items: Vec<_> = result.items().collect();
        shown.extend(
            items
                .iter()
                .filter_map(|i| i.disabled.as_ref().map(ToString::to_string)),
        );
        let actual: Vec<ExpectedItem> = items
            .iter()
            .zip(&case.expect)
            .map(|(item, expected)| actual_item(item, expected))
            .collect();
        r.check(&format!("palette {}", case.name), &case.expect, &actual);
        if let Some(count) = case.count {
            r.check(
                &format!("palette {} count", case.name),
                &count,
                &items.len(),
            );
        }
        if let Some(totals) = &case.totals {
            let actual: BTreeMap<String, u32> = result
                .groups
                .iter()
                .map(|g| (kind_of_section(g.section), g.total))
                .collect();
            r.check(&format!("palette {} totals", case.name), totals, &actual);
        }
        if let Some(notice) = &case.notice {
            r.check(
                &format!("palette {} notice", case.name),
                notice,
                &result.notice,
            );
        }
    }
    let offenders: Vec<&String> = shown.iter().filter(|t| not_words(t)).collect();
    r.check(
        "step and palette reasons are words",
        &Vec::<&String>::new(),
        &offenders,
    );
}

fn kind_of_section(section: PaletteSection) -> String {
    serde_json::to_value(section)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixture_provider_finds_english_and_unpointed_hebrew() {
        let provider = FixtureTextProvider::new();
        let hits = provider.search("LOVED", 5);
        assert_eq!(hits.len(), 1);
        assert_eq!(
            (hits[0].edition_id.as_str(), hits[0].reference.as_str()),
            ("bsb", "John.3.16")
        );
        let hebrew = provider.search("ברא", 5);
        assert!(
            hebrew
                .iter()
                .any(|h| h.edition_id == "wlc" && h.reference == "Gen.1.1")
        );
        assert!(provider.search("   ", 5).is_empty());
        assert!(provider.search("the", 1).len() == 1);
    }
}
