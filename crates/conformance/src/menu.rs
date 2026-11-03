//! The context menu fixture (B077): for every target kind (a word in a translation and in an
//! original-language edition, a selection, a note spot, selection and passage link, a tab, a
//! column, the background), with and without a selection and after the steps that change
//! what applies, the header, sections and rows [`Shell::context_menu`] returns, rows in full
//! where their arguments matter, the focus change a host runs first, and choosing a row; and
//! scenarios for the menu's new commands with their undo (specs/22-workspace-shell.md §4.2).
//! Swift, Kotlin and JavaScript hosts run it through [`crate::run_all`].

use std::collections::BTreeMap;

use openlogos_workspaces::commands::{CommandArgs, CommandContext, Invocation, Shell, command};
use openlogos_workspaces::menu::{
    ContextMenu, ContextMenuItem, ContextTarget, ContextTargetKind, MENU_ICONS,
};
use openlogos_workspaces::shell::Session;
use serde::{Deserialize, Serialize};

use crate::Recorder;
use crate::shell::{ShellScenario, ShellStep, StepExpectation, check_step, error_kind};
use crate::well::WellStep;

/// Most top-level rows a menu may have (specs/22 §4.2).
const MAX_ROWS: usize = 12;

/// The menu's header: what it is for, its title, the focus change and the dispatch context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuHeader {
    /// The target kind.
    pub target_kind: ContextTargetKind,
    /// The title, `null` for none.
    pub title: Option<String>,
    /// The focus change run before the chosen row, `null` for none.
    pub focus: Option<Invocation>,
    /// The context rows are dispatched under.
    pub context: CommandContext,
}

/// One row in short form: label, command, and only what is set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowView {
    /// What the row says.
    pub label: String,
    /// The command it dispatches.
    pub command: String,
    /// Whether it is the current choice.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub active: bool,
    /// Why it cannot run, in words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<String>,
    /// Its submenu.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub submenu: Vec<RowView>,
}

/// One section in short form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionView {
    /// Its heading.
    pub label: Option<String>,
    /// Its rows.
    pub rows: Vec<RowView>,
}

/// A row in full, without its submenu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemView {
    /// The command.
    pub command_id: String,
    /// Its arguments.
    pub args: CommandArgs,
    /// The key shown, `null` for none.
    pub key: Option<String>,
    /// The icon name.
    pub icon: String,
    /// Whether it is the current choice.
    pub active: bool,
    /// The value shown, `null` for none.
    pub value: Option<String>,
    /// Why it cannot run, `null` when it can.
    pub disabled_text: Option<String>,
}

/// Choosing a row: the menu's focus change, then the row under the menu's context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuChoice {
    /// The row's label, a submenu row as `Parent/Child`.
    pub path: String,
    /// What must hold after the row ran.
    pub expect: StepExpectation,
    /// Commands run afterwards (an undo), with their expectations.
    #[serde(default)]
    pub then: Vec<ShellStep>,
}

/// What the menu must look like; only the fields present are checked.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MenuExpectation {
    /// The reason kind when the target does not exist.
    pub error: Option<String>,
    /// The header.
    pub header: Option<MenuHeader>,
    /// Every section and row in short form.
    pub outline: Option<Vec<SectionView>>,
    /// Rows in full, by path.
    pub items: Option<BTreeMap<String, ItemView>>,
    /// The whole menu.
    pub menu: Option<ContextMenu>,
    /// Rows chosen, each from a fresh run of the case's steps.
    pub choose: Vec<MenuChoice>,
}

/// Steps from the fixture session, then the menu for a target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuCase {
    /// Unique within the fixture.
    pub name: String,
    /// Commands run first.
    #[serde(default)]
    pub steps: Vec<WellStep>,
    /// What was right-clicked.
    pub target: ContextTarget,
    /// The host's context.
    #[serde(default)]
    pub context: CommandContext,
    /// What must hold.
    pub expect: MenuExpectation,
}

