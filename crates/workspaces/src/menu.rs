//! The OpenLogos context menu of specs/22 §4.2 (B077): what a right-click, or the keyboard's
//! menu key, offers for what was clicked, decided here so the web shell and the Mac app show
//! the same menu.
//!
//! **Targets.** The host says what was clicked ([`ContextTarget`]): a word or a selection in
//! reading text, a plain spot, a selection or a passage link in a note, a tab, a column's
//! identity row, or the empty area of a column. A reading or note target names the tab it is
//! in (`tab_id`; absent means the active tab, or the notes dock for a note). A word's language
//! is its resource's registered language ([`Shell::set_resource_languages`]).
//!
//! **Focus first.** Tab, column and note commands act on what has focus, so a menu on
//! something that does not have it carries [`ContextMenu::focus`]: the `tab.focus` or
//! `column.focus` a click there would have run. The host applies it before the chosen item
//! (focus changes are not undoable) and dispatches every item under [`ContextMenu::context`].
//! Items are worked out on the session as it is after that focus change.
//!
//! **Items** carry the command with its arguments filled from the target (the clicked word's
//! passage, the tab's column), the label, the default key as the tool well shows it (`{n}`
//! filled with the column), a neutral icon name ([`MENU_ICONS`]), whether it is the current
//! choice (`active`: the theme, the swatch, the link mode) and why it cannot run, in words.
//! Disabled items stay visible; a section that does not apply (Open without a passage) is
//! left out. An item with a `submenu` carries its first enabled child's command, which a host
//! may run on a click where its platform allows; otherwise the click opens the submenu.
//!
//! **Words.** The host selects a word when it opens the menu on it, as macOS does, so the
//! word menu's Mark items are the selection commands.
//!
//! **Editions** are the resources with a registered language. Compare offers the others,
//! leaving out an original-language edition that cannot hold the passage (Greek outside the
//! New Testament, Hebrew and Aramaic outside the Old).
//!
//! The host performs `search.*`, `copy.*`, `host.*`, `nav.paste_passage` and the
//! `selection.*` and `note.*` items (`handled: false`); the core performs the tab, column,
//! navigation and view items. [`Shell::context_menu`] runs on every right-click; `just
//! bench-shell` times it.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::PaneKind;
use crate::commands::{
    CommandArgs, CommandContext, DisabledReason, Focus, Invocation, Shell, apply, command, enabled,
};
use crate::palette::{concrete_shortcut, human_reference, parse_tab_reference};
use crate::shell::{LinkMode, Session, THEMES, Tab};
use crate::view::reference_labels;

/// What was right-clicked, as the host knows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContextTarget {
    /// A word in reading text. The host selects it as it opens the menu.
    Word {
        /// The word as shown.
        text: String,
        /// Its passage, OSIS (`John.3.16`).
        #[serde(default)]
        reference: Option<String>,
        /// Its segment.
        #[serde(default)]
        segment_id: Option<String>,
        /// Where it starts in the segment, in scalar values.
        #[serde(default)]
        scalar_start: Option<u32>,
        /// Where it ends, exclusive.
        #[serde(default)]
        scalar_end: Option<u32>,
        /// Its edition; the tab's resource when absent.
        #[serde(default)]
        resource_id: Option<String>,
        /// The tab it is in; the active tab when absent.
        #[serde(default)]
        tab_id: Option<String>,
    },
    /// Selected reading text.
    Selection {
        /// The selected text.
        text: String,
        /// The passage it starts in.
        #[serde(default)]
        reference: Option<String>,
        /// The passage it ends in, when that is another.
        #[serde(default)]
        end_reference: Option<String>,
        /// Its edition; the tab's resource when absent.
        #[serde(default)]
        resource_id: Option<String>,
        /// The tab it is in; the active tab when absent.
        #[serde(default)]
        tab_id: Option<String>,
    },
    /// A spot in a note, or selected note text.
    NoteText {
        /// The selected text; `None` for a plain spot.
        #[serde(default)]
        text: Option<String>,
        /// The Notes tab; the notes dock when absent.
        #[serde(default)]
        tab_id: Option<String>,
    },
    /// A passage link in a note.
    NoteLink {
        /// The linked passage, OSIS.
        reference: String,
        /// The Notes tab; the notes dock when absent.
        #[serde(default)]
        tab_id: Option<String>,
    },
    /// A tab in a column's strip.
    Tab {
        /// The tab.
        tab_id: String,
    },
    /// A column's identity row or number badge.
    Column {
        /// The column number, 1-based.
        column: u32,
    },
    /// The empty area of a column.
    Background {
        /// The column number; the active column when absent.
        #[serde(default)]
        column: Option<u32>,
    },
}

/// Which kind of target a menu is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextTargetKind {
    /// A word in reading text.
    Word,
    /// Selected reading text.
    Selection,
    /// A spot or selection in a note.
    NoteText,
    /// A passage link in a note.
    NoteLink,
    /// A tab.
    Tab,
    /// A column's identity row.
    Column,
    /// The empty area of a column.
    Background,
}

impl ContextTargetKind {
    /// Every kind, in specs/22 §4.2 order.
    pub const ALL: [ContextTargetKind; 7] = [
        ContextTargetKind::Word,
        ContextTargetKind::Selection,
        ContextTargetKind::NoteText,
        ContextTargetKind::NoteLink,
        ContextTargetKind::Tab,
        ContextTargetKind::Column,
        ContextTargetKind::Background,
    ];

    /// The kind's name as serialized.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            ContextTargetKind::Word => "word",
            ContextTargetKind::Selection => "selection",
            ContextTargetKind::NoteText => "note_text",
            ContextTargetKind::NoteLink => "note_link",
            ContextTargetKind::Tab => "tab",
            ContextTargetKind::Column => "column",
            ContextTargetKind::Background => "background",
        }
    }
}

