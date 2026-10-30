//! The contextual tool well of specs/22 §4.1 (B076): what the floating tool palette holds
//! for what has focus, decided here so the web shell and the Mac app show the same well.
//!
//! **Context.** [`tool_context`] reads the focused column's shown tab, or the host's
//! [`Focus`] when the notes dock or an empty column has it: a Reading tab is `original` when
//! its resource's registered language ([`Shell::set_resource_languages`]) is Greek, Hebrew
//! or Aramaic ([`crate::shell::is_original_language`]) and `translation` otherwise; a Notes
//! tab or the focused dock is `notes`; Lexicon and Research tabs are `reference`.
//!
//! **Effective tool.** The stored [`Session::tool`] is never lost by moving focus. When the
//! context does not offer it, the well uses the context's default ([`ToolContext::default_tool`]:
//! Select, or Type in notes) and marks that active; a context that offers the stored tool
//! uses it again. Choosing a tool stores it. A tool or note action outside its contexts is
//! disabled with [`DisabledReason::WrongContext`] (`Pen works on Bible text`), so the palette
//! still lists it with the reason.
//!
//! **Groups**, in order: `tools` always; `ink` (current and alternate swatch, brush width)
//! for `translation` and `original`; `selection` when text is selected and the context is
//! not `none`. Items carry the command to dispatch, the label, the platform-neutral key
//! (the web default; every well command binds the same key on web and Mac), a neutral icon
//! name each host maps to its own artwork ([`ICONS`]) and whether it is the active tool.
//! A command that only lacks its argument (Link passage, the swatch, the brush width) is
//! not disabled in the well: the host asks for the value.
//!
//! [`Shell::tool_well`] runs on every focus change and render; `just bench-shell` times it.

use serde::{Deserialize, Serialize};

use crate::PaneKind;
use crate::commands::{
    CommandContext, DisabledReason, Focus, Invocation, Shell, command, enabled, tool_of,
};
use crate::shell::{Session, Tool, is_original_language};

/// What the tool well is fitted to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolContext {
    /// A Reading tab of a translation.
    Translation,
    /// A Reading tab in Greek, Hebrew or Aramaic.
    Original,
    /// A Notes tab, or the notes dock with focus.
    Notes,
    /// A Lexicon or Research tab.
    Reference,
    /// Nothing to act on.
    None,
}

/// The tools group per context, in well order: mode tools and note actions.
const TRANSLATION_TOOLS: &[&str] = &[
    "tool.select",
    "tool.highlight",
    "tool.pen",
    "tool.highlighter_pen",
    "tool.eraser",
    "tool.collect",
];
const ORIGINAL_TOOLS: &[&str] = &[
    "tool.select",
    "tool.word_study",
    "tool.highlight",
    "tool.pen",
    "tool.highlighter_pen",
    "tool.eraser",
    "tool.collect",
];
const NOTES_TOOLS: &[&str] = &[
    "tool.type",
    "note.bold",
    "note.italic",
    "note.heading",
    "note.list",
    "note.link_passage",
    "tool.collect",
];
const REFERENCE_TOOLS: &[&str] = &["tool.select", "tool.highlight", "tool.collect"];
const NONE_TOOLS: &[&str] = &["tool.select"];

/// The ink group: current swatch, alternate swatch (`X` swaps), brush width.
const INK: &[&str] = &["tool.swatch", "tool.swap_swatches", "tool.width"];

/// The selection group.
const SELECTION: &[&str] = &[
    "selection.highlight",
    "selection.collect",
    "selection.note",
    "selection.search",
    "selection.copy_reference",
    "selection.study",
];

/// Every command the well shows, with its label and neutral icon name. The web shell maps
/// the icon names to inline SVG, the Mac app to SF Symbols.
pub const ICONS: &[(&str, &str, &str)] = &[
    ("tool.select", "Select", "select"),
    ("tool.word_study", "Word study", "word-study"),
    ("tool.highlight", "Highlight", "highlight"),
    ("tool.pen", "Pen", "pen"),
    ("tool.highlighter_pen", "Highlighter pen", "highlighter-pen"),
    ("tool.eraser", "Eraser", "eraser"),
    ("tool.collect", "Collect", "collect"),
    ("tool.type", "Type", "type"),
    ("note.bold", "Bold", "bold"),
    ("note.italic", "Italic", "italic"),
    ("note.heading", "Heading", "heading"),
    ("note.list", "List", "list"),
    ("note.link_passage", "Link passage", "link-passage"),
    ("tool.swatch", "Swatch", "swatch"),
    ("tool.swap_swatches", "Alternate swatch", "swatch-alternate"),
    ("tool.width", "Brush width", "brush-width"),
    ("selection.highlight", "Highlight now", "highlight-now"),
    ("selection.collect", "Collect", "collect"),
    ("selection.note", "Note on it", "note"),
    ("selection.search", "Search this", "search"),
    (
        "selection.copy_reference",
        "Copy reference",
        "copy-reference",
    ),
    ("selection.study", "Study", "study"),
];

