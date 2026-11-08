# B071 evidence (Mac): the real editions in the Mac app

Recorded 6–7 November 2026 on the Mac mini (Apple M1, macOS 26.4, Xcode 26.6) by a Claude Code
subagent, in the worktree `openlogos-b071mac`, branch `b071-mac` (core `b071-preview-pack` plus
`b074-mac-store`, PR #38). Nothing was pushed or merged. Measured and pictured at commit eb6bc7f (the code is unchanged after it).
What the packs hold and every text decision: B071.md. No text decision was changed. No corpus
byte is in the repository or the app bundle, though the pictures here show verses on screen.

## What works

| Item | How | Test |
|---|---|---|
| Real editions | `ReadingHandle.open_pack` for each `packs/<id>`, over the placeholder library; no format change. Location: `-OPENLOGOS_PREVIEW_PACK <dir>` (`none`: placeholder), else `OPENLOGOS_PREVIEW_PACK`, else `~/.cache/openlogos/preview-pack` (`Sources/Shell/PreviewPack.swift`). Opening all three takes 4.6 ms p95 at launch | `testPacksReplaceThePlaceholder…`, `testTheThreeEditionsInstall…` |
| No pack | The app runs on the placeholder library. Each chapter reads **"Placeholder text — this edition is not installed"**, the web's words. An edition whose pack is missing or refused stays a placeholder and is logged. A changed chunk is refused, not shown | `testNoPackAtAll…`, `testAPackThatDoesNotOpen…`, `testAChangedChunkIsRefused…` |
| Text search | `use_edition_search` on a second `ShellHandle`, off the main thread, because the session handle has one lock. Passages, commands and tabs paint in the keystroke. Text rows follow; the previous keystroke's rows stay until then and go at once for `>` and `@`. Enter waits for the current answer | `testTextRowsArrive…`, `testEnterActs…`, `testTheRowsOfTheLastKeystroke…`, `testCloseCancels…` |
| A hit opens the verse | The core's rule (lead's ruling, cherry-picked from `b071-web`): the row carries `resource_id`. A tab already showing that edition moves; otherwise a new tab of it opens and the shown tab is left alone | `testAHitFromAnotherEdition…`, `testAWLCHitOnABSBTab…`, `testASearchHitOpens…` |
| Swift test the core evidence asked for | `CoreHostTests` calls `use_edition_search` over `fixtures/search` (made-up sentences; `crates/preview-pack/tests/search_fixture.rs` fails when they go stale) | `testUseEditionSearch…`, `testADirectoryWithoutSearchPacks…` |
| Attribution | `view.about_edition` (the core's command, the web's panel): first row of a column's … menu, and the palette (`>licence`). Shows name, revision and `Edition.attribution` word for word, addresses followable. Nothing is added under chapters. Disabled by the core off a reading tab. An edition with no pack shows the placeholder words | `testAttributionComes…`, `testAboutThisEdition…` |
| Search packs that do not open | "Text search could not load." under the Text heading, the web's words (lead's ruling), when the editions are installed but `search/` is missing or refused and the field asks for text (`#`, or two characters). No loading line: the packs open within the launch's 4.6 ms. A Text answer still on its way no longer shows "No results" | `testSearchPacksThatDoNotOpen…`, `testNothingIsAnAnswer…` |
| Sticky header order | "BSB · Jn 3", the tab label's order and the web's (lead's lockstep fix, its own commit 2376fe2; B078-mac's pictures show the old "Jn 3 · BSB") | `ReadingTests` |
| Absent verses | A follower lands on the last verse numbered at or below the mapped one: BSB Mk 9:44 → SBLGNT 9:43; 9:46 → 9:45 | `testAFollowerLandsBeside…` |
| Past the chapter's end | BSB Rom 16:25 → the linked SBLGNT stays on Rom 16 at 16:24. Only a passage typed in that tab gets the not-in-edition state. Also true after a restore | `testAFollowerPast…`, `testARestoredFollower…`, `testFollowersOnTheEditions…` |
| WLC | `tanakh_bhs`: Mal 3 → Ps 1, ends at 2 Chr 36; right to left | `testTheWLCReads…` |
| New revisions | A chapter, its anchors and new highlights name the pack's `content_revision` | `testAnEditionsOwnNewRevision…` |
| A session saved on placeholder text | Restores with no notice: place, Acts 2:14 in column 2, the highlight on "loved the world", the ink, the note | `testASessionSavedOnThePlaceholder…` (made-up pack and real pack) |