impl ContextTarget {
    /// Which kind of target it is.
    #[must_use]
    pub fn kind(&self) -> ContextTargetKind {
        match self {
            ContextTarget::Word { .. } => ContextTargetKind::Word,
            ContextTarget::Selection { .. } => ContextTargetKind::Selection,
            ContextTarget::NoteText { .. } => ContextTargetKind::NoteText,
            ContextTarget::NoteLink { .. } => ContextTargetKind::NoteLink,
            ContextTarget::Tab { .. } => ContextTargetKind::Tab,
            ContextTarget::Column { .. } => ContextTargetKind::Column,
            ContextTarget::Background { .. } => ContextTargetKind::Background,
        }
    }
}

/// One row of the menu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextMenuItem {
    /// The command a click dispatches (for a submenu, its first enabled child's).
    pub command_id: String,
    /// Its arguments, filled from the target.
    pub args: CommandArgs,
    /// What the row says.
    pub label: String,
    /// The default key (`Alt+W`, `Cmd+C`), the same on web and Mac; `None` for none.
    pub key: Option<String>,
    /// A neutral icon name ([`MENU_ICONS`]).
    pub icon: String,
    /// Whether it is the current choice: the theme, the swatch, the tab's link mode.
    pub active: bool,
    /// The value a row shows: a swatch as `#RRGGBB`.
    pub value: Option<String>,
    /// Why it cannot run now, in words; `None` when it can (or the host asks for a value).
    pub disabled_text: Option<String>,
    /// Its submenu, in order; empty for a plain row.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub submenu: Vec<ContextMenuItem>,
}

/// A run of rows between separators.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextMenuSection {
    /// A heading a host may show (`Search`, `Copy`); hosts that draw separators only may
    /// ignore it.
    pub label: Option<String>,
    /// Its rows.
    pub items: Vec<ContextMenuItem>,
}

/// The menu for one target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextMenu {
    /// What it is for.
    pub target_kind: ContextTargetKind,
    /// A header: the word, the selection, the tab, `Column 2`; `None` for none.
    pub title: Option<String>,
    /// The focus change to apply before the chosen item, if the target does not have focus.
    pub focus: Option<Invocation>,
    /// The context to dispatch every item under.
    pub context: CommandContext,
    /// Its sections, in order.
    pub sections: Vec<ContextMenuSection>,
}

/// Every icon the menu names, beyond the tool well's ([`crate::well::ICONS`]). The web shell
/// maps them to inline SVG, the Mac app to SF Symbols.
pub const MENU_ICONS: &[&str] = &[
    "search",
    "search-all",
    "lemma",
    "word-study",
    "find",
    "compare",
    "open",
    "column",
    "new-column",
    "cross-references",
    "highlight",
    "swatch",
    "collect",
    "note",
    "study",
    "copy",
    "copy-reference",
    "copy-passage",
    "cut",
    "paste",
    "bold",
    "italic",
    "link-passage",
    "remove-link",
    "close",
    "close-others",
    "duplicate",
    "move",
    "pin",
    "follow",
    "link",
    "focus",
    "move-left",
    "move-right",
    "new-tab",
    "equalize",
    "close-column",
    "paste-passage",
    "text-size",
    "text-larger",
    "text-smaller",
    "text-reset",
    "theme",
];

/// The highlight swatches of design/tokens/shell.json, in palette order.
pub const HIGHLIGHT_SWATCHES: [(&str, &str); 4] = [
    ("Yellow", "#F5D90A"),
    ("Green", "#4CC38A"),
    ("Blue", "#5B9BFF"),
    ("Pink", "#F25C9A"),
];

/// The platform keys of the host clipboard items; the registry binds none, so a host never
/// takes ⌘C away from the text field that has focus.
const CLIPBOARD_KEYS: [(&str, &str); 3] = [
    ("host.cut", "Cmd+X"),
    ("host.copy", "Cmd+C"),
    ("host.paste", "Cmd+V"),
];

/// Longest a word or selection is quoted in a label or title before it is cut with `…`.
const QUOTE_CHARS: usize = 24;

fn quoted(text: &str) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= QUOTE_CHARS {
        text
    } else {
        let cut: String = text.chars().take(QUOTE_CHARS - 1).collect();
        format!("{}…", cut.trim_end())
    }
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// Whether an edition in `language` can hold a passage of USFM book `book`: Greek in the New
/// Testament, Hebrew and Aramaic in the Old, any other language anywhere.
fn holds(language: &str, book: Option<u8>) -> bool {
    let Some(book) = book else {
        return true;
    };
    let primary = language
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match primary.as_str() {
        "grc" => (41..=67).contains(&book),
        "hbo" | "he" | "heb" | "arc" => book <= 39,
        _ => true,
    }
}

/// What the menu is built against: the session after the focus change, the stacks and the
/// dispatch context.
struct Builder<'a> {
    session: Cow<'a, Session>,
    can_undo: bool,
    can_redo: bool,
    context: CommandContext,
}