fn label_and_icon(command_id: &str) -> (&'static str, &'static str) {
    ICONS
        .iter()
        .find(|(id, _, _)| *id == command_id)
        .map_or(("", ""), |(_, label, icon)| (*label, *icon))
}

impl ToolContext {
    /// Every context, in specs/22 §4.1 order.
    pub const ALL: [ToolContext; 5] = [
        ToolContext::Translation,
        ToolContext::Original,
        ToolContext::Notes,
        ToolContext::Reference,
        ToolContext::None,
    ];

    /// The context's name as serialized.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            ToolContext::Translation => "translation",
            ToolContext::Original => "original",
            ToolContext::Notes => "notes",
            ToolContext::Reference => "reference",
            ToolContext::None => "none",
        }
    }

    /// The tools group's commands, in well order.
    #[must_use]
    pub fn commands(self) -> &'static [&'static str] {
        match self {
            ToolContext::Translation => TRANSLATION_TOOLS,
            ToolContext::Original => ORIGINAL_TOOLS,
            ToolContext::Notes => NOTES_TOOLS,
            ToolContext::Reference => REFERENCE_TOOLS,
            ToolContext::None => NONE_TOOLS,
        }
    }

    /// Whether the context offers a tool.
    #[must_use]
    pub fn offers(self, tool: Tool) -> bool {
        self.commands().iter().any(|id| tool_of(id) == Some(tool))
    }

    /// The tool the well uses when the stored one does not fit: Type in notes, else Select.
    #[must_use]
    pub fn default_tool(self) -> Tool {
        match self {
            ToolContext::Notes => Tool::Type,
            _ => Tool::Select,
        }
    }

    /// Whether the well shows the ink group.
    #[must_use]
    pub fn has_ink(self) -> bool {
        matches!(self, ToolContext::Translation | ToolContext::Original)
    }
}

/// The context of what has focus: the host's [`Focus`], else the active tab's kind and its
/// resource's language.
#[must_use]
pub fn tool_context(session: &Session, context: &CommandContext) -> ToolContext {
    match context.focus {
        Focus::NotesDock => ToolContext::Notes,
        Focus::Empty => ToolContext::None,
        Focus::Tab => {
            let tab = session.active().active_tab();
            match tab.kind {
                PaneKind::Notes => ToolContext::Notes,
                PaneKind::Lexicon | PaneKind::Research => ToolContext::Reference,
                PaneKind::Reading => {
                    if session
                        .resource_language(&tab.content_id)
                        .is_some_and(is_original_language)
                    {
                        ToolContext::Original
                    } else {
                        ToolContext::Translation
                    }
                }
            }
        }
    }
}

/// The tool in effect: the stored tool when the context offers it, else its default.
#[must_use]
pub fn effective_tool(session: &Session, context: &CommandContext) -> Tool {
    let well = tool_context(session, context);
    if well.offers(session.tool) {
        session.tool
    } else {
        well.default_tool()
    }
}

/// Where a context-bound command works, in the words of [`DisabledReason::WrongContext`].
fn place(command_id: &str, context: ToolContext) -> &'static str {
    match command_id {
        "tool.select" => "outside notes",
        "tool.word_study" => "on Greek and Hebrew text",
        "tool.pen" | "tool.highlighter_pen" | "tool.eraser" => "on Bible text",
        "tool.type" => "in notes",
        _ if command_id.starts_with("note.") => "in notes",
        _ if context == ToolContext::None => "with a tab open",
        _ => "outside notes",
    }
}