/// The context menu fixture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextMenuFixture {
    /// Fixture id (also the file name).
    pub id: String,
    /// What the fixture shows.
    pub description: String,
    /// License of the data.
    pub license: String,
    /// Where the expectations come from.
    pub source: String,
    /// The session every case starts from.
    pub session: Session,
    /// Resource titles, as a host registers them.
    pub resource_titles: BTreeMap<String, String>,
    /// Resource languages, as a host registers them.
    pub resource_languages: BTreeMap<String, String>,
    /// Menu cases.
    pub cases: Vec<MenuCase>,
    /// The menu's new commands run in sequence, with undo.
    pub scenarios: Vec<ShellScenario>,
}

fn shell_of(fixture: &ContextMenuFixture, r: &mut Recorder, name: &str) -> Option<Shell> {
    match Shell::new(fixture.session.clone()) {
        Ok(mut shell) => {
            shell.set_resource_titles(fixture.resource_titles.clone());
            shell.set_resource_languages(fixture.resource_languages.clone());
            Some(shell)
        }
        Err(error) => {
            r.fail(&format!("{name} session"), "valid", &error.to_string());
            None
        }
    }
}

/// The fixture session after a case's steps; failures are recorded.
fn prepared(fixture: &ContextMenuFixture, r: &mut Recorder, case: &MenuCase) -> Option<Shell> {
    let mut shell = shell_of(fixture, r, &case.name)?;
    for (i, step) in case.steps.iter().enumerate() {
        let name = format!("{} {} {}", case.name, i + 1, step.command);
        let result = shell.apply(
            &Invocation::with(&step.command, step.args.clone()),
            &step.context,
        );
        match (&step.error, &result) {
            (Some(kind), Err(error)) => r.check(&format!("{name} error"), kind, &error_kind(error)),
            (Some(kind), Ok(_)) => r.fail(&format!("{name} error"), kind, "ok"),
            (None, Err(error)) => r.fail(&format!("{name} runs"), "ok", &error.to_string()),
            (None, Ok(_)) => {}
        }
    }
    Some(shell)
}

fn row_view(item: &ContextMenuItem) -> RowView {
    RowView {
        label: item.label.clone(),
        command: item.command_id.clone(),
        active: item.active,
        disabled: item.disabled_text.clone(),
        submenu: item.submenu.iter().map(row_view).collect(),
    }
}

/// The menu in short form.
#[must_use]
pub fn outline(menu: &ContextMenu) -> Vec<SectionView> {
    menu.sections
        .iter()
        .map(|s| SectionView {
            label: s.label.clone(),
            rows: s.items.iter().map(row_view).collect(),
        })
        .collect()
}

fn item_view(item: &ContextMenuItem) -> ItemView {
    ItemView {
        command_id: item.command_id.clone(),
        args: item.args.clone(),
        key: item.key.clone(),
        icon: item.icon.clone(),
        active: item.active,
        value: item.value.clone(),
        disabled_text: item.disabled_text.clone(),
    }
}

/// The row at `Parent/Child` or `Label`.
#[must_use]
pub fn row_at<'a>(menu: &'a ContextMenu, path: &str) -> Option<&'a ContextMenuItem> {
    let mut parts = path.splitn(2, '/');
    let top = parts.next()?;
    let row = menu
        .sections
        .iter()
        .flat_map(|s| s.items.iter())
        .find(|i| i.label == top)?;
    match parts.next() {
        Some(child) => row.submenu.iter().find(|c| c.label == child),
        None => Some(row),
    }
}

/// What every menu must satisfy whatever the case: registered commands, known icons, words
/// for reasons, at most [`MAX_ROWS`] top-level rows, no empty section or submenu.
fn check_shape(r: &mut Recorder, name: &str, menu: &ContextMenu) {
    let mut problems: Vec<String> = Vec::new();
    let rows: usize = menu.sections.iter().map(|s| s.items.len()).sum();
    if rows > MAX_ROWS {
        problems.push(format!("{rows} top-level rows"));
    }
    for section in &menu.sections {
        if section.items.is_empty() {
            problems.push(format!("empty section {:?}", section.label));
        }
        for item in &section.items {
            for row in std::iter::once(item).chain(item.submenu.iter()) {
                if command(&row.command_id).is_none() {
                    problems.push(format!("{}: unregistered {}", row.label, row.command_id));
                }
                if !MENU_ICONS.contains(&row.icon.as_str()) {
                    problems.push(format!("{}: unknown icon {}", row.label, row.icon));
                }
                if row
                    .disabled_text
                    .as_deref()
                    .is_some_and(|t| t.contains('_'))
                {
                    problems.push(format!("{}: reason is not words", row.label));
                }
            }
        }
    }
    r.check(&format!("{name} shape"), &Vec::<String>::new(), &problems);
}