impl Builder<'_> {
    fn item(
        &self,
        command_id: &str,
        args: CommandArgs,
        label: &str,
        icon: &str,
    ) -> ContextMenuItem {
        let invocation = Invocation::with(command_id, args);
        let disabled = enabled(
            &self.session,
            self.can_undo,
            self.can_redo,
            &invocation,
            &self.context,
        )
        .err();
        let key = CLIPBOARD_KEYS
            .iter()
            .find(|(id, _)| *id == command_id)
            .map(|(_, key)| (*key).to_owned())
            .or_else(|| {
                command(command_id).and_then(|c| {
                    concrete_shortcut(
                        c.shortcuts.web.first().map(String::as_str),
                        &invocation.args,
                    )
                })
            });
        ContextMenuItem {
            command_id: invocation.command_id,
            args: invocation.args,
            label: label.to_owned(),
            key,
            icon: icon.to_owned(),
            active: false,
            value: None,
            disabled_text: disabled.as_ref().map(ToString::to_string),
            submenu: Vec::new(),
        }
    }

    /// The active workspace (after the focus change).
    fn ws(&self) -> &crate::shell::Workspace {
        self.session.active()
    }

    /// `Column 2 · BSB`, by the column's shown tab.
    fn column_label(&self, number: usize) -> String {
        let column = &self.ws().columns[number - 1];
        match column.shown() {
            Some(tab) => format!(
                "Column {number} · {}",
                self.session.resource_title(&tab.content_id)
            ),
            None => format!("Column {number}"),
        }
    }

    /// One child per column and a new column, for a command that takes a column number or
    /// `new_column`, on top of `base`'s arguments.
    fn column_children(&self, command_id: &str, base: &CommandArgs) -> Vec<ContextMenuItem> {
        let mut children: Vec<ContextMenuItem> = (1..=self.ws().columns.len())
            .map(|n| {
                let args = CommandArgs {
                    column: Some(n as u32),
                    ..base.clone()
                };
                self.item(command_id, args, &self.column_label(n), "column")
            })
            .collect();
        let args = CommandArgs {
            new_column: true,
            ..base.clone()
        };
        children.push(self.item(command_id, args, "New column", "new-column"));
        children
    }

    /// "Open John 3:16 in ▸": every column but the one the passage is read in and those
    /// showing a note, and a new column.
    fn open_in(&self, reference: &str, own: Option<usize>) -> ContextMenuItem {
        let base = CommandArgs {
            reference: Some(reference.to_owned()),
            ..CommandArgs::default()
        };
        let children = self
            .column_children("nav.navigate", &base)
            .into_iter()
            .filter(|c| {
                let shows_note = c.args.column.is_some_and(|n| {
                    self.ws().columns[n as usize - 1]
                        .shown()
                        .is_some_and(|t| t.kind == PaneKind::Notes)
                });
                !shows_note && own.is_none_or(|n| c.args.column != Some(n as u32))
            })
            .collect();
        let words = human_reference(reference).unwrap_or_else(|| reference.to_owned());
        submenu(&format!("Open {words} in"), "open", children)
    }

    /// "Compare in ▸": the other editions that can hold the passage.
    fn compare(&self, reference: Option<&str>, except: Option<&str>) -> ContextMenuItem {
        let book = reference
            .and_then(parse_tab_reference)
            .map(|set| set.ranges[0].start.book.number());
        let children: Vec<ContextMenuItem> = self
            .session
            .resource_languages
            .iter()
            .filter(|(id, language)| Some(id.as_str()) != except && holds(language, book))
            .map(|(id, _)| {
                let args = CommandArgs {
                    reference: reference.map(str::to_owned),
                    resource_id: Some(id.clone()),
                    ..CommandArgs::default()
                };
                let title = self.session.resource_title(id).to_owned();
                self.item("tab.open_edition", args, &title, "compare")
            })
            .collect();
        match children.len() {
            0 => {
                let mut item = self.item(
                    "tab.open_edition",
                    CommandArgs {
                        reference: reference.map(str::to_owned),
                        ..CommandArgs::default()
                    },
                    "Compare in another edition",
                    "compare",
                );
                item.disabled_text = Some("No other edition is installed".to_owned());
                item
            }
            1 => {
                let mut item = children.into_iter().next().expect("one");
                item.label = format!("Compare in {}", item.label);
                item
            }
            _ => submenu("Compare in", "compare", children),
        }
    }

    /// "Highlight ▸": the four swatches, and the current one first when it is not one of them.
    fn highlight(&self) -> ContextMenuItem {
        let current = self.session.ink.swatch.to_uppercase();
        let mut swatches: Vec<(String, String)> = Vec::new();
        if !HIGHLIGHT_SWATCHES.iter().any(|(_, hex)| *hex == current) {
            swatches.push(("Current swatch".to_owned(), current.clone()));
        }
        swatches.extend(
            HIGHLIGHT_SWATCHES
                .iter()
                .map(|(name, hex)| ((*name).to_owned(), (*hex).to_owned())),
        );
        let children = swatches
            .into_iter()
            .map(|(name, hex)| {
                let args = CommandArgs {
                    text: Some(hex.clone()),
                    ..CommandArgs::default()
                };
                let mut item = self.item("selection.highlight_in", args, &name, "swatch");
                item.active = hex == current;
                item.value = Some(hex);
                item
            })
            .collect();
        submenu("Highlight", "highlight", children)
    }

    /// Search in this edition and in all editions.
    fn searches(&self, text: &str, resource: Option<&str>, words: &str) -> Vec<ContextMenuItem> {
        let edition = CommandArgs {
            text: Some(text.to_owned()),
            scope: Some("edition".to_owned()),
            resource_id: resource.map(str::to_owned),
            ..CommandArgs::default()
        };
        let all = CommandArgs {
            text: Some(text.to_owned()),
            scope: Some("all".to_owned()),
            ..CommandArgs::default()
        };
        vec![
            self.item(
                "search.text",
                edition,
                &format!("Search {words} in this edition"),
                "search",
            ),
            self.item(
                "search.text",
                all,
                &format!("Search {words} in all editions"),
                "search-all",
            ),
        ]
    }
}

