//! The palette query of specs/22 §3 (B070): one field that takes a passage, a command, a tab
//! or workspace name, or words to find in the text, answered by [`palette_query`] as one
//! ranked list in four sections, always in this order: Passage, Commands, Tabs and
//! workspaces (across every workspace of the session), Text.
//!
//! **Grammar** ([`parse_query`]). A leading `>` restricts to commands, `@` to tabs and
//! workspaces, `#` to text and `:` to a passage; plain typing searches all four. A trailing
//! ` @N` or ` @new` (after whitespace) targets column N or a new column for every result
//! that opens a passage.
//!
//! **Ranking** is deterministic. Text is folded to lower case with runs of whitespace as one
//! space, and a candidate string scores, best first: equal to the query 1000; starts with it
//! 900 minus the extra length (at most 99); a word of it starts with the query 800 minus 10
//! per earlier word (at most 90); every query word starts some word of it 700; contains the
//! query 600; holds the query's characters in order 300 minus the gaps (at most 200). A
//! command scores its best field: the title as is, an alias 30 lower, a keyword 100 lower and
//! only from 600 up. A command whose title or alias is followed by an argument
//! (`rename workspace Greek`, `move tab to column 3`, `set line spacing 1.7`, `link passage
//! Jn 3:16`) scores 950 and carries the argument, a passage as OSIS; a trailing number fills a column-number argument. A command typed without the column, tab or
//! workspace it needs expands into one concrete row per choice at the same score ("Move tab
//! to column 2", "Move tab to new column", "Go to BSB · Rom 8:28"), omitting the current
//! one; an empty `>` lists the commands themselves. Within a section, results sort by score,
//! then enabled before disabled, then registry order (commands) or session order
//! (workspaces, then tabs by workspace, column and position). A tab row reads
//! `resource · passage` with the resource's registered short name ([`Shell::set_resource_titles`],
//! else its id) and the short reference (`BSB · Jn 3:16`, `ReferenceSet::to_short`). A tab
//! matches its resource id and name, its passage as written, in display form (`John 3:16`),
//! short form (`Jn 3:16`) and with the book's full name (`Genesis 1:1`); it also scores 800 when the
//! query is a passage in the same book and chapter as its reference, 600 in the same book.
//! The Text section keeps the provider's order. When the notes dock or a Notes tab has
//! focus ([`PaletteContext::focus`]) and no column target is given, the Passage section adds
//! "Link John 3:16" (`note.link_passage`) after the navigation row (B076). Each section shows
//! [`DEFAULT_SECTION_LIMIT`] results (one passage) unless it is the expanded one.
//!
//! **Incremental.** Everything that depends only on the session (folded tab and workspace
//! strings, parsed tab references) is built once into a [`PaletteIndex`]; a host keeps it
//! while [`Shell::revision`] is unchanged and calls [`palette_query_indexed`] on every
//! keystroke. Command strings are folded once per process.
//!
//! **Text** comes from a [`TextProvider`]; B071 supplies the edition search, the conformance
//! crate a fixture provider over its fixture texts.

use openlogos_core::reference::{
    AliasTable, BookId, Reference, ReferenceRange, ReferenceSet, ReviewFlag, parse,
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::commands::{
    ArgumentShape, CommandArgs, CommandContext, DisabledReason, Focus, Invocation, Platform, Shell,
    registry,
};
use crate::shell::Session;

/// Results per section when the section is not expanded.
pub const DEFAULT_SECTION_LIMIT: usize = 5;

/// Results in an expanded section.
pub const EXPANDED_SECTION_LIMIT: usize = 50;

/// Characters of query text below which plain typing does not ask the text provider.
pub const TEXT_MIN_CHARS: usize = 2;

/// The four sections, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaletteSection {
    /// `parseReference` succeeded.
    Passage,
    /// Registered commands.
    Commands,
    /// Tabs and workspaces across every workspace.
    Tabs,
    /// Full-text hits.
    Text,
}

/// Which sections a query searches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaletteMode {
    /// Plain typing: every section.
    All,
    /// `>`.
    Commands,
    /// `@`.
    Tabs,
    /// `#`.
    Text,
    /// `:`.
    Passage,
}

/// Where a result that opens a passage opens it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ColumnTarget {
    /// ` @N`.
    Column {
        /// The column number.
        number: u32,
    },
    /// ` @new`.
    New,
}

/// A query split by the grammar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedQuery {
    /// The sections searched.
    pub mode: PaletteMode,
    /// The query without its prefix or target, trimmed.
    pub text: String,
    /// The column target, if any.
    pub target: Option<ColumnTarget>,
}

