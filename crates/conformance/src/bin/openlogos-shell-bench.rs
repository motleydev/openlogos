//! The B070 shell benchmark (`just bench-shell`): core-side timings for the specs/22 §7
//! budgets. Plain `std` timing, no criterion; prints a Markdown table the evidence file
//! quotes verbatim and, with `--json`, writes the same numbers.
//!
//! ```text
//! openlogos-shell-bench [--rounds N] [--json out.json]   (N defaults to 100)
//! ```
//!
//! The session is ten workspaces of three columns of four tabs (specs/22 §7), with resources
//! and passages spread over the fixture editions and twenty books. Palette timings replay
//! every prefix of a set of typed queries, as a host calls `paletteQuery` on each keystroke,
//! against an index kept across keystrokes (the handles key it on the shell revision); the
//! index build after a command is timed on its own. The text section uses the conformance
//! fixture provider, so its row measures the palette's overhead around a provider, not the
//! B071 search. The view row is `Shell::view`, what both bindings return from `session()` /
//! `view()` on every render, with every tab's reference and chapter labels. Command timings run `Shell::apply` (undo record included) and exclude any
//! host rendering: they are the core's share of the frame. The tool well row (B076) is
//! `Shell::tool_well` after each column focus, alternating the tab and the notes dock with a
//! selection, so every group is built; it rides inside the column-focus budget. The context
//! menu row (B077) is `Shell::context_menu` after each column focus for every target kind:
//! a word and a selection in the active tab, a note spot and a passage link in the dock, a
//! hidden tab in the next column (the menu works on a copy of the session with that tab
//! focused), a column and the background; budget: shown on the next frame.
//!
//! B078 adds continuous reading: `locate` over verse ranges, chapters and books;
//! `chapter_content` from the placeholder pack (Psalm 119, the longest chapter, among them) and
//! from a file pack on disk (every chapter of John and Psalm 119 written in the B078 format
//! under the `--json` directory, verses padded to the length of real verses with 25 tokens
//! each, so each read is a file read, a SHA-256 and a JSON parse); `chapter_neighbors`; and
//! `tab.record_scroll` moving the active tab verse by verse with its group and a follower
//! mapped from `eng` to `org`. Budget: a chapter opens in ≤ 100 ms end to end (specs/22 §7);
//! the core's share should stay far below 5 ms.

use std::time::{Duration, Instant};

use openlogos_conformance::FixtureTextProvider;
use openlogos_workspaces::PaneKind;
use openlogos_workspaces::commands::{CommandArgs, CommandContext, Focus, Invocation, Shell};
use openlogos_workspaces::menu::ContextTarget;
use openlogos_workspaces::palette::{
    PaletteContext, PaletteIndex, TextProvider, palette_query_indexed,
};
use openlogos_workspaces::shell::{Column, NotesDock, Session, Tab, Tool, Workspace};
use serde::Serialize;

const BOOKS: [&str; 20] = [
    "Gen", "Exod", "Ps", "Prov", "Isa", "Jer", "Matt", "Mark", "Luke", "John", "Acts", "Rom",
    "1Cor", "Gal", "Eph", "Phil", "Heb", "Jas", "1John", "Rev",
];
const EDITIONS: [&str; 4] = ["bsb", "sblgnt", "wlc", "fixture:lexicon-1"];