/// A row that opens `children`; it carries the first enabled child's command, and is
/// disabled only when every child is.
fn submenu(label: &str, icon: &str, children: Vec<ContextMenuItem>) -> ContextMenuItem {
    let first = children
        .iter()
        .find(|c| c.disabled_text.is_none())
        .or(children.first());
    let (command_id, args) = first.map_or_else(
        || (String::new(), CommandArgs::default()),
        |c| (c.command_id.clone(), c.args.clone()),
    );
    let disabled_text = if children.iter().all(|c| c.disabled_text.is_some()) {
        first.and_then(|c| c.disabled_text.clone())
    } else {
        None
    };
    ContextMenuItem {
        command_id,
        args,
        label: label.to_owned(),
        key: None,
        icon: icon.to_owned(),
        active: false,
        value: None,
        disabled_text,
        submenu: children,
    }
}

fn section(label: &str, items: Vec<ContextMenuItem>) -> ContextMenuSection {
    ContextMenuSection {
        label: Some(label.to_owned()),
        items,
    }
}

fn args_text(text: &str) -> CommandArgs {
    CommandArgs {
        text: Some(text.to_owned()),
        ..CommandArgs::default()
    }
}

fn args_reference(reference: &str) -> CommandArgs {
    CommandArgs {
        reference: Some(reference.to_owned()),
        ..CommandArgs::default()
    }
}

/// `John.3` for `John.3.16`; the reference itself when it is not `Book.C.V`.
fn chapter_of(reference: &str) -> String {
    let parts: Vec<&str> = reference.split('.').collect();
    if parts.len() == 3 && !reference.contains('-') {
        format!("{}.{}", parts[0], parts[1])
    } else {
        reference.to_owned()
    }
}

/// The focus change a click on the tab would run, `None` when it is already the active
/// column's shown tab.
fn focus_tab(session: &Session, tab_id: &str) -> Result<Option<Invocation>, DisabledReason> {
    let ws = session.active();
    let (index, _) = ws
        .find_tab(tab_id)
        .ok_or_else(|| DisabledReason::NoSuchTab {
            tab_id: tab_id.to_owned(),
        })?;
    if index == ws.active_index() && ws.columns[index].shown_tab_id == tab_id {
        return Ok(None);
    }
    Ok(Some(Invocation::with(
        "tab.focus",
        CommandArgs {
            tab_id: Some(tab_id.to_owned()),
            ..CommandArgs::default()
        },
    )))
}

/// The focus change a click in column `number` would run.
fn focus_column(session: &Session, number: u32) -> Result<Option<Invocation>, DisabledReason> {
    let ws = session.active();
    if number == 0 || number as usize > ws.columns.len() {
        return Err(DisabledReason::NoSuchColumn { number });
    }
    if number as usize == ws.active_index() + 1 {
        return Ok(None);
    }
    Ok(Some(Invocation::with(
        "column.focus",
        CommandArgs {
            column: Some(number),
            ..CommandArgs::default()
        },
    )))
}

/// The menu for `target` on `shell`, with what the host knows (`context.has_selection` for
/// the tab, column and background menus). Fails only for a tab or column that does not exist.
pub fn context_menu(
    shell: &Shell,
    target: &ContextTarget,
    context: &CommandContext,
) -> Result<ContextMenu, DisabledReason> {
    let session = &shell.session;
    let note_focus = |tab_id: &Option<String>| match tab_id {
        Some(_) => Focus::Tab,
        None => Focus::NotesDock,
    };
    let (focus, dispatch) = match target {
        ContextTarget::Word { tab_id, .. } | ContextTarget::Selection { tab_id, .. } => (
            tab_id
                .as_deref()
                .map(|t| focus_tab(session, t))
                .transpose()?
                .flatten(),
            CommandContext {
                has_selection: true,
                focus: Focus::Tab,
            },
        ),
        ContextTarget::NoteText { text, tab_id } => (
            tab_id
                .as_deref()
                .map(|t| focus_tab(session, t))
                .transpose()?
                .flatten(),
            CommandContext {
                has_selection: non_empty(text).is_some(),
                focus: note_focus(tab_id),
            },
        ),
        ContextTarget::NoteLink { tab_id, .. } => (
            tab_id
                .as_deref()
                .map(|t| focus_tab(session, t))
                .transpose()?
                .flatten(),
            CommandContext {
                has_selection: false,
                focus: note_focus(tab_id),
            },
        ),
        ContextTarget::Tab { tab_id } => (
            focus_tab(session, tab_id)?,
            CommandContext {
                has_selection: context.has_selection,
                focus: Focus::Tab,
            },
        ),
        ContextTarget::Column { column } => (
            focus_column(session, *column)?,
            CommandContext {
                has_selection: context.has_selection,
                focus: Focus::Tab,
            },
        ),
        ContextTarget::Background { column } => (
            column
                .map(|n| focus_column(session, n))
                .transpose()?
                .flatten(),
            CommandContext {
                has_selection: context.has_selection,
                focus: Focus::Tab,
            },
        ),
    };
    let session = match &focus {
        Some(invocation) => Cow::Owned(
            apply(session, invocation, &dispatch)
                .map_err(|_| DisabledReason::InvalidValue {
                    detail: "That target cannot take focus".to_owned(),
                })?
                .session,
        ),
        None => Cow::Borrowed(session),
    };
    let b = Builder {
        session,
        can_undo: !shell.undo_stack.is_empty(),
        can_redo: !shell.redo_stack.is_empty(),
        context: dispatch,
    };
    let (title, sections) = match target {
        ContextTarget::Word {
            text,
            reference,
            resource_id,
            ..
        } => word_menu(&b, text, non_empty(reference), non_empty(resource_id)),
        ContextTarget::Selection {
            text,
            reference,
            end_reference,
            resource_id,
            ..
        } => selection_menu(
            &b,
            text,
            non_empty(reference),
            non_empty(end_reference),
            non_empty(resource_id),
        ),
        ContextTarget::NoteText { text, .. } => note_text_menu(&b, non_empty(text)),
        ContextTarget::NoteLink { reference, .. } => note_link_menu(&b, reference),
        ContextTarget::Tab { .. } => tab_menu(&b),
        ContextTarget::Column { column } => column_menu(&b, *column),
        ContextTarget::Background { .. } => background_menu(&b),
    };
    Ok(ContextMenu {
        target_kind: target.kind(),
        title,
        focus,
        context: dispatch,
        sections,
    })
}