/// Splits palette input by the grammar of specs/22 §3.
#[must_use]
pub fn parse_query(input: &str) -> ParsedQuery {
    let trimmed = input.trim_start();
    let (mode, rest) = match trimmed.chars().next() {
        Some('>') => (PaletteMode::Commands, &trimmed[1..]),
        Some('@') => (PaletteMode::Tabs, &trimmed[1..]),
        Some('#') => (PaletteMode::Text, &trimmed[1..]),
        Some(':') => (PaletteMode::Passage, &trimmed[1..]),
        _ => (PaletteMode::All, trimmed),
    };
    let mut text = rest.trim();
    let mut target = None;
    if let Some(at) = text.rfind('@')
        && at > 0
        && text[..at].ends_with(char::is_whitespace)
    {
        let suffix = &text[at + 1..];
        let parsed = if suffix.eq_ignore_ascii_case("new") {
            Some(ColumnTarget::New)
        } else if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
            suffix
                .parse::<u32>()
                .ok()
                .map(|number| ColumnTarget::Column { number })
        } else {
            None
        };
        if parsed.is_some() {
            target = parsed;
            text = text[..at].trim_end();
        }
    }
    ParsedQuery {
        mode,
        text: text.to_owned(),
        target,
    }
}

/// What the host knows for a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaletteContext {
    /// Whether text is selected in some tab (enables selection commands).
    pub has_selection: bool,
    /// Whose shortcuts to show.
    pub platform: Platform,
    /// The section showing [`EXPANDED_SECTION_LIMIT`] results.
    pub expanded: Option<PaletteSection>,
    /// Where focus is (B076): tool and note commands are disabled outside their context,
    /// and in notes a passage also offers Link passage.
    pub focus: Focus,
}

impl Default for PaletteContext {
    fn default() -> Self {
        PaletteContext {
            has_selection: false,
            platform: Platform::Web,
            expanded: None,
            focus: Focus::Tab,
        }
    }
}

/// One full-text hit from a [`TextProvider`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextHit {
    /// The edition.
    pub edition_id: String,
    /// The passage, OSIS.
    pub reference: String,
    /// A snippet around the hit.
    pub snippet: String,
}

/// Supplies the Text section. Called on every keystroke that reaches it, so it should
/// answer from an index; results are shown in the order returned.
pub trait TextProvider {
    /// At most `limit` hits for `query`, best first.
    fn search(&self, query: &str, limit: usize) -> Vec<TextHit>;
}

/// One result row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaletteItem {
    /// Its section.
    pub section: PaletteSection,
    /// The main line.
    pub title: String,
    /// The second line, if any.
    pub subtitle: Option<String>,
    /// The ranking score (see the module documentation).
    pub score: u32,
    /// What Enter dispatches.
    pub invocation: Invocation,
    /// Why Enter would do nothing now, if so.
    pub disabled: Option<DisabledReason>,
    /// The same reason in words for the row ("Choose a tab"), so no host copies the core's
    /// wording.
    pub disabled_text: Option<String>,
    /// The platform's default shortcut, for a command.
    pub shortcut: Option<String>,
}

/// One section of results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaletteGroup {
    /// The section.
    pub section: PaletteSection,
    /// Its results, capped.
    pub items: Vec<PaletteItem>,
    /// How many matched before the cap.
    pub total: u32,
}

/// The answer to one query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaletteResult {
    /// The query as the grammar split it.
    pub query: ParsedQuery,
    /// Non-empty sections, in section order.
    pub groups: Vec<PaletteGroup>,
    /// A message for the field, such as why `:` text is not a passage.
    pub notice: Option<String>,
}

impl PaletteResult {
    /// Every item, in display order.
    pub fn items(&self) -> impl Iterator<Item = &PaletteItem> {
        self.groups.iter().flat_map(|g| g.items.iter())
    }
}

fn english() -> &'static AliasTable {
    static TABLE: OnceLock<AliasTable> = OnceLock::new();
    TABLE.get_or_init(AliasTable::english)
}