Two bugs found and fixed on the way, both older than this task:

- **A restored place at the end of an edition moved.** Rev 22:21 came back at 22:14 and that was
  saved. The column grew at first layout, AppKit pushed the text back, and that was reported as the
  reader's scroll. Now a height change is not a scroll, and the room below the last chapter is
  recomputed with the verse kept in place. `testAPlaceAtAnEndOfTheEdition…`.
- **Closing the palette could reopen it.** Emptying the field in code ran `paletteTextChanged`.

**Re-anchoring, plainly.** No host calls `reanchor`, and a stored highlight keeps the revision it was
saved with (`bsb-2024`) and is painted by its offsets on any revision. That is right here only because
the overlay verses are byte-equal in the packs (B071.md, `just preview-pack-verify`). A span that no
longer fits its verse is not painted and stays stored (`testAHighlightThatNoLongerFits…`). A span that
still fits a changed verse would mark the wrong words: there is no lineage to detect that yet (specs/03,
specs/24 §2.2). Nothing shipped can reach that case until an edition changes revision.

## Speed

Release build, window 1440 × 900, the 10 × 3 × 4 session on real text, ≥ 200 samples, p95 against
specs/22 §7. "Drawn" stops after the window is laid out and drawn synchronously. Other agents were
working on this machine during the runs. `scripts/b071-mac-shots.sh measure`.

| Budget | Budget ms | Samples | p50 | p95 | max | |
|---|---:|---:|---:|---:|---:|---|
| **Keystroke → painted text results for that keystroke** (not the first paint; stale rows do not count), three editions, `bench-queries.json` from the second character | 50 | 464 | 10.67 | **29.58** | 66.82 | met |
| The keystroke on the main thread: passage, commands, tabs, drawn | 16 | 464 | 4.47 | **10.93** | 27.07 | met |
| **Open a chapter, warm**: typed jump, already laid out, long chapters included, two followers | 100 | 240 | 29.83 | **46.81** | 54.51 | met |
| Open a chapter never shown yet: read, hash, layout, two followers | 100 | 240 | 22.01 | **27.47** | 47.14 | met |
| **Workspace switch**, real text | 100 | 240 | 40.73 | **50.70** | 86.40 | met |
| **Cold launch to restored text**: process start → first frame, acceptance store (20 launches) | 1000 | 20 | 554.9 | **570.3** | 571.2 | met, under its 200 samples |
| The same from before `open` (LaunchServices included) | 1000 | 20 | 923.5 | **940.1** | 942.4 | met, by 60 ms |
| Restore: file read → text laid out | 100 | 20 | 154.7 | **157.6** | 158.1 | **missed** |
| Restore: file read → first frame | 100 | 20 | 302.2 | **304.9** | 305.8 | **missed** |

Load average during that run: 4.5 at the start, 3.1 at the end (other agents, `b071web` among them, working).

- All 464 keystroke samples saw the list body render with the answer before the clock stopped; 463
  had text rows. The clock starts at the model's `paletteTextChanged`, not at a physical key event.
- **Single keystrokes do exceed a frame.** The worst main-thread keystroke was 27 ms and the worst
  end-to-end sample 67 ms. Typing 240 keystrokes at 90 ms, frames over one refresh / over two:

  | Run | Load avg | Over one | Over two |
  |---|---:|---:|---:|
  | Pack (four runs, 6–7 Nov) | 2.8–4.7 | 137, 35, 98, 48, 140, 51 | 15, 20, 30, 28, 18, 30 |
  | Placeholder library, no search (two runs) | about 3–4 | 50, 47 | 31, 29 |

  The spread within the pack runs is wider than the difference to the placeholder, so I cannot say
  whether search adds hitches. An alternating comparison (four pairs) was started and lost when the
  session went idle; only its first pack run (51 / 30, load 2.8) was recorded.