type Built = (Option<String>, Vec<ContextMenuSection>);

/// The reading tab a word or selection is in (after the focus change, the active tab), its
/// edition and column.
fn reading_tab<'a>(b: &'a Builder<'_>, resource: Option<&'a str>) -> (&'a Tab, &'a str, usize) {
    let ws = b.ws();
    let tab = ws.active_tab();
    (
        tab,
        resource.unwrap_or(tab.content_id.as_str()),
        ws.active_index() + 1,
    )
}

fn word_menu(
    b: &Builder<'_>,
    text: &str,
    reference: Option<&str>,
    resource: Option<&str>,
) -> Built {
    let (_, resource, own) = reading_tab(b, resource);
    let original = b
        .session
        .resource_language(resource)
        .is_some_and(crate::shell::is_original_language);
    let words = format!("“{}”", quoted(text));
    let mut search = b.searches(text, Some(resource), &words);
    search.push(b.item("search.lemma", args_text(text), "Search lemma", "lemma"));
    let mut study = b.item(
        "tab.open_lexicon",
        CommandArgs {
            text: Some(text.to_owned()),
            reference: reference.map(str::to_owned),
            ..CommandArgs::default()
        },
        "Word study",
        "word-study",
    );
    if !original && study.disabled_text.is_none() {
        study.disabled_text = Some("Word study works on Greek and Hebrew text".to_owned());
    }
    search.push(study);
    let mut find = b.item(
        "search.text",
        CommandArgs {
            text: Some(text.to_owned()),
            scope: Some("chapter".to_owned()),
            reference: reference.map(chapter_of),
            resource_id: Some(resource.to_owned()),
            ..CommandArgs::default()
        },
        "Find in this chapter",
        "find",
    );
    if reference.is_none() && find.disabled_text.is_none() {
        find.disabled_text = Some("Needs a passage".to_owned());
    }
    search.push(find);
    let mut sections = vec![section("Search", search)];
    if let Some(reference) = reference {
        sections.push(section(
            "Open",
            vec![
                b.compare(Some(reference), Some(resource)),
                b.open_in(reference, Some(own)),
                b.item(
                    "nav.cross_references",
                    args_reference(reference),
                    "Cross references",
                    "cross-references",
                ),
            ],
        ));
    }
    sections.push(section(
        "Mark",
        vec![
            b.highlight(),
            b.item(
                "selection.collect",
                CommandArgs::default(),
                "Collect",
                "collect",
            ),
            b.item(
                "selection.note",
                CommandArgs::default(),
                "Note on it",
                "note",
            ),
        ],
    ));
    let mut copy = vec![b.item("copy.text", args_text(text), "Copy word", "copy")];
    if let Some(reference) = reference {
        copy.push(b.item(
            "copy.reference",
            args_reference(reference),
            "Copy reference",
            "copy-reference",
        ));
        copy.push(b.item(
            "copy.passage",
            CommandArgs {
                reference: Some(reference.to_owned()),
                resource_id: Some(resource.to_owned()),
                ..CommandArgs::default()
            },
            "Copy verse with reference",
            "copy-passage",
        ));
    }
    let copy = if copy.len() == 1 {
        copy
    } else {
        vec![submenu("Copy", "copy", copy)]
    };
    sections.push(section("Copy", copy));
    (Some(quoted(text)), sections)
}

fn selection_menu(
    b: &Builder<'_>,
    text: &str,
    reference: Option<&str>,
    end_reference: Option<&str>,
    resource: Option<&str>,
) -> Built {
    let (_, resource, own) = reading_tab(b, resource);
    let range = match (reference, end_reference) {
        (Some(start), Some(end)) if start != end => Some(format!("{start}-{end}")),
        (start, _) => start.map(str::to_owned),
    };
    let mut sections = vec![section(
        "Search",
        b.searches(text, Some(resource), "selection"),
    )];
    if let Some(reference) = reference {
        sections.push(section(
            "Open",
            vec![
                b.compare(Some(reference), Some(resource)),
                b.open_in(reference, Some(own)),
            ],
        ));
    }
    sections.push(section(
        "Mark",
        vec![
            b.highlight(),
            b.item(
                "selection.collect",
                CommandArgs::default(),
                "Collect",
                "collect",
            ),
            b.item(
                "selection.note",
                CommandArgs::default(),
                "Note on it",
                "note",
            ),
            b.item("selection.study", CommandArgs::default(), "Study", "study"),
        ],
    ));
    let mut copy = vec![b.item("host.copy", CommandArgs::default(), "Copy", "copy")];
    if let Some(range) = range {
        copy.push(b.item(
            "copy.passage",
            CommandArgs {
                text: Some(text.to_owned()),
                reference: Some(range),
                resource_id: Some(resource.to_owned()),
                ..CommandArgs::default()
            },
            "Copy with reference",
            "copy-passage",
        ));
    }
    sections.push(section("Copy", copy));
    (Some(format!("“{}”", quoted(text))), sections)
}