fn check_case(fixture: &ContextMenuFixture, r: &mut Recorder, case: &MenuCase) {
    let name = &case.name;
    let Some(shell) = prepared(fixture, r, case) else {
        return;
    };
    let expect = &case.expect;
    let menu = match (
        shell.context_menu(&case.target, &case.context),
        &expect.error,
    ) {
        (Err(reason), Some(kind)) => {
            let actual = serde_json::to_value(&reason)
                .ok()
                .and_then(|v| v["kind"].as_str().map(str::to_owned))
                .unwrap_or_default();
            r.check(&format!("{name} error"), kind, &actual);
            return;
        }
        (Ok(_), Some(kind)) => {
            r.fail(&format!("{name} error"), kind, "a menu");
            return;
        }
        (Err(reason), None) => {
            r.fail(&format!("{name} menu"), "a menu", &reason.to_string());
            return;
        }
        (Ok(menu), None) => menu,
    };
    check_shape(r, name, &menu);
    if let Some(header) = &expect.header {
        let actual = MenuHeader {
            target_kind: menu.target_kind,
            title: menu.title.clone(),
            focus: menu.focus.clone(),
            context: menu.context,
        };
        r.check(&format!("{name} header"), header, &actual);
    }
    if let Some(sections) = &expect.outline {
        r.check(&format!("{name} outline"), sections, &outline(&menu));
    }
    if let Some(items) = &expect.items {
        for (path, expected) in items {
            match row_at(&menu, path) {
                Some(row) => r.check(&format!("{name} row {path}"), expected, &item_view(row)),
                None => r.fail(&format!("{name} row {path}"), "a row", "none"),
            }
        }
    }
    if let Some(whole) = &expect.menu {
        r.check(&format!("{name} menu"), whole, &menu);
    }
    for choice in &expect.choose {
        let label = format!("{name} choose {}", choice.path);
        let Some(row) = row_at(&menu, &choice.path) else {
            r.fail(&label, "a row", "none");
            continue;
        };
        let Some(mut shell) = prepared(fixture, r, case) else {
            continue;
        };
        if let Some(focus) = &menu.focus
            && let Err(error) = shell.apply(focus, &menu.context)
        {
            r.fail(&format!("{label} focus"), "ok", &error.to_string());
            continue;
        }
        let result = shell.apply(
            &Invocation::with(&row.command_id, row.args.clone()),
            &menu.context,
        );
        check_step(r, &label, &shell, &result, &choice.expect);
        for (i, step) in choice.then.iter().enumerate() {
            let result = shell.apply(
                &Invocation::with(&step.command, step.args.clone()),
                &step.context,
            );
            check_step(
                r,
                &format!("{label} then {} {}", i + 1, step.command),
                &shell,
                &result,
                &step.expect,
            );
        }
    }
}

pub(crate) fn run_context_menu(fixture: &ContextMenuFixture, r: &mut Recorder) {
    let mut kinds: Vec<&str> = Vec::new();
    for case in &fixture.cases {
        kinds.push(case.target.kind().name());
        check_case(fixture, r, case);
    }
    let missing: Vec<&str> = ContextTargetKind::ALL
        .iter()
        .map(|k| k.name())
        .filter(|k| !kinds.contains(k))
        .collect();
    r.check(
        "every target kind has a case",
        &Vec::<&str>::new(),
        &missing,
    );
    for scenario in &fixture.scenarios {
        let Some(mut shell) = shell_of(fixture, r, &scenario.name) else {
            continue;
        };
        for (i, step) in scenario.steps.iter().enumerate() {
            let name = format!("{} {} {}", scenario.name, i + 1, step.command);
            let result = shell.apply(
                &Invocation::with(&step.command, step.args.clone()),
                &step.context,
            );
            check_step(r, &name, &shell, &result, &step.expect);
        }
    }
}