/// Ten workspaces × three columns × four tabs; tabs alternate group 1, follow and pinned.
fn session() -> Session {
    let mut workspaces = Vec::new();
    for w in 0..10 {
        let mut columns = Vec::new();
        for c in 0..3 {
            let mut tabs = Vec::new();
            for (t, edition) in EDITIONS.iter().enumerate() {
                let n = w * 12 + c * 4 + t;
                let (group, pinned) = match t % 3 {
                    0 => (Some("1".to_owned()), false),
                    1 => (None, false),
                    _ => (None, true),
                };
                tabs.push(Tab {
                    id: format!("t{n}"),
                    kind: if t == 3 {
                        PaneKind::Lexicon
                    } else {
                        PaneKind::Reading
                    },
                    content_id: (*edition).to_owned(),
                    content_revision: "0".to_owned(),
                    reference: Some(format!("{}.{}.{}", BOOKS[n % 20], 1 + n % 7, 1 + n % 11)),
                    scroll_anchor: None,
                    link_group: group,
                    pinned,
                    history: Default::default(),
                    lexicon_word: None,
                    highlight_range: None,
                });
            }
            columns.push(Column {
                id: format!("c{c}"),
                shown_tab_id: tabs[0].id.clone(),
                tabs,
                width_fraction: 1.0 / 3.0,
            });
        }
        workspaces.push(Workspace {
            schema_version: "2.0".to_owned(),
            fixture_only: true,
            id: format!("w{w}"),
            title: format!(
                "Study {}",
                ["of John", "Romans", "Psalms", "Genesis", "Hebrews"][w % 5]
            ),
            active_column_id: "c0".to_owned(),
            columns,
            notes_dock: NotesDock::default(),
            layout_profiles: Default::default(),
            next_serial: 1000,
            closed_tabs: Vec::new(),
        });
    }
    let session = Session {
        schema_version: "1.0".to_owned(),
        fixture_only: true,
        active_workspace_id: "w0".to_owned(),
        workspaces,
        tool: Tool::Select,
        next_serial: 100,
        reading: Default::default(),
        ink: Default::default(),
        resource_titles: Default::default(),
        resource_languages: Default::default(),
        resource_schemes: Default::default(),
    };
    session.check().expect("the bench session is valid");
    session
}

/// Queries typed one character at a time, by the section they exercise.
const PASSAGE_COMMANDS_TABS: [&str; 10] = [
    "Jn 3:16",
    "Rom 8:28-30 @2",
    ":Gen 1:1 @new",
    ">new tab",
    ">move tab to 3",
    ">rename workspace Greek study",
    "@romans",
    "@sblgnt",
    "focus column 2",
    "close",
];
const TEXT: [&str; 4] = ["#loved the world", "#ברא", "world", "#God so"];

fn prefixes(query: &str) -> Vec<String> {
    let chars: Vec<char> = query.chars().collect();
    (1..=chars.len())
        .map(|n| chars[..n].iter().collect())
        .collect()
}

#[derive(Serialize)]
struct Row {
    interaction: String,
    budget_ms: f64,
    samples: usize,
    p50_us: f64,
    p95_us: f64,
    max_us: f64,
}

fn row(interaction: &str, budget_ms: f64, samples: &mut [Duration]) -> Row {
    samples.sort();
    let at = |q: f64| samples[((samples.len() as f64 - 1.0) * q).round() as usize];
    let us = |d: Duration| (d.as_secs_f64() * 1e7).round() / 10.0;
    Row {
        interaction: interaction.to_owned(),
        budget_ms,
        samples: samples.len(),
        p50_us: us(at(0.5)),
        p95_us: us(at(0.95)),
        max_us: us(*samples.last().expect("samples")),
    }
}

fn time<F: FnMut()>(samples: &mut Vec<Duration>, mut f: F) {
    let start = Instant::now();
    f();
    samples.push(start.elapsed());
}

