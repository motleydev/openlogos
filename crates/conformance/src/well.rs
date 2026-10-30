//! The tool well fixture (B076): for every tool context, with and without a selection, the
//! groups and items [`Shell::tool_well`] returns, the effective tool falling back and
//! coming back as focus moves, the plain reasons a tool or note action outside its context
//! is disabled with, the new commands' host hand-off, and the palette rows that depend on
//! focus (specs/22-workspace-shell.md §4.1). Swift, Kotlin and JavaScript hosts run it
//! through [`crate::run_all`].

use std::collections::BTreeMap;

use openlogos_workspaces::commands::{
    CommandArgs, CommandContext, CommandError, Invocation, Shell,
};
use openlogos_workspaces::palette::palette_query;
use openlogos_workspaces::shell::{Session, Tool};
use openlogos_workspaces::well::{ToolContext, ToolWell, ToolWellGroupKind};
use serde::{Deserialize, Serialize};

use crate::Recorder;
use crate::shell::{PaletteCase, actual_item, error_kind};

/// One command run before a case's well is read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WellStep {
    /// The command id.
    pub command: String,
    /// Its arguments.
    #[serde(default)]
    pub args: CommandArgs,
    /// The host's context.
    #[serde(default)]
    pub context: CommandContext,
    /// The error kind, when the command must fail.
    #[serde(default)]
    pub error: Option<String>,
    /// Whether the core performed it, when checked.
    #[serde(default)]
    pub handled: Option<bool>,
}

/// One group as `kind` and its command ids, the active tool prefixed `*`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupView {
    /// `tools`, `ink` or `selection`.
    pub kind: ToolWellGroupKind,
    /// Command ids in order.
    pub items: Vec<String>,
}

/// What the well must look like; only the fields present are checked.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WellExpectation {
    /// The context.
    pub context: Option<ToolContext>,
    /// The stored tool.
    pub tool: Option<Tool>,
    /// The tool in effect.
    pub effective_tool: Option<Tool>,
    /// `Shell::view`'s effective tool (always for the active tab).
    pub view_effective_tool: Option<Tool>,
    /// The groups in short form.
    pub groups: Option<Vec<GroupView>>,
    /// The whole well, labels, keys, icons and values included.
    pub well: Option<ToolWell>,
    /// Disabled reasons in words under the case's context, `null` for enabled, by command.
    pub reasons: Option<BTreeMap<String, Option<String>>>,
}

/// Steps from the fixture session, then the well under a context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WellCase {
    /// Unique within the fixture.
    pub name: String,
    /// Commands run first.
    #[serde(default)]
    pub steps: Vec<WellStep>,
    /// The context the well is read under.
    #[serde(default)]
    pub context: CommandContext,
    /// What must hold.
    pub expect: WellExpectation,
}

/// The tool well fixture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolWellFixture {
    /// Fixture id (also the file name).
    pub id: String,
    /// What the fixture shows.
    pub description: String,
    /// License of the data.
    pub license: String,
    /// Where the expectations come from.
    pub source: String,
    /// The session every case starts from: one column per context.
    pub session: Session,
    /// Resource languages, as a host registers them.
    pub resource_languages: BTreeMap<String, String>,
    /// Well cases.
    pub cases: Vec<WellCase>,
    /// Palette queries whose rows depend on focus.
    pub palette: Vec<PaletteCase>,
}

fn groups_view(well: &ToolWell) -> Vec<GroupView> {
    well.groups
        .iter()
        .map(|g| GroupView {
            kind: g.kind,
            items: g
                .items
                .iter()
                .map(|i| {
                    if i.active {
                        format!("*{}", i.command_id)
                    } else {
                        i.command_id.clone()
                    }
                })
                .collect(),
        })
        .collect()
}

fn shell_of(fixture: &ToolWellFixture, r: &mut Recorder, name: &str) -> Option<Shell> {
    match Shell::new(fixture.session.clone()) {
        Ok(mut shell) => {
            shell.set_resource_languages(fixture.resource_languages.clone());
            Some(shell)
        }
        Err(error) => {
            r.fail(&format!("{name} session"), "valid", &error.to_string());
            None
        }
    }
}

fn check_case(r: &mut Recorder, case: &WellCase, shell: &Shell) {
    let name = &case.name;
    let well = shell.tool_well(&case.context);
    let expect = &case.expect;
    if let Some(context) = &expect.context {
        r.check(&format!("{name} context"), context, &well.context);
    }
    if let Some(tool) = &expect.tool {
        r.check(&format!("{name} tool"), tool, &well.tool);
    }
    if let Some(effective) = &expect.effective_tool {
        r.check(
            &format!("{name} effective tool"),
            effective,
            &well.effective_tool,
        );
    }
    if let Some(effective) = &expect.view_effective_tool {
        r.check(
            &format!("{name} view effective tool"),
            &effective.name().to_owned(),
            &shell.view().effective_tool,
        );
    }
    if let Some(groups) = &expect.groups {
        r.check(&format!("{name} groups"), groups, &groups_view(&well));
    }
    if let Some(expected) = &expect.well {
        r.check(&format!("{name} well"), expected, &well);
    }
    if let Some(reasons) = &expect.reasons {
        let actual: BTreeMap<String, Option<String>> = reasons
            .keys()
            .map(|id| {
                let reason = shell
                    .enabled(&Invocation::new(id), &case.context)
                    .err()
                    .map(|r| r.to_string());
                (id.clone(), reason)
            })
            .collect();
        r.check(&format!("{name} reasons"), reasons, &actual);
    }
}

pub(crate) fn run_tool_well(fixture: &ToolWellFixture, r: &mut Recorder) {
    let mut shown: Vec<String> = Vec::new();
    for case in &fixture.cases {
        let Some(mut shell) = shell_of(fixture, r, &case.name) else {
            continue;
        };
        for (i, step) in case.steps.iter().enumerate() {
            let name = format!("{} {} {}", case.name, i + 1, step.command);
            let result: Result<_, CommandError> = shell.apply(
                &Invocation::with(&step.command, step.args.clone()),
                &step.context,
            );
            if let Err(CommandError::Disabled { reason, .. }) = &result {
                shown.push(reason.to_string());
            }
            match (&step.error, &result) {
                (Some(kind), Err(error)) => {
                    r.check(&format!("{name} error"), kind, &error_kind(error));
                }
                (Some(kind), Ok(_)) => r.fail(&format!("{name} error"), kind, "ok"),
                (None, Err(error)) => r.fail(&format!("{name} runs"), "ok", &error.to_string()),
                (None, Ok(outcome)) => {
                    if let Some(handled) = step.handled {
                        r.check(&format!("{name} handled"), &handled, &outcome.handled);
                    }
                }
            }
        }
        check_case(r, case, &shell);
        if let Some(reasons) = &case.expect.reasons {
            shown.extend(reasons.values().flatten().cloned());
        }
    }

    let Some(shell) = shell_of(fixture, r, "palette") else {
        return;
    };
    for case in &fixture.palette {
        let result = palette_query(&shell, &case.query, &case.context, None);
        let items: Vec<_> = result.items().collect();
        shown.extend(
            items
                .iter()
                .filter_map(|i| i.disabled.as_ref().map(ToString::to_string)),
        );
        let actual: Vec<_> = items
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
    }
    let offenders: Vec<&String> = shown.iter().filter(|t| t.contains('_')).collect();
    r.check("well reasons are words", &Vec::<&String>::new(), &offenders);
}