/// Why a mode tool or note action cannot run in the current context; `None` for every
/// other command and for one that fits.
pub(crate) fn context_reason(
    session: &Session,
    command_id: &str,
    context: &CommandContext,
) -> Option<DisabledReason> {
    if tool_of(command_id).is_none() && !command_id.starts_with("note.") {
        return None;
    }
    let well = tool_context(session, context);
    if well.commands().contains(&command_id) {
        return None;
    }
    Some(DisabledReason::WrongContext {
        what: label_and_icon(command_id).0.to_owned(),
        place: place(command_id, well).to_owned(),
    })
}

/// Which group of the well.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolWellGroupKind {
    /// Mode tools and note actions.
    Tools,
    /// Swatches and brush width.
    Ink,
    /// Actions on the selection.
    Selection,
}

/// One button of the well.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolWellItem {
    /// The command a click dispatches.
    pub command_id: String,
    /// The tool it chooses, for a mode tool.
    pub tool: Option<Tool>,
    /// What the button's tooltip and accessibility label say.
    pub label: String,
    /// The default key (`V`, `Cmd+B`), the same on web and Mac; `None` for none.
    pub key: Option<String>,
    /// A neutral icon name ([`ICONS`]).
    pub icon: String,
    /// Whether it is the tool in effect.
    pub active: bool,
    /// The value an ink item shows: the swatch as `#RRGGBB`, the brush width.
    pub value: Option<String>,
    /// Why it cannot run now, in words; `None` when it can (or only lacks its argument).
    pub disabled_text: Option<String>,
}

/// One group of the well.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolWellGroup {
    /// Which group.
    pub kind: ToolWellGroupKind,
    /// Its buttons, in order.
    pub items: Vec<ToolWellItem>,
}

/// The well for what has focus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolWell {
    /// What it is fitted to.
    pub context: ToolContext,
    /// The stored tool.
    pub tool: Tool,
    /// The tool in effect ([`effective_tool`]).
    pub effective_tool: Tool,
    /// `tools`, then `ink` and `selection` where they apply.
    pub groups: Vec<ToolWellGroup>,
}

/// The well for `session` with what the host knows.
#[must_use]
pub fn tool_well(session: &Session, context: &CommandContext) -> ToolWell {
    let well = tool_context(session, context);
    let effective = effective_tool(session, context);
    let item = |command_id: &str| {
        let (label, icon) = label_and_icon(command_id);
        let tool = tool_of(command_id);
        let disabled = enabled(session, false, false, &Invocation::new(command_id), context)
            .err()
            .filter(|r| !matches!(r, DisabledReason::NeedsArgument { .. }));
        let value = match command_id {
            "tool.swatch" => Some(session.ink.swatch.clone()),
            "tool.swap_swatches" => Some(session.ink.alternate_swatch.clone()),
            "tool.width" => Some(session.ink.width.to_string()),
            _ => None,
        };
        ToolWellItem {
            command_id: command_id.to_owned(),
            tool,
            label: label.to_owned(),
            key: command(command_id).and_then(|c| c.shortcuts.web.first().cloned()),
            icon: icon.to_owned(),
            active: tool == Some(effective),
            value,
            disabled_text: disabled.as_ref().map(ToString::to_string),
        }
    };
    let group = |kind, ids: &[&str]| ToolWellGroup {
        kind,
        items: ids.iter().map(|id| item(id)).collect(),
    };
    let mut groups = vec![group(ToolWellGroupKind::Tools, well.commands())];
    if well.has_ink() {
        groups.push(group(ToolWellGroupKind::Ink, INK));
    }
    if context.has_selection && well != ToolContext::None {
        groups.push(group(ToolWellGroupKind::Selection, SELECTION));
    }
    ToolWell {
        context: well,
        tool: session.tool,
        effective_tool: effective,
        groups,
    }
}