/// B078 rows: locate, chapter content (placeholder and file pack), neighbours, and the
/// scroll-follow row.
fn reading_bench(rounds: usize, json: Option<&str>) -> (Vec<Row>, Row) {
    use openlogos_core::reference::BookId;
    use openlogos_text::reading::{
        ChapterVerse, DirSource, FilePack, PackWriter, TextPack, VerseToken,
    };

    let library = openlogos_conformance::reading_library();
    let inputs = [
        "John 3:16-18",
        "Jn 3:36–4:2",
        "Ps 119",
        "Genesis",
        "Joel 2:28",
        "Rev 22:21",
        "1 Cor 13:4-7",
        "Mal 4",
    ];
    let chapters = [
        "Ps.119", "Gen.1", "John.3", "Joel.2", "Mal.4", "Matt.1", "Rev.22",
    ];
    let mut locate = Vec::new();
    let mut placeholder = Vec::new();
    let mut neighbors = Vec::new();
    for _ in 0..rounds {
        for input in inputs {
            time(&mut locate, || {
                std::hint::black_box(library.locate("bsb", input).expect("locate"));
            });
        }
        for key in chapters {
            time(&mut placeholder, || {
                std::hint::black_box(library.chapter_content("bsb", key).expect("chapter"));
            });
            time(&mut neighbors, || {
                std::hint::black_box(library.chapter_neighbors("bsb", key).expect("neighbors"));
            });
        }
    }

    // A file pack on disk with real-sized verses: John 1–21 and Psalm 119.
    let dir = std::path::Path::new(json.unwrap_or("target/bench-shell/b070.json"))
        .parent()
        .map_or_else(
            || std::path::PathBuf::from("."),
            std::path::Path::to_path_buf,
        )
        .join("b078-pack");
    let bsb = library.editions().into_iter().next().expect("bsb");
    let mut writer = PackWriter::new(&bsb, "synthetic", "openlogos-shell-bench", true);
    let mut keys = Vec::new();
    for (book, count) in [(BookId::John, 21), (BookId::Ps, 0)] {
        let range: Vec<u32> = if count == 0 {
            vec![119]
        } else {
            (1..=count).collect()
        };
        for chapter in range {
            let key = format!("{}.{chapter}", book.as_osis());
            let block = library.chapter_content("bsb", &key).expect("chapter");
            let verses: Vec<ChapterVerse> = block
                .verses
                .into_iter()
                .map(|mut v| {
                    v.text = format!("{} {}", v.text, "lorem ipsum ".repeat(10));
                    v.placeholder = false;
                    v.tokens = (0..25)
                        .map(|i| VerseToken {
                            id: format!(
                                "bsb:{:02}{:03}{:03}{:03}",
                                book.number(),
                                chapter,
                                v.number,
                                i + 1
                            ),
                            start: i * 5,
                            end: i * 5 + 4,
                        })
                        .collect();
                    v
                })
                .collect();
            writer.chapter(
                book,
                openlogos_text::reading::book_title(book),
                chapter,
                verses,
            );
            keys.push(key);
        }
    }
    let written = writer.finish().expect("pack");
    for (path, bytes) in &written.files {
        let full = dir.join(path);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
        std::fs::write(full, bytes).expect("write chunk");
    }
    let pack =
        FilePack::from_manifest(&written.manifest_json, Some(Box::new(DirSource::new(&dir))))
            .expect("pack reads");
    let id = pack.edition().resource_id.clone();
    let mut packs = openlogos_text::reading::ReadingLibrary::new();
    packs.insert(Box::new(pack));
    let mut from_file = Vec::new();
    for _ in 0..rounds.div_ceil(10).max(5) {
        for key in &keys {
            time(&mut from_file, || {
                std::hint::black_box(packs.chapter_content(&id, key).expect("pack chapter"));
            });
        }
    }

    // The active tab scrolls verse by verse through John 3–4; its group (an `org` edition) and
    // a follow-mode tab move along.
    let mut session = openlogos_conformance::fixture_session();
    session.resource_schemes = openlogos_conformance::fixture_resource_schemes();
    let mut shell = Shell::new(session).expect("fixture session");
    let context = CommandContext::default();
    let mut scroll = Vec::new();
    for round in 0..rounds {
        for verse in 1..=20 {
            let go = Invocation::with(
                "tab.record_scroll",
                CommandArgs {
                    tab_id: Some("t1".to_owned()),
                    segment_id: Some(format!("John.{}.{verse}", 3 + round % 2)),
                    scalar: Some(0),
                    ..CommandArgs::default()
                },
            );
            time(&mut scroll, || {
                shell.apply(&go, &context).expect("record_scroll");
            });
        }
    }

    (
        vec![
            row(
                "locate a typed passage (ranges, chapters, books)",
                5.0,
                &mut locate,
            ),
            row(
                "chapter content, placeholder pack (Ps 119 among them)",
                5.0,
                &mut placeholder,
            ),
            row(
                "chapter content, file pack on disk (read, SHA-256, parse)",
                5.0,
                &mut from_file,
            ),
            row("chapter neighbours", 5.0, &mut neighbors),
        ],
        row(
            "scroll into the next verse: tab, group and follower move (tab.record_scroll)",
            16.0,
            &mut scroll,
        ),
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let rounds: usize = flag("--rounds").and_then(|r| r.parse().ok()).unwrap_or(100);
    let context = PaletteContext::default();
    let command_context = CommandContext::default();
    let provider = FixtureTextProvider::new();
    let mut shell = Shell::new(session()).expect("valid");
    let tabs: usize = shell
        .session
        .workspaces
        .iter()
        .map(|w| w.tabs().count())
        .sum();

    let mut palette = Vec::new();
    let mut text = Vec::new();
    let mut index_build = Vec::new();
    let mut tab_switch = Vec::new();
    let mut column_focus = Vec::new();
    let mut workspace_switch = Vec::new();
    let mut tool_change = Vec::new();
    let mut navigate = Vec::new();
    let mut move_tab = Vec::new();
    let mut undo = Vec::new();
    let mut view = Vec::new();
    let mut tool_well = Vec::new();
    let mut context_menu = Vec::new();
    shell.set_resource_languages(
        [("sblgnt", "grc"), ("wlc", "hbo"), ("bsb", "en")]
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .into(),
    );
    let dock = CommandContext {
        has_selection: true,
        focus: Focus::NotesDock,
    };

    for round in 0..rounds {
        let mut index = PaletteIndex::new(&shell.session);
        for _ in 0..2 {
            time(&mut view, || {
                std::hint::black_box(shell.view());
            });
        }
        for _ in 0..2 {
            time(&mut index_build, || {
                index = PaletteIndex::new(&shell.session)
            });
        }
        for query in PASSAGE_COMMANDS_TABS {
            for prefix in prefixes(query) {
                time(&mut palette, || {
                    std::hint::black_box(palette_query_indexed(
                        &shell, &index, &prefix, &context, None,
                    ));
                });
            }
        }
        let provider: &dyn TextProvider = &provider;
        for query in TEXT {
            for prefix in prefixes(query) {
                time(&mut text, || {
                    std::hint::black_box(palette_query_indexed(
                        &shell,
                        &index,
                        &prefix,
                        &context,
                        Some(provider),
                    ));
                });
            }
        }
        for _ in 0..12 {
            time(&mut tab_switch, || {
                shell
                    .apply(&Invocation::new("tab.next"), &command_context)
                    .expect("tab.next");
            });
        }
        for n in 1..=3u32 {
            let focus = Invocation::with(
                "column.focus",
                CommandArgs {
                    column: Some(n),
                    ..CommandArgs::default()
                },
            );
            time(&mut column_focus, || {
                shell.apply(&focus, &command_context).expect("column.focus");
            });
            for context in [&command_context, &dock] {
                time(&mut tool_well, || {
                    std::hint::black_box(shell.tool_well(context));
                });
            }
            let ws = shell.session.active();
            let hidden = ws.columns[n as usize % 3].tabs[1].id.clone();
            let reference = ws.active_tab().reference.clone();
            let targets = [
                ContextTarget::Word {
                    text: "loved".to_owned(),
                    reference: reference.clone(),
                    segment_id: reference.clone(),
                    scalar_start: Some(11),
                    scalar_end: Some(16),
                    resource_id: None,
                    tab_id: None,
                },
                ContextTarget::Selection {
                    text: "For God so loved the world".to_owned(),
                    reference: reference.clone(),
                    end_reference: None,
                    resource_id: None,
                    tab_id: None,
                },
                ContextTarget::NoteText {
                    text: None,
                    tab_id: None,
                },
                ContextTarget::NoteLink {
                    reference: "Rom.8.28".to_owned(),
                    tab_id: None,
                },
                ContextTarget::Tab { tab_id: hidden },
                ContextTarget::Column { column: 2 },
                ContextTarget::Background { column: None },
            ];
            for target in &targets {
                time(&mut context_menu, || {
                    std::hint::black_box(
                        shell
                            .context_menu(target, &command_context)
                            .expect("context menu"),
                    );
                });
            }
        }
        for _ in 0..10 {
            time(&mut workspace_switch, || {
                shell
                    .apply(&Invocation::new("workspace.next"), &command_context)
                    .expect("workspace.next");
            });
        }
        for tool in ["tool.pen", "tool.select", "tool.eraser"] {
            time(&mut tool_change, || {
                shell
                    .apply(&Invocation::new(tool), &command_context)
                    .expect("tool");
            });
        }
        for step in 0..2 {
            let go = Invocation::with(
                "nav.navigate",
                CommandArgs {
                    reference: Some(format!("John.3.{}", 1 + (2 * round + step) % 30)),
                    ..CommandArgs::default()
                },
            );
            time(&mut navigate, || {
                shell.apply(&go, &command_context).expect("navigate");
            });
            let send = Invocation::with(
                "tab.move_to_column",
                CommandArgs {
                    column: Some(2),
                    ..CommandArgs::default()
                },
            );
            time(&mut move_tab, || {
                shell.apply(&send, &command_context).expect("move");
            });
            time(&mut undo, || {
                shell.undo().expect("undo");
            });
        }
    }

    let (reading_rows, scroll) = reading_bench(rounds, flag("--json").as_deref());
    let mut rows = vec![
        row(
            "palette keystroke: passage, commands, tabs (index kept)",
            16.0,
            &mut palette,
        ),
        row(
            "palette keystroke with the text section (fixture provider)",
            50.0,
            &mut text,
        ),
        row(
            "palette index build after a command (120 tabs)",
            16.0,
            &mut index_build,
        ),
        row(
            "tab switch within a column (tab.next)",
            16.0,
            &mut tab_switch,
        ),
        row("column focus (column.focus N)", 16.0, &mut column_focus),
        row(
            "tool well for the focused tab or dock (Shell::tool_well)",
            16.0,
            &mut tool_well,
        ),
        row(
            "context menu for every target kind (Shell::context_menu)",
            16.0,
            &mut context_menu,
        ),
        row(
            "workspace switch (workspace.next, 10 × 3 × 4)",
            100.0,
            &mut workspace_switch,
        ),
        row("tool change (tool.*)", 16.0, &mut tool_change),
        row(
            "go to passage, group and follow tabs move (nav.navigate)",
            100.0,
            &mut navigate,
        ),
        row(
            "move tab to column 2, with undo record",
            16.0,
            &mut move_tab,
        ),
        row("undo", 16.0, &mut undo),
        row(
            "session view with tab labels (Shell::view, 120 tabs)",
            1.0,
            &mut view,
        ),
    ];
    rows.extend(reading_rows);
    rows.push(scroll);

    println!(
        "Session: {} workspaces, {tabs} tabs; {rounds} rounds.\n",
        shell.session.workspaces.len()
    );
    println!(
        "| interaction | specs/22 §7 budget | samples | p50 µs | p95 µs | max µs |\n|---|---:|---:|---:|---:|---:|"
    );
    for r in &rows {
        println!(
            "| {} | {} ms | {} | {} | {} | {} |",
            r.interaction, r.budget_ms, r.samples, r.p50_us, r.p95_us, r.max_us
        );
    }
    if let Some(path) = flag("--json") {
        let json = serde_json::to_string_pretty(&rows).expect("rows serialize");
        std::fs::write(&path, json + "\n").expect("write --json");
        println!("\nWrote {path}");
    }
}