- **The restore row was already missed** (B074, 137.9 ms laid out, owner decision pending). It is now
  about 26 ms later on the same store. 4.6 ms is opening the packs; I did not break down the rest.
- **The launch was not truly cold.** I cannot drop the page cache without root, so the files were in
  memory. A first launch after boot is unmeasured.
- Rows of the existing §7 run that are over budget are the same AppKit menu rows as B077: right-click
  → menu window 62.58 p95, with the plain-menu floor at 46.78 and 72.16. Nothing else in that run is over.

| Existing §7 rows on real text | Budget | p50 | p95 |
|---|---:|---:|---:|
| Palette keystroke with no pack data rows (the measurement's words) | 16 | 3.61 | 11.34 |
| Tab switch within a column | 16 | 10.87 | 11.44 |
| Column focus | 16 | 7.33 | 7.92 |
| Tool change | 16 | 7.29 | 8.93 |
| Selection → selection group drawn | 16 | 11.06 | 11.27 |
| Open a chapter: typed jump outside the window | 100 | 31.58 | 44.87 |
| Scroll into the next verse, two followers | 16 | 6.88 | 9.06 |
| Labels after a scroll | 16 | 9.27 | 9.49 |
| Window edit, add a block below | 16 | 2.42 | 4.20 |

## What looks wrong on screen, for the owner (rendered as the packs hold it, not changed)

- **BSB Ps 11:** the title runs into verse 1 unmarked, while Ps 3 and Ps 12 show an italic title
  (`mac-open-psalm-poetry.png`, `mac-open-psalm-titles.png`; decision 5).
- **Poetry** reads as one run per verse, with no line structure (decision 4).
- **Qere** is not shown (decision 16). The WLC's About panel says so. **Sigla** are gone (decision 7).
- **Romans doxology:** SBLGNT Romans stops at 16:24 and the next thing is 1 Corinthians. The linked
  column's identity row still says "Rom 16:25", a verse it does not print
  (`mac-follow-past-end.png`). Absent verses show the same way.
- **English labels over Hebrew and Greek** ("Psalm 1", "Chapter 1") under a heading in the edition's
  own script (B071.md, Open).
- **A hit can add a second tab of an edition.** If WLC is a hidden tab in the column and BSB is shown,
  a WLC hit opens another WLC tab (`mac-hit-other-edition.png`). That is the core's rule as ruled; it
  follows from "a tab already *showing* that edition moves".
- The six apparatus signs in the SBLGNT panel render very small in the system font.
- The study note in column 3 is still the fixture's.

## Tests

`just test-reader mac` (Debug, in the app): 111 tests, 0 failures, including `PreviewPackTests` over a
made-up pack the test writes and `RealPackTests` over the built pack. `RealPackTests` skips with
`SKIPPED: no built preview pack` where none exists, so on such a machine those rows do not run.
`just test-swift`: 13 XCTests, 0 failures; conformance 1,844 of 1,844. The iOS app target builds
(`generic/platform=iOS Simulator`; no simulator was booted or changed). iPad does not read packs yet.

## Pictures

`docs/evidence/B071/mac-*.png` (17 files, 5.6 MB, retaken at eb6bc7f; `mac-search-failed.png` added, the WLC right-to-left picture dropped for the cap: `mac-about-wlc.png` shows the same text): the app draws its own window; `scripts/b071-mac-shots.sh`.
In the About pictures the panel's backdrop and arrow come out plain white, an artefact of the app
drawing its own popover (Liquid Glass does not draw that way, as in B077).

## Not done, not verified

- No physical keystrokes, and no screen recording: every sample and picture is scripted in-app.
- Lexicon and Word study over real Greek and Hebrew were not exercised beyond the existing tests.
- A Text row's disabled state is checked against the real session in Swift, with the words taken from a
  passage-row probe of the core. The web has `disabledText` (wasm only, 78fc3e0); this branch did
  not add a native one, to leave `crates/ffi` unchanged.
