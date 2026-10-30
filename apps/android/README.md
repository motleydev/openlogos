# apps/android

Kotlin + Jetpack Compose shell with a platform text surface and extended selection
(docs/adr/0001-rust-core-native-ui.md). No project exists yet.

## Environment

`nix develop .#android` pins JDK 21, Gradle and `cargo-ndk`, and Rust with the
`aarch64-linux-android` target. It does **not** provide the Android SDK or NDK:

- nixpkgs' `androidenv.composeAndroidPackages` would make the SDK, build tools, platform and NDK
  reproducible, but its closure is several gigabytes and the host this skeleton was built on had
  no room for it; the SDK also carries its own license terms that must be accepted explicitly
  (specs/06-nix-operations.md). Revisit when the Mac mini build host is provisioned (B063).
- Until then install the SDK and an NDK outside Nix (Android Studio or `sdkmanager`), accept the
  licenses, and export the paths in `.envrc.local`:

  ```sh
  export ANDROID_HOME="$HOME/Library/Android/sdk"
  export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/<version>"
  ```

  The shell warns on entry when either is missing, and `just doctor` reports both.

## CoreHost: the UniFFI Kotlin bindings test host (B061)

`CoreHost/` is a plain JVM Gradle project, not the app and not an Android module. It proves
that the Kotlin bindings generated from `crates/ffi` load the Rust core through JNA and give
the same answers as the Rust and Swift tests: `apiVersion()`, `parseReference("Jn 3:16, 18;
Rom 8:28-30")`, every fixture selection as a UTF-16 round trip through `selectionToAnchors`
and `anchorToUtf16`, `mapVerse` on the Psalm 9/10 table, `reanchor`, and
`runConformance().failed == 0u` (1371 cases). It needs neither the Android SDK nor the NDK.

```sh
nix develop .#android
just bindings-kotlin  # release build of libopenlogos_ffi.dylib for this Mac; uniffi-bindgen
                      # into apps/android/CoreHost/generated/kotlin (gitignored)
just test-kotlin      # the same, then `gradle test` in apps/android/CoreHost
```

What `scripts/kotlin-bindings.sh` does, and what is checked in:

- `cargo build -p openlogos-ffi --release --target aarch64-apple-darwin` (Xcode's clang on a
  Mac, as `scripts/apple-bindings.sh`). When `ANDROID_NDK_HOME` is set it also runs
  `cargo ndk --target arm64-v8a --platform 26 -- build -p openlogos-ffi --release` for the
  `aarch64-linux-android` `.so` the app will ship; without it that step is reported as skipped.
- `cargo run -p openlogos-ffi --features bindgen --bin uniffi-bindgen -- generate --library
  <dylib> --language kotlin`: library mode, package `org.openlogos.core`
  (`crates/ffi/uniffi.toml`).
- `gradle test -PopenlogosLibDir=<dir of the dylib>`; the build sets `jna.library.path` from it.
  Committed: `settings.gradle.kts`, `build.gradle.kts` (Kotlin 2.2.0, JNA 5.17.0, JUnit 5.12.2,
  org.json for reading the fixtures), `gradle.properties`, `gradle.lockfile` (every
  configuration locked) and `src/test/kotlin/.../CoreHostTest.kt`. Gradle itself and JDK 21
  come from the Nix shell, so there is no wrapper.

The Android app will use the same generated Kotlin with the `.so` from `cargo ndk` in
`jniLibs`; `docs/evidence/B061-bindings.md` records sizes and timings.

Release signing keys stay outside the repository and outside agent runs, as for Apple.

## B039 note: the Android Ink API (Jetpack `androidx.ink`), from its documentation

Not built here (no SDK or NDK). The assessment, with what the API exposes (raw `StrokeInput`
batches with x, y, elapsed time, pressure, tilt, orientation and tool type; versioned stock
brushes; meshes rather than outlines; Apache 2.0) and how it maps onto the ink-document
schema, is docs/evidence/B039.md, "Android Ink API". The Compose host for the ink margin would
be `InProgressStrokesView` for the live stroke and `CanvasStrokeRenderer` for the committed
ones, with the core's `inkOutline` as the portable renderer beside them.

## B005 sketch: a Compose text surface over the same anchors