/// Lower case with runs of whitespace as one space.
fn fold(text: &str) -> String {
    text.split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Scores one folded candidate against one folded, non-empty query.
fn match_score(query: &str, candidate: &str) -> Option<u32> {
    if candidate == query {
        return Some(1000);
    }
    if candidate.starts_with(query) {
        return Some(900 - (candidate.len() - query.len()).min(99) as u32);
    }
    let word_starts = || {
        std::iter::once(0).chain(
            candidate
                .match_indices(' ')
                .map(|(i, _)| i + 1)
                .filter(|i| *i < candidate.len()),
        )
    };
    if let Some(word) = word_starts().position(|i| candidate[i..].starts_with(query)) {
        return Some(800 - (word.min(9) as u32) * 10);
    }
    let words: Vec<&str> = candidate.split(' ').collect();
    if query.contains(' ')
        && query
            .split(' ')
            .all(|q| words.iter().any(|w| w.starts_with(q)))
    {
        return Some(700);
    }
    if candidate.contains(query) {
        return Some(600);
    }
    // Positions are in characters, so the span never splits a multi-byte character.
    let mut chars = candidate.chars().enumerate();
    let mut first = None;
    let mut last = 0;
    let mut wanted = 0;
    for q in query.chars().filter(|c| *c != ' ') {
        let (i, _) = chars.by_ref().find(|(_, c)| *c == q)?;
        first.get_or_insert(i);
        last = i;
        wanted += 1;
    }
    let gaps = (last + 1 - first?).saturating_sub(wanted);
    Some(300 - gaps.min(200) as u32)
}

struct FoldedCommand {
    index: usize,
    title: String,
    aliases: Vec<String>,
    keywords: Vec<String>,
}

fn folded_commands() -> &'static [FoldedCommand] {
    static FOLDED: OnceLock<Vec<FoldedCommand>> = OnceLock::new();
    FOLDED.get_or_init(|| {
        registry()
            .iter()
            .enumerate()
            .map(|(index, c)| FoldedCommand {
                index,
                title: fold(&c.title),
                aliases: c.aliases.iter().map(|a| fold(a)).collect(),
                keywords: c.keywords.iter().map(|k| fold(k)).collect(),
            })
            .collect()
    })
}

struct IndexedTab {
    workspace_id: String,
    workspace_title: String,
    tab_id: String,
    column_number: usize,
    title: String,
    folded: Vec<String>,
    reference: Option<Reference>,
}

struct IndexedWorkspace {
    id: String,
    title: String,
    folded: String,
    columns: usize,
    tabs: usize,
}

/// What the palette precomputes from a session; valid until the session changes.
pub struct PaletteIndex {
    workspaces: Vec<IndexedWorkspace>,
    tabs: Vec<IndexedTab>,
}

impl PaletteIndex {
    /// Indexes every workspace and tab of the session.
    #[must_use]
    pub fn new(session: &Session) -> PaletteIndex {
        let table = english();
        let mut workspaces = Vec::with_capacity(session.workspaces.len());
        let mut tabs = Vec::new();
        for ws in &session.workspaces {
            workspaces.push(IndexedWorkspace {
                id: ws.id.clone(),
                title: ws.title.clone(),
                folded: fold(&ws.title),
                columns: ws.columns.len(),
                tabs: ws.tabs().count(),
            });
            for (c, column) in ws.columns.iter().enumerate() {
                for tab in &column.tabs {
                    let parsed = tab.reference.as_deref().and_then(parse_tab_reference);
                    let human = parsed.as_ref().map(|set| set.to_human(table));
                    let short = parsed.as_ref().map(ReferenceSet::to_short);
                    let resource = session.resource_title(&tab.content_id);
                    let title = match &short {
                        Some(s) => format!("{resource} · {s}"),
                        None => resource.to_owned(),
                    };
                    // The row title too, so words can span resource and passage (`sblgnt jn`).
                    let mut folded = vec![fold(&tab.content_id), fold(&title)];
                    if resource != tab.content_id {
                        folded.push(fold(resource));
                    }
                    folded.extend(human.as_deref().map(fold));
                    folded.extend(short.as_deref().map(fold));
                    // The book's full name too, so `@genesis` finds a tab at `Gen 1:1`.
                    if let (Some(set), Some(h)) = (&parsed, &human) {
                        let book = set.ranges[0].start.book;
                        if let Some(long) = table
                            .books
                            .iter()
                            .find(|b| b.book == book)
                            .and_then(|b| b.aliases.first())
                        {
                            folded.push(fold(&h.replacen(table.display_name(book), long, 1)));
                        }
                    }
                    folded.extend(tab.reference.as_deref().map(fold));
                    tabs.push(IndexedTab {
                        workspace_id: ws.id.clone(),
                        workspace_title: ws.title.clone(),
                        tab_id: tab.id.clone(),
                        column_number: c + 1,
                        title,
                        folded,
                        reference: parsed.map(|set| set.ranges[0].start.clone()),
                    });
                }
            }
        }
        PaletteIndex { workspaces, tabs }
    }
}