fn note_text_menu(b: &Builder<'_>, text: Option<&str>) -> Built {
    let none = CommandArgs::default;
    let mut link = b.item("note.link_passage", none(), "Link passage", "link-passage");
    // The host asks for the passage, as from the tool well.
    if link.disabled_text.as_deref() == Some("Needs a passage") {
        link.disabled_text = None;
    }
    let search = match text {
        Some(text) => b.item(
            "search.text",
            CommandArgs {
                text: Some(text.to_owned()),
                scope: Some("all".to_owned()),
                ..CommandArgs::default()
            },
            "Search selection in all editions",
            "search-all",
        ),
        None => {
            let mut item = b.item(
                "search.text",
                CommandArgs {
                    scope: Some("all".to_owned()),
                    ..CommandArgs::default()
                },
                "Search selection in all editions",
                "search-all",
            );
            item.disabled_text = Some(DisabledReason::NeedsSelection.to_string());
            item
        }
    };
    let sections = vec![
        section(
            "Edit",
            vec![
                b.item("host.cut", none(), "Cut", "cut"),
                b.item("host.copy", none(), "Copy", "copy"),
                b.item("host.paste", none(), "Paste", "paste"),
            ],
        ),
        section(
            "Format",
            vec![
                b.item("note.bold", none(), "Bold", "bold"),
                b.item("note.italic", none(), "Italic", "italic"),
                link,
            ],
        ),
        section("Search", vec![search]),
    ];
    (text.map(|t| format!("“{}”", quoted(t))), sections)
}

fn note_link_menu(b: &Builder<'_>, reference: &str) -> Built {
    let sections = vec![
        section(
            "Open",
            vec![b.open_in(reference, None), b.compare(Some(reference), None)],
        ),
        section(
            "Link",
            vec![
                b.item(
                    "copy.reference",
                    args_reference(reference),
                    "Copy reference",
                    "copy-reference",
                ),
                b.item(
                    "note.remove_link",
                    args_reference(reference),
                    "Remove link",
                    "remove-link",
                ),
            ],
        ),
    ];
    (
        Some(human_reference(reference).unwrap_or_else(|| reference.to_owned())),
        sections,
    )
}

fn tab_menu(b: &Builder<'_>) -> Built {
    let ws = b.ws();
    let tab = ws.active_tab();
    let none = CommandArgs::default;
    let mut pin = b.item("tab.pin", none(), "Pin", "pin");
    let mut follow = b.item("tab.follow", none(), "Follow active passage", "follow");
    let mode = tab.link_mode();
    pin.active = mode == LinkMode::Pinned;
    follow.active = mode == LinkMode::Follows;
    // The groups of the workspace and the next free number.
    let mut groups: Vec<String> = ws.tabs().filter_map(|t| t.link_group.clone()).collect();
    groups.sort_by_key(|g| (g.parse::<u32>().unwrap_or(u32::MAX), g.clone()));
    groups.dedup();
    let next = (1..)
        .map(|n: u32| n.to_string())
        .find(|n| !groups.contains(n))
        .expect("a free number");
    groups.push(next);
    let children = groups
        .into_iter()
        .map(|group| {
            let args = CommandArgs {
                group: Some(group.clone()),
                ..CommandArgs::default()
            };
            let mut item = b.item("tab.link", args, &format!("Group {group}"), "link");
            // Its key links to group 1, the default.
            if group != "1" {
                item.key = None;
            }
            item.active = mode == LinkMode::Group { group };
            item
        })
        .collect();
    let sections = vec![
        section(
            "Tab",
            vec![
                b.item("tab.close", none(), "Close tab", "close"),
                b.item(
                    "tab.close_others",
                    none(),
                    "Close other tabs",
                    "close-others",
                ),
            ],
        ),
        section(
            "Move",
            vec![
                submenu(
                    "Duplicate to",
                    "duplicate",
                    b.column_children("tab.duplicate_to_column", &none()),
                ),
                submenu(
                    "Move to",
                    "move",
                    b.column_children("tab.move_to_column", &none()),
                ),
            ],
        ),
        section(
            "Link",
            vec![pin, follow, submenu("Link group", "link", children)],
        ),
    ];
    let title = match reference_labels(tab.reference.as_deref()).0 {
        Some(label) => format!("{} · {label}", b.session.resource_title(&tab.content_id)),
        None => b.session.resource_title(&tab.content_id).to_owned(),
    };
    (Some(title), sections)
}

fn column_menu(b: &Builder<'_>, number: u32) -> Built {
    let none = CommandArgs::default;
    let focus = CommandArgs {
        column: Some(number),
        ..CommandArgs::default()
    };
    let sections = vec![
        section(
            "Column",
            vec![
                b.item(
                    "column.focus",
                    focus,
                    &format!("Focus column {number}"),
                    "focus",
                ),
                b.item("tab.new", none(), "New tab", "new-tab"),
            ],
        ),
        section(
            "Move",
            vec![
                b.item("column.move_left", none(), "Move left", "move-left"),
                b.item("column.move_right", none(), "Move right", "move-right"),
            ],
        ),
        section(
            "Layout",
            vec![
                b.item("column.equalize", none(), "Equalize widths", "equalize"),
                b.item("column.close", none(), "Close column", "close-column"),
            ],
        ),
    ];
    (Some(format!("Column {number}")), sections)
}