Written for B005 (docs/evidence/B005.md, acceptance 4) from the platform's documented text
stack; nothing here was built or measured, since this machine has no Android SDK or NDK. The
full Android gate is its own task and must run the conformance cases below on a device.

**What the surface is.** A Compose `BasicText`/`Text` composable draws through
`androidx.compose.ui.text` (Minikin and HarfBuzz underneath), and `SelectionContainer` gives
the platform selection handles and toolbar across any number of composables. Verse markers,
poetry indentation and the attribution are `AnnotatedString` spans and inline content, exactly
as the TextKit 2 surface uses attributed runs, so the display string again contains runs that
are not core text and the same `RunMap` rule applies: a selection end inside a verse number
collapses to its edge, and a highlight the core hands back never covers one. One paragraph of
verses is one `AnnotatedString`; a chapter is a `LazyColumn` of paragraphs, which is the
virtualization the ADR asks for (one item per paragraph, laid out when visible). A 1,000-verse
synthetic set is 125 items of 8 verses, the shape the Apple measurement used.

**Offsets.** Compose counts `TextRange` in UTF-16 code units of the `AnnotatedString`, as Java
strings do, so the binding's `selectionToAnchors(paragraph, Utf16Range)` takes the display
range after the run map, without conversion, and `anchorToUtf16` gives back a range that
`AnnotatedString.Builder.addStyle(SpanStyle(background = …), start, end)` paints. The Kotlin
CoreHost already asserts every fixture selection through these two calls (B061), so the
Android question is only whether the platform selection reports the same code-unit range the
user sees, which is what the Apple and DOM evidence in B005 checked for their stacks.

**Grapheme clusters.** Compose places the cursor and the selection handles only on grapheme
boundaries (`BreakIterator`-based, the same ICU data the core's grapheme adapter follows), so a
user cannot select between a Hebrew letter and its dagesh or between an emoji and its skin-tone
modifier; a programmatic `TextRange` inside a cluster is widened by the surface with
`java.text.BreakIterator.getCharacterInstance()` before it goes to the core, as the Apple
surface does with composed character sequences and the DOM reader with `Intl.Segmenter`.

**Extended selection action mode.** The specs/21 §3 group (Highlight, Collect, Study, More)
goes into the selection toolbar, not over it. Compose 1.7's `TextToolbar` can be replaced
through `LocalTextToolbar` with one whose `showMenu` adds the four items ahead of Copy and
Select all; underneath it is the platform `ActionMode` (`ActionMode.Callback2` floating mode)
so the OS items keep their positions and the project items are a labelled group, as on Apple.
The callbacks receive the current `TextRange`, run the round trip above and hand the anchors to
the workspace; nothing in the toolbar reinterprets text.

**Cross-verse and right-to-left.** `SelectionContainer` spans composables, so a selection that
crosses a paragraph boundary is reported per `Text` with its own `TextRange`; the surface joins
adjacent ranges in display order and runs one `selectionToAnchors` per paragraph, which already
splits per segment. Hebrew paragraphs set `LocalLayoutDirection` to RTL inside an LTR workspace
and `TextStyle(textDirection = TextDirection.ContentOrRtl)`; verse markers then sit at the
reading start, as on Apple, with the stored text untouched (no normalisation; the fixture's
non-NFC WLC bytes must survive `AnnotatedString`, which stores the string as given).

**TalkBack.** `SelectionContainer` and `Text` expose the text with `semantics { }`; verse
markers get `contentDescription = "verse 16"` on their inline content, and a custom
accessibility action "Select verse" per marker gives the VoiceOver rotor's function. The
accessibility audit equivalent is the Accessibility Test Framework's `AccessibilityChecks` in
an instrumented test, which the Android gate should run on the same three-pane scenario.

**What could fail, and the alternative.** If `SelectionContainer` reports ranges after its own
word snapping (it extends a drag to word boundaries on long press) that cannot be reduced to
the arbitrary range the user dragged, the alternative is the platform `TextView` with
`Spannable` text inside `AndroidView`, whose `Selection.getSelectionStart/End` is exact and
whose `ActionMode.Callback` is the extended action mode directly; it is the same Minikin/
HarfBuzz stack with the same UTF-16 offsets, so the core side is unchanged. That is the
measurement the Android gate must make first.