/// A tab's stored reference parsed for display: the OSIS fast path ([`osis_verse`]) for the
/// `Book.C.V` form tabs store, else the full parser over the English table built once per
/// process. `None` when it does not parse or names no range. Shared by the palette index
/// and [`crate::view`], both of which run on every render or keystroke.
#[must_use]
pub fn parse_tab_reference(text: &str) -> Option<ReferenceSet> {
    osis_verse(text)
        .or_else(|| parse(text, english()).ok())
        .filter(|set| !set.ranges.is_empty())
}

/// A single `Book.C` or `Book.C.V` OSIS reference without the alias table, which is what
/// tabs store; anything else goes through [`parse`]. Indexing 120 tabs through the full
/// parser cost about 5 ms (docs/evidence/B070.md), most of a frame.
fn osis_verse(text: &str) -> Option<ReferenceSet> {
    let mut parts = text.split('.');
    let book = BookId::from_osis(parts.next()?)?;
    let chapter: u32 = parts.next()?.parse().ok().filter(|c| *c > 0)?;
    let (reference, flags) = match parts.next() {
        Some(verse) => (
            Reference::verse(book, chapter, verse.parse().ok()?),
            Vec::new(),
        ),
        None => (
            Reference::chapter(book, chapter),
            vec![ReviewFlag::ChapterOnly { range: 0 }],
        ),
    };
    if parts.next().is_some() {
        return None;
    }
    Some(ReferenceSet {
        ranges: vec![ReferenceRange::single(reference)],
        flags,
    })
}

/// `1 column`, `3 columns`.
fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

fn target_args(target: Option<ColumnTarget>, reference: String) -> CommandArgs {
    CommandArgs {
        reference: Some(reference),
        column: match target {
            Some(ColumnTarget::Column { number }) => Some(number),
            _ => None,
        },
        new_column: target == Some(ColumnTarget::New),
        ..CommandArgs::default()
    }
}

fn target_subtitle(target: Option<ColumnTarget>) -> String {
    match target {
        Some(ColumnTarget::Column { number }) => format!("Open in column {number}"),
        Some(ColumnTarget::New) => "Open in a new column".to_owned(),
        None => "Open in the active tab".to_owned(),
    }
}

/// An argument for a command from the text after one of its names.
fn argument_from(shape: ArgumentShape, rest: &str) -> Option<CommandArgs> {
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    let mut args = CommandArgs::default();
    match shape {
        ArgumentShape::ColumnNumber => args.column = Some(rest.parse().ok()?),
        ArgumentShape::Text => args.text = Some(rest.to_owned()),
        ArgumentShape::LinkGroup => args.group = Some(rest.to_owned()),
        ArgumentShape::Number => {
            args.number = Some(rest.parse().ok().filter(|n: &f64| n.is_finite())?)
        }
        ArgumentShape::Reference => {
            let set = parse(rest, english())
                .ok()
                .filter(|s| !s.ranges.is_empty())?;
            args.reference = Some(set.to_osis());
        }
        _ => return None,
    }
    Some(args)
}