fn background_menu(b: &Builder<'_>) -> Built {
    let none = CommandArgs::default;
    let sizes = vec![
        b.item("view.text_size_up", none(), "Larger", "text-larger"),
        b.item("view.text_size_down", none(), "Smaller", "text-smaller"),
        b.item("view.text_size_reset", none(), "Actual size", "text-reset"),
    ];
    let current = b.session.reading.theme.as_str();
    let themes = THEMES
        .iter()
        .map(|theme| {
            let mut label = theme.to_string();
            label[..1].make_ascii_uppercase();
            let mut item = b.item("view.theme", args_text(theme), &label, "theme");
            item.active = *theme == current;
            item
        })
        .collect();
    let new_tab_kind = b.ws().active_tab().kind;
    let mut paste = b.item(
        "nav.paste_passage",
        none(),
        "Paste passage",
        "paste-passage",
    );
    if new_tab_kind != PaneKind::Reading && paste.disabled_text.is_none() {
        paste.disabled_text = Some("Paste a passage into a reading tab".to_owned());
    }
    let sections = vec![
        section(
            "Tab",
            vec![b.item("tab.new", none(), "New tab", "new-tab"), paste],
        ),
        section(
            "View",
            vec![
                submenu("Text size", "text-size", sizes),
                submenu("Theme", "theme", themes),
            ],
        ),
    ];
    (None, sections)
}

impl Shell {
    /// The context menu for what was right-clicked (specs/22 §4.2).
    pub fn context_menu(
        &self,
        target: &ContextTarget,
        context: &CommandContext,
    ) -> Result<ContextMenu, DisabledReason> {
        context_menu(self, target, context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::tests::fixture_session;
    use crate::commands::{CommandError, registry};

    fn shell() -> Shell {
        let mut shell = Shell::new(fixture_session()).unwrap();
        shell.set_resource_languages(
            [
                ("bsb".to_owned(), "en".to_owned()),
                ("sblgnt".to_owned(), "grc".to_owned()),
                ("wlc".to_owned(), "hbo".to_owned()),
            ]
            .into(),
        );
        shell.set_resource_titles(
            [
                ("bsb".to_owned(), "BSB".to_owned()),
                ("sblgnt".to_owned(), "SBLGNT".to_owned()),
            ]
            .into(),
        );
        shell
    }

    fn target(json: &str) -> ContextTarget {
        serde_json::from_str(json).unwrap()
    }

    fn find<'a>(menu: &'a ContextMenu, label: &str) -> &'a ContextMenuItem {
        menu.sections
            .iter()
            .flat_map(|s| s.items.iter())
            .find(|i| i.label == label)
            .unwrap_or_else(|| panic!("no row {label}"))
    }

    const TARGETS: &[&str] = &[
        r#"{"kind":"word","text":"loved","reference":"John.3.16"}"#,
        r#"{"kind":"word","text":"ἠγάπησεν","reference":"John.3.16","tab_id":"t3"}"#,
        r#"{"kind":"word","text":"Selah"}"#,
        r#"{"kind":"selection","text":"so loved","reference":"John.3.16","end_reference":"John.3.17"}"#,
        r#"{"kind":"note_text"}"#,
        r#"{"kind":"note_text","text":"grace"}"#,
        r#"{"kind":"note_link","reference":"Rom.8.28"}"#,
        r#"{"kind":"tab","tab_id":"t2"}"#,
        r#"{"kind":"column","column":2}"#,
        r#"{"kind":"background"}"#,
    ];

    #[test]
    fn every_row_is_a_registered_command_with_a_known_icon_and_words() {
        let shell = shell();
        let mut kinds = Vec::new();
        for json in TARGETS {
            let menu = shell
                .context_menu(&target(json), &CommandContext::default())
                .unwrap();
            kinds.push(menu.target_kind);
            let rows: usize = menu.sections.iter().map(|s| s.items.len()).sum();
            assert!(rows <= 12, "{json}: {rows} rows");
            for item in menu.sections.iter().flat_map(|s| s.items.iter()) {
                for row in std::iter::once(item).chain(item.submenu.iter()) {
                    assert!(command(&row.command_id).is_some(), "{}", row.command_id);
                    assert!(MENU_ICONS.contains(&row.icon.as_str()), "{}", row.icon);
                    assert!(!row.label.is_empty());
                    assert!(
                        row.disabled_text
                            .as_deref()
                            .is_none_or(|t| !t.contains('_')),
                        "{row:?}"
                    );
                }
            }
        }
        for kind in ContextTargetKind::ALL {
            assert!(kinds.contains(&kind), "{}", kind.name());
        }
        // Every command the menu may name is in the registry, appended after B076's.
        let ids: Vec<&str> = registry().iter().map(|c| c.id.as_str()).collect();
        let first = ids.iter().position(|id| *id == "search.text").unwrap();
        // B077's last, then B074's drag.
        let last = ids.iter().position(|id| *id == "note.remove_link").unwrap();
        assert_eq!(&ids[last + 1..], ["tab.move"]);
        assert_eq!(ids[first - 1], "tab.open_lexicon");
    }