impl Shell {
    /// The tool well for what has focus (specs/22 §4.1).
    #[must_use]
    pub fn tool_well(&self, context: &CommandContext) -> ToolWell {
        tool_well(&self.session, context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::tests::fixture_session;
    use crate::commands::{Platform, registry};

    fn ids(well: &ToolWell, kind: ToolWellGroupKind) -> Vec<String> {
        well.groups
            .iter()
            .find(|g| g.kind == kind)
            .map(|g| g.items.iter().map(|i| i.command_id.clone()).collect())
            .unwrap_or_default()
    }

    #[test]
    fn every_well_command_is_registered_labelled_and_keyed_alike_on_web_and_mac() {
        for (id, label, icon) in ICONS {
            let spec = command(id).unwrap_or_else(|| panic!("{id} is not registered"));
            assert!(!label.is_empty() && !icon.is_empty() && !icon.contains('_'));
            assert_eq!(
                spec.shortcuts.for_platform(Platform::Web),
                spec.shortcuts.for_platform(Platform::Mac),
                "{id}"
            );
        }
        for context in ToolContext::ALL {
            for id in context.commands() {
                assert!(ICONS.iter().any(|(i, _, _)| i == id), "{id}");
            }
        }
        for spec in registry() {
            if tool_of(&spec.id).is_some() {
                assert!(
                    ToolContext::ALL
                        .iter()
                        .any(|c| c.commands().contains(&spec.id.as_str())),
                    "{} is in no context",
                    spec.id
                );
            }
        }
        assert_eq!(Tool::ALL.len(), 8);
        assert!(
            Tool::ALL
                .iter()
                .all(|t| tool_of(&format!("tool.{}", t.name())) == Some(*t))
        );
    }

    #[test]
    fn the_context_follows_focus_kind_and_language() {
        let mut shell = Shell::new(fixture_session()).unwrap();
        let tab = CommandContext::default();
        assert_eq!(shell.tool_well(&tab).context, ToolContext::Translation);
        shell.set_resource_languages(
            [
                ("sblgnt".to_owned(), "grc".to_owned()),
                ("bsb".to_owned(), "en".to_owned()),
            ]
            .into(),
        );
        let focus = |n| {
            Invocation::with(
                "column.focus",
                crate::commands::CommandArgs {
                    column: Some(n),
                    ..Default::default()
                },
            )
        };
        shell.apply(&focus(2), &tab).unwrap();
        assert_eq!(shell.tool_well(&tab).context, ToolContext::Original);
        shell.apply(&focus(3), &tab).unwrap();
        assert_eq!(shell.tool_well(&tab).context, ToolContext::Reference);
        let dock = CommandContext {
            focus: Focus::NotesDock,
            ..Default::default()
        };
        assert_eq!(shell.tool_well(&dock).context, ToolContext::Notes);
        let empty = CommandContext {
            has_selection: true,
            focus: Focus::Empty,
        };
        let well = shell.tool_well(&empty);
        assert_eq!(well.context, ToolContext::None);
        assert_eq!(
            well.groups.len(),
            1,
            "no selection group with nothing focused"
        );
        assert!(is_original_language("hbo-Hebr") && is_original_language("HE"));
        assert!(!is_original_language("en") && !is_original_language("el"));
    }

    #[test]
    fn the_stored_tool_survives_contexts_that_lack_it() {
        let mut shell = Shell::new(fixture_session()).unwrap();
        let tab = CommandContext::default();
        let dock = CommandContext {
            focus: Focus::NotesDock,
            ..Default::default()
        };
        shell.apply(&Invocation::new("tool.pen"), &tab).unwrap();
        let notes = shell.tool_well(&dock);
        assert_eq!((notes.tool, notes.effective_tool), (Tool::Pen, Tool::Type));
        assert!(notes.groups[0].items[0].active);
        assert_eq!(
            shell
                .apply(&Invocation::new("tool.pen"), &dock)
                .unwrap_err()
                .to_string(),
            "tool.pen is disabled: Pen works on Bible text"
        );
        assert_eq!(shell.view().effective_tool, "pen");
        let well = shell.tool_well(&tab);
        assert_eq!(well.effective_tool, Tool::Pen);
        assert_eq!(
            ids(&well, ToolWellGroupKind::Ink),
            ["tool.swatch", "tool.swap_swatches", "tool.width"]
        );
        shell.apply(&Invocation::new("tool.type"), &dock).unwrap();
        assert_eq!(shell.tool_well(&tab).effective_tool, Tool::Select);
        assert_eq!(shell.tool_well(&dock).effective_tool, Tool::Type);
        let selected = CommandContext {
            has_selection: true,
            focus: Focus::NotesDock,
        };
        let well = shell.tool_well(&selected);
        assert_eq!(ids(&well, ToolWellGroupKind::Selection), SELECTION);
        assert!(well.groups.iter().all(|g| g.kind != ToolWellGroupKind::Ink));
        let link = well.groups[0]
            .items
            .iter()
            .find(|i| i.command_id == "note.link_passage")
            .unwrap();
        assert_eq!(link.disabled_text, None, "the host asks for the passage");
        assert_eq!(
            well.groups[0].items[1].key.as_deref(),
            Some("Cmd+B"),
            "bold"
        );
    }
}