fn command_items(
    shell: &Shell,
    index: &PaletteIndex,
    query: &ParsedQuery,
    context: &PaletteContext,
) -> Vec<(PaletteItem, usize)> {
    let folded_query = fold(&query.text);
    // A trailing number fills a column-number argument: `focus 2`, `move tab to 3`.
    let (trailing_head, trailing_number) = match folded_query.rsplit_once(' ') {
        Some((head, n)) if n.chars().all(|c| c.is_ascii_digit()) => {
            (Some(head), n.parse::<u32>().ok())
        }
        _ => (None, None),
    };
    let command_context = CommandContext {
        has_selection: context.has_selection,
        focus: context.focus,
    };
    let undo_title = shell.undo_stack.last().map(|r| r.title.clone());
    let redo_title = shell.redo_stack.last().map(|r| r.title.clone());
    let specs = registry();
    let mut items = Vec::new();
    for folded in folded_commands() {
        let spec = &specs[folded.index];
        let mut best: Option<(u32, CommandArgs)> = None;
        let mut consider = |score: u32, args: CommandArgs| {
            if best.as_ref().is_none_or(|(s, _)| score > *s) {
                best = Some((score, args));
            }
        };
        if folded_query.is_empty() {
            consider(0, CommandArgs::default());
        } else {
            let names =
                std::iter::once((&folded.title, 0)).chain(folded.aliases.iter().map(|a| (a, 30)));
            for (name, penalty) in names.clone() {
                if let Some(score) = match_score(&folded_query, name) {
                    consider(score - penalty.min(score), CommandArgs::default());
                }
                if spec.argument != ArgumentShape::None
                    && let Some(rest) = query_rest_after(&query.text, name)
                    && let Some(args) = argument_from(spec.argument, rest)
                {
                    consider(950 - penalty, args);
                }
            }
            for keyword in &folded.keywords {
                if let Some(score) = match_score(&folded_query, keyword).filter(|s| *s >= 600) {
                    consider(score - 100, CommandArgs::default());
                }
            }
            if spec.argument == ArgumentShape::ColumnNumber
                && let (Some(head), Some(number)) = (trailing_head, trailing_number)
            {
                for (name, penalty) in names {
                    if let Some(score) = match_score(head, name) {
                        consider(
                            score.saturating_sub(penalty),
                            CommandArgs {
                                column: Some(number),
                                ..CommandArgs::default()
                            },
                        );
                    }
                }
            }
        }
        let Some((score, args)) = best else {
            continue;
        };
        let shortcut = spec
            .shortcuts
            .for_platform(context.platform)
            .first()
            .cloned();
        if !folded_query.is_empty() && args == CommandArgs::default() {
            let rows = expansions(shell, index, &spec.id);
            if !rows.is_empty() {
                for (i, (title, subtitle, args)) in rows.into_iter().enumerate() {
                    let args_shown = args.clone();
                    let invocation = Invocation::with(&spec.id, args);
                    let disabled = shell.enabled(&invocation, &command_context).err();
                    items.push((
                        PaletteItem {
                            section: PaletteSection::Commands,
                            title,
                            subtitle,
                            score,
                            invocation,
                            disabled_text: disabled.as_ref().map(ToString::to_string),
                            disabled,
                            shortcut: concrete_shortcut(shortcut.as_deref(), &args_shown),
                        },
                        folded.index * 1000 + i,
                    ));
                }
                continue;
            }
        }
        let shortcut = concrete_shortcut(shortcut.as_deref(), &args);
        let invocation = Invocation::with(&spec.id, args);
        let disabled = shell.enabled(&invocation, &command_context).err();
        let subtitle = match spec.id.as_str() {
            "edit.undo" => undo_title.clone(),
            "edit.redo" => redo_title.clone(),
            _ => None,
        };
        items.push((
            PaletteItem {
                section: PaletteSection::Commands,
                title: spec.title.clone(),
                subtitle,
                score,
                invocation,
                disabled_text: disabled.as_ref().map(ToString::to_string),
                disabled,
                shortcut,
            },
            folded.index * 1000,
        ));
    }
    items
}

/// The shortcut a row shows: a `{n}` template filled with the row's column (1–9), else
/// none (a new column, a column past 9, or no column given); any other shortcut as is.
fn concrete_shortcut(template: Option<&str>, args: &CommandArgs) -> Option<String> {
    let template = template?;
    if !template.contains("{n}") {
        return Some(template.to_owned());
    }
    match args.column {
        Some(n @ 1..=9) if !args.new_column => Some(template.replace("{n}", &n.to_string())),
        _ => None,
    }
}