    #[test]
    fn a_word_menu_follows_the_language_and_fills_the_passage() {
        let shell = shell();
        let context = CommandContext::default();
        let english = shell.context_menu(&target(TARGETS[0]), &context).unwrap();
        assert_eq!(english.focus, None);
        assert_eq!(
            english.context,
            CommandContext {
                has_selection: true,
                focus: Focus::Tab
            }
        );
        assert_eq!(
            find(&english, "Word study").disabled_text.as_deref(),
            Some("Word study works on Greek and Hebrew text")
        );
        assert_eq!(
            find(&english, "Search lemma").disabled_text.as_deref(),
            Some("Needs the lexicon import")
        );
        let compare = find(&english, "Compare in SBLGNT");
        assert_eq!(compare.args.resource_id.as_deref(), Some("sblgnt"));
        assert_eq!(
            find(&english, "Find in this chapter")
                .args
                .reference
                .as_deref(),
            Some("John.3")
        );
        let open = find(&english, "Open John 3:16 in");
        assert_eq!(
            open.submenu
                .iter()
                .map(|c| c.label.as_str())
                .collect::<Vec<_>>(),
            [
                "Column 2 · SBLGNT",
                "Column 3 · fixture:lexicon-1",
                "New column"
            ]
        );
        let greek = shell.context_menu(&target(TARGETS[1]), &context).unwrap();
        assert_eq!(
            greek.focus.as_ref().map(|f| f.args.tab_id.as_deref()),
            Some(Some("t3"))
        );
        let study = find(&greek, "Word study");
        assert_eq!(study.disabled_text, None);
        assert_eq!(study.command_id, "tab.open_lexicon");
        assert_eq!(study.args.reference.as_deref(), Some("John.3.16"));
        // WLC cannot hold John; BSB is the only other edition.
        assert_eq!(find(&greek, "Compare in BSB").submenu.len(), 0);
        let bare = shell.context_menu(&target(TARGETS[2]), &context).unwrap();
        assert!(
            bare.sections
                .iter()
                .all(|s| s.label.as_deref() != Some("Open"))
        );
        assert_eq!(
            find(&bare, "Find in this chapter").disabled_text.as_deref(),
            Some("Needs a passage")
        );
    }

    #[test]
    fn the_focus_runs_first_and_the_items_act_on_the_target() {
        let mut shell = shell();
        let menu = shell
            .context_menu(&target(TARGETS[7]), &CommandContext::default())
            .unwrap();
        assert_eq!(menu.title.as_deref(), Some("BSB · Rom 8:28"));
        let pin = find(&menu, "Pin");
        assert!(pin.active);
        assert_eq!(
            pin.disabled_text.as_deref(),
            Some("The tab is already pinned")
        );
        let close = find(&menu, "Close tab").clone();
        shell
            .apply(menu.focus.as_ref().unwrap(), &menu.context)
            .unwrap();
        shell
            .apply(
                &Invocation::with(&close.command_id, close.args),
                &menu.context,
            )
            .unwrap();
        assert!(shell.session.active().find_tab("t2").is_none());
        assert!(shell.session.active().find_tab("t1").is_some());

        let notes = shell
            .context_menu(&target(TARGETS[5]), &CommandContext::default())
            .unwrap();
        assert_eq!(notes.context.focus, Focus::NotesDock);
        assert_eq!(find(&notes, "Bold").disabled_text, None);
        assert_eq!(find(&notes, "Copy").key.as_deref(), Some("Cmd+C"));
        let plain = shell
            .context_menu(&target(TARGETS[4]), &CommandContext::default())
            .unwrap();
        assert_eq!(
            find(&plain, "Cut").disabled_text.as_deref(),
            Some("Select some text first")
        );
        assert_eq!(find(&plain, "Link passage").disabled_text, None);
    }

    #[test]
    fn a_target_that_does_not_exist_is_refused() {
        let shell = shell();
        let context = CommandContext::default();
        assert_eq!(
            shell.context_menu(&target(r#"{"kind":"tab","tab_id":"t9"}"#), &context),
            Err(DisabledReason::NoSuchTab {
                tab_id: "t9".to_owned()
            })
        );
        assert_eq!(
            shell.context_menu(&target(r#"{"kind":"column","column":7}"#), &context),
            Err(DisabledReason::NoSuchColumn { number: 7 })
        );
    }

    #[test]
    fn compare_opens_beside_the_source_and_close_others_reopens() {
        let mut shell = shell();
        let context = CommandContext::default();
        let compare = |resource: &str, new_column: bool| {
            Invocation::with(
                "tab.open_edition",
                CommandArgs {
                    resource_id: Some(resource.to_owned()),
                    new_column,
                    ..CommandArgs::default()
                },
            )
        };
        // Column 2 already shows SBLGNT: it is reused, and its group stays where it was.
        let before = shell.session.clone();
        shell.apply(&compare("sblgnt", false), &context).unwrap();
        let ws = shell.session.active();
        assert_eq!(ws.columns[1].tabs.len(), 1);
        assert_eq!(ws.active_index(), 0, "focus stays on the source");
        // A new column after the source, with the source's link state.
        shell.apply(&compare("wlc", true), &context).unwrap();
        let ws = shell.session.active();
        assert_eq!(ws.columns.len(), 4);
        let opened = &ws.columns[1].tabs[0];
        assert_eq!(
            (opened.content_id.as_str(), opened.reference.as_deref()),
            ("wlc", Some("John.3.16"))
        );
        assert_eq!(opened.link_group.as_deref(), Some("1"));
        assert_eq!(ws.active_index(), 0);
        shell.undo().unwrap();
        shell.undo().unwrap();
        assert_eq!(shell.session, before);
        assert!(matches!(
            shell.apply(&Invocation::new("tab.open_edition"), &context),
            Err(CommandError::Disabled {
                reason: DisabledReason::NeedsArgument { .. },
                ..
            })
        ));

        shell
            .apply(&Invocation::new("tab.close_others"), &context)
            .unwrap();
        let column = &shell.session.active().columns[0];
        assert_eq!(column.tabs.len(), 1);
        assert_eq!(column.shown_tab_id, "t1");
        assert_eq!(
            shell
                .enabled(&Invocation::new("tab.close_others"), &context)
                .unwrap_err()
                .to_string(),
            "This column has one tab"
        );
        shell
            .apply(&Invocation::new("tab.reopen_closed"), &context)
            .unwrap();
        assert_eq!(shell.session.active().columns[0].tabs.len(), 2);
    }
}