/// Concrete rows for a command typed without the argument it needs, so the palette offers
/// "Move tab to column 2" instead of a disabled "Move tab to column": one row per other
/// column (and a new column where the command can make one) for the column commands, one
/// per other tab for Go to tab, one per other workspace for Switch to workspace. Empty for
/// every other command, or when there is nothing to offer.
fn expansions(
    shell: &Shell,
    index: &PaletteIndex,
    command_id: &str,
) -> Vec<(String, Option<String>, CommandArgs)> {
    let session = &shell.session;
    let ws = session.active();
    let active = ws.active_index() + 1;
    let column = |number: usize| CommandArgs {
        column: Some(number as u32),
        ..CommandArgs::default()
    };
    let new_column = CommandArgs {
        new_column: true,
        ..CommandArgs::default()
    };
    let other_columns = (1..=ws.columns.len()).filter(|n| *n != active);
    match command_id {
        "column.focus" => other_columns
            .map(|n| (format!("Focus column {n}"), None, column(n)))
            .collect(),
        "tab.move_to_column" => {
            let mut rows: Vec<_> = other_columns
                .map(|n| (format!("Move tab to column {n}"), None, column(n)))
                .collect();
            if ws.active_column().tabs.len() > 1 {
                rows.push(("Move tab to new column".to_owned(), None, new_column));
            }
            rows
        }
        "tab.duplicate_to_column" => {
            let mut rows: Vec<_> = other_columns
                .map(|n| (format!("Duplicate tab into column {n}"), None, column(n)))
                .collect();
            rows.push(("Duplicate tab into new column".to_owned(), None, new_column));
            rows
        }
        "tab.focus" => {
            let current = &ws.active_tab().id;
            index
                .tabs
                .iter()
                .filter(|t| !(t.workspace_id == ws.id && &t.tab_id == current))
                .map(|t| {
                    (
                        format!("Go to {}", t.title),
                        Some(format!(
                            "{} · column {}",
                            t.workspace_title, t.column_number
                        )),
                        CommandArgs {
                            workspace_id: Some(t.workspace_id.clone()),
                            tab_id: Some(t.tab_id.clone()),
                            ..CommandArgs::default()
                        },
                    )
                })
                .collect()
        }
        "workspace.switch" => index
            .workspaces
            .iter()
            .filter(|w| w.id != ws.id)
            .map(|w| {
                (
                    format!("Switch to {}", w.title),
                    None,
                    CommandArgs {
                        workspace_id: Some(w.id.clone()),
                        ..CommandArgs::default()
                    },
                )
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The original-case text after `name` when the folded query starts with `name` and a
/// space, so an argument keeps its case (`rename workspace Greek`).
fn query_rest_after<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let words: Vec<&str> = name.split(' ').collect();
    let mut rest = text.trim_start();
    for word in words {
        let head: String = rest.chars().take(word.chars().count()).collect();
        if head.to_lowercase() != word {
            return None;
        }
        let after = &rest[head.len()..];
        if !after.starts_with(char::is_whitespace) {
            return None;
        }
        rest = after.trim_start();
    }
    Some(rest)
}

fn tab_items(
    index: &PaletteIndex,
    query: &ParsedQuery,
    passage: Option<&Reference>,
) -> Vec<(PaletteItem, usize)> {
    let folded_query = fold(&query.text);
    let mut items = Vec::new();
    let mut order = 0;
    for ws in &index.workspaces {
        let score = if folded_query.is_empty() {
            Some(0)
        } else {
            match_score(&folded_query, &ws.folded)
        };
        if let Some(score) = score {
            items.push((
                PaletteItem {
                    section: PaletteSection::Tabs,
                    title: ws.title.clone(),
                    subtitle: Some(format!(
                        "Workspace · {}, {}",
                        count(ws.columns, "column"),
                        count(ws.tabs, "tab")
                    )),
                    score,
                    invocation: Invocation::with(
                        "workspace.switch",
                        CommandArgs {
                            workspace_id: Some(ws.id.clone()),
                            ..CommandArgs::default()
                        },
                    ),
                    disabled: None,
                    disabled_text: None,
                    shortcut: None,
                },
                order,
            ));
        }
        order += 1;
    }
    for tab in &index.tabs {
        let mut score = if folded_query.is_empty() {
            Some(0)
        } else {
            tab.folded
                .iter()
                .filter_map(|f| match_score(&folded_query, f))
                .max()
        };
        if let (Some(wanted), Some(have)) = (passage, &tab.reference)
            && wanted.book == have.book
        {
            let by_passage = if wanted.chapter == have.chapter {
                800
            } else {
                600
            };
            score = Some(score.unwrap_or(0).max(by_passage));
        }
        if let Some(score) = score {
            items.push((
                PaletteItem {
                    section: PaletteSection::Tabs,
                    title: tab.title.clone(),
                    subtitle: Some(format!(
                        "{} · column {}",
                        tab.workspace_title, tab.column_number
                    )),
                    score,
                    invocation: Invocation::with(
                        "tab.focus",
                        CommandArgs {
                            workspace_id: Some(tab.workspace_id.clone()),
                            tab_id: Some(tab.tab_id.clone()),
                            ..CommandArgs::default()
                        },
                    ),
                    disabled: None,
                    disabled_text: None,
                    shortcut: None,
                },
                order,
            ));
        }
        order += 1;
    }
    items
}

fn group(
    section: PaletteSection,
    mut items: Vec<(PaletteItem, usize)>,
    context: &PaletteContext,
) -> Option<PaletteGroup> {
    if items.is_empty() {
        return None;
    }
    items.sort_by(|(a, ai), (b, bi)| {
        b.score
            .cmp(&a.score)
            .then(a.disabled.is_some().cmp(&b.disabled.is_some()))
            .then(ai.cmp(bi))
    });
    let total = items.len() as u32;
    let limit = if context.expanded == Some(section) {
        EXPANDED_SECTION_LIMIT
    } else {
        DEFAULT_SECTION_LIMIT
    };
    Some(PaletteGroup {
        section,
        items: items
            .into_iter()
            .take(limit)
            .map(|(item, _)| item)
            .collect(),
        total,
    })
}

/// [`palette_query_indexed`] with an index built for this call.
#[must_use]
pub fn palette_query(
    shell: &Shell,
    input: &str,
    context: &PaletteContext,
    text: Option<&dyn TextProvider>,
) -> PaletteResult {
    palette_query_indexed(
        shell,
        &PaletteIndex::new(&shell.session),
        input,
        context,
        text,
    )
}

/// Answers one palette query. `index` must have been built from `shell.session` at its
/// current revision.
#[must_use]
pub fn palette_query_indexed(
    shell: &Shell,
    index: &PaletteIndex,
    input: &str,
    context: &PaletteContext,
    text: Option<&dyn TextProvider>,
) -> PaletteResult {
    let query = parse_query(input);
    let mode = query.mode;
    let blank = query.text.is_empty();
    let mut groups = Vec::new();
    let mut notice = None;
    let command_context = CommandContext {
        has_selection: context.has_selection,
        focus: context.focus,
    };

    let parsed: Option<ReferenceSet> = if blank {
        None
    } else {
        match parse(&query.text, english()) {
            Ok(set) if !set.ranges.is_empty() => Some(set),
            Ok(_) => None,
            Err(error) => {
                if mode == PaletteMode::Passage {
                    notice = Some(error.to_string());
                }
                None
            }
        }
    };

    if matches!(mode, PaletteMode::All | PaletteMode::Passage)
        && let Some(set) = &parsed
    {
        let invocation = Invocation::with("nav.navigate", target_args(query.target, set.to_osis()));
        let disabled = shell.enabled(&invocation, &command_context).err();
        let mut items = vec![(
            PaletteItem {
                section: PaletteSection::Passage,
                title: set.to_human(english()),
                subtitle: Some(target_subtitle(query.target)),
                score: 1000,
                invocation,
                disabled_text: disabled.as_ref().map(ToString::to_string),
                disabled,
                shortcut: None,
            },
            0,
        )];
        // B076: in notes a passage can also be linked into the note.
        if query.target.is_none()
            && crate::well::tool_context(&shell.session, &command_context)
                == crate::well::ToolContext::Notes
        {
            let invocation = Invocation::with(
                "note.link_passage",
                CommandArgs {
                    reference: Some(set.to_osis()),
                    ..CommandArgs::default()
                },
            );
            let disabled = shell.enabled(&invocation, &command_context).err();
            items.push((
                PaletteItem {
                    section: PaletteSection::Passage,
                    title: format!("Link {}", set.to_human(english())),
                    subtitle: Some("Insert a link to it in the note".to_owned()),
                    score: 1000,
                    invocation,
                    disabled_text: disabled.as_ref().map(ToString::to_string),
                    disabled,
                    shortcut: None,
                },
                1,
            ));
        }
        groups.extend(group(PaletteSection::Passage, items, context));
    }

    if mode == PaletteMode::Commands || (mode == PaletteMode::All && !blank) {
        groups.extend(group(
            PaletteSection::Commands,
            command_items(shell, index, &query, context),
            context,
        ));
    }

    if mode == PaletteMode::Tabs || (mode == PaletteMode::All && !blank) {
        let passage = parsed.as_ref().map(|set| &set.ranges[0].start);
        groups.extend(group(
            PaletteSection::Tabs,
            tab_items(index, &query, passage),
            context,
        ));
    }

    let wants_text = mode == PaletteMode::Text
        || (mode == PaletteMode::All && query.text.chars().count() >= TEXT_MIN_CHARS);
    if wants_text
        && !blank
        && let Some(provider) = text
    {
        let limit = if context.expanded == Some(PaletteSection::Text) {
            EXPANDED_SECTION_LIMIT
        } else {
            DEFAULT_SECTION_LIMIT
        };
        let items: Vec<(PaletteItem, usize)> = provider
            .search(&query.text, limit)
            .into_iter()
            .enumerate()
            .map(|(i, hit)| {
                let short = osis_verse(&hit.reference)
                    .or_else(|| parse(&hit.reference, english()).ok())
                    .map_or_else(|| hit.reference.clone(), |set| set.to_short());
                let invocation = Invocation::with(
                    "nav.navigate",
                    target_args(query.target, hit.reference.clone()),
                );
                let disabled = shell.enabled(&invocation, &command_context).err();
                (
                    PaletteItem {
                        section: PaletteSection::Text,
                        title: format!(
                            "{} · {short}",
                            shell.session.resource_title(&hit.edition_id)
                        ),
                        subtitle: Some(hit.snippet),
                        score: 0,
                        invocation,
                        disabled_text: disabled.as_ref().map(ToString::to_string),
                        disabled,
                        shortcut: None,
                    },
                    i,
                )
            })
            .collect();
        groups.extend(group(PaletteSection::Text, items, context));
    }

    PaletteResult {
        query,
        groups,
        notice,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::tests::fixture_session;

    #[test]
    fn the_grammar_splits_prefix_text_and_target() {
        let q = |s: &str| parse_query(s);
        assert_eq!(q("Jn 3:16").mode, PaletteMode::All);
        assert_eq!(q(">new tab").mode, PaletteMode::Commands);
        assert_eq!(q(">new tab").text, "new tab");
        assert_eq!(q("@gen").mode, PaletteMode::Tabs);
        assert_eq!(q("#agape @3").mode, PaletteMode::Text);
        assert_eq!(
            q("#agape @3").target,
            Some(ColumnTarget::Column { number: 3 })
        );
        assert_eq!(q("#agape @3").text, "agape");
        assert_eq!(q(": Jn 3:16 @new").target, Some(ColumnTarget::New));
        assert_eq!(q(": Jn 3:16 @NEW").text, "Jn 3:16");
        // `@2` with no space before it is a tab query, not a target.
        assert_eq!(q("@2").target, None);
        assert_eq!(q("@2").text, "2");
        assert_eq!(q("a@2").target, None);
        assert_eq!(q("x @two").target, None);
        assert_eq!(q("x @two").text, "x @two");
    }

    #[test]
    fn the_osis_fast_path_agrees_with_the_parser() {
        for text in ["John.3.16", "Gen.1", "1John.4.8", "Ps.119.105", "Rev.22.21"] {
            assert_eq!(osis_verse(text), parse(text, english()).ok(), "{text}");
        }
        for text in [
            "John.3.16-John.3.17",
            "John.0.1",
            "Nope.1.1",
            "John",
            "fixture:passage:1",
        ] {
            assert!(osis_verse(text).is_none(), "{text}");
        }
    }

    #[test]
    fn match_scores_follow_the_documented_order() {
        let s = |q: &str, c: &str| match_score(q, c);
        assert_eq!(s("new tab", "new tab"), Some(1000));
        assert_eq!(s("new", "new tab"), Some(896));
        assert_eq!(s("tab", "new tab"), Some(790));
        assert_eq!(s("tab cl", "close tab"), Some(700));
        assert_eq!(s("ab", "new tab"), Some(600));
        assert_eq!(s("ntb", "new tab"), Some(296));
        assert_eq!(s("xyz", "new tab"), None);
        // Character positions, not bytes: no panic on Greek or Hebrew.
        assert!(s("λγ", "ὁ λόγος").is_some());
        assert!(s("בא", "בְּרֵאשִׁית").is_some());
    }

    #[test]
    fn results_are_deterministic_and_the_index_matches_a_fresh_one() {
        let shell = Shell::new(fixture_session()).unwrap();
        let index = PaletteIndex::new(&shell.session);
        let context = PaletteContext::default();
        for query in [
            "", "j", "jn", "jn 3", "new", ">", "@", "#lo", "to", "col 2", ":Gen 1",
        ] {
            let a = palette_query(&shell, query, &context, None);
            let b = palette_query_indexed(&shell, &index, query, &context, None);
            assert_eq!(a, b, "{query}");
            let sections: Vec<PaletteSection> = a.groups.iter().map(|g| g.section).collect();
            let mut sorted = sections.clone();
            sorted.sort();
            assert_eq!(sections, sorted, "{query}: sections out of order");
            for g in &a.groups {
                assert!(g.items.len() <= DEFAULT_SECTION_LIMIT);
                assert!(g.items.windows(2).all(|w| w[0].score >= w[1].score));
            }
        }
    }

    #[test]
    fn every_command_is_reachable_by_its_title() {
        let shell = Shell::new(fixture_session()).unwrap();
        let context = PaletteContext {
            expanded: Some(PaletteSection::Commands),
            ..PaletteContext::default()
        };
        for spec in registry() {
            let result = palette_query(&shell, &format!(">{}", spec.title), &context, None);
            let first = result.items().next().unwrap();
            assert_eq!(first.invocation.command_id, spec.id, "{}", spec.title);
            assert_eq!(first.score, 1000);
        }
    }

    #[test]
    fn shortcuts_follow_the_platform() {
        let shell = Shell::new(fixture_session()).unwrap();
        let mac = PaletteContext {
            platform: Platform::Mac,
            ..PaletteContext::default()
        };
        let item = palette_query(&shell, ">new tab", &mac, None);
        assert_eq!(
            item.items().next().unwrap().shortcut.as_deref(),
            Some("Alt+T")
        );
        let undo = palette_query(&shell, ">undo", &mac, None);
        assert_eq!(
            undo.items().next().unwrap().shortcut.as_deref(),
            Some("Cmd+Z")
        );
    }
}
