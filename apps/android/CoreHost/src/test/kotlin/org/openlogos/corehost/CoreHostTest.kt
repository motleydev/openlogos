// The four checks B061 asks of every host, against the generated Kotlin bindings loaded
// through JNA: the API version, a citation parse, a UTF-16 selection round trip through an
// Anchor on the Hebrew cantillation fixture, and the conformance suite. Fixture texts and
// expected offsets come from the core through conformanceFixturesJson(), so Kotlin asserts
// the same numbers the Rust and Swift tests do.
package org.openlogos.corehost

import org.json.JSONArray
import org.json.JSONObject
import org.openlogos.core.Paragraph
import org.openlogos.core.ReanchorResult
import org.openlogos.core.Reference
import org.openlogos.core.ReferenceException
import org.openlogos.core.ReviewFlag
import org.openlogos.core.Revision
import org.openlogos.core.ScalarRange
import org.openlogos.core.Segment
import org.openlogos.core.ShellCommandArgs
import org.openlogos.core.ShellCommandException
import org.openlogos.core.ShellHandle
import org.openlogos.core.ShellPaletteContext
import org.openlogos.core.ShellPaletteSection
import org.openlogos.core.ShellPlatform
import org.openlogos.core.TokenSpan
import org.openlogos.core.Utf16Range
import org.openlogos.core.MappingStatus
import org.openlogos.core.anchorToUtf16
import org.openlogos.core.apiVersion
import org.openlogos.core.conformanceFixturesJson
import org.openlogos.core.formatHuman
import org.openlogos.core.formatOsis
import org.openlogos.core.mapVerse
import org.openlogos.core.parseReference
import org.openlogos.core.reanchor
import org.openlogos.core.runConformance
import org.openlogos.core.selectionToAnchors
import org.openlogos.core.shellRegistry
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertTrue

class CoreHostTest {
    @Test
    fun apiVersionMatchesTheCore() {
        assertEquals("0.1.2", apiVersion())
        assertEquals(apiVersion(), runConformance().apiVersion)
    }

    @Test
    fun parsesACitation() {
        val set = parseReference("Jn 3:16, 18; Rom 8:28-30")
        assertEquals(apiVersion(), set.apiVersion)
        assertEquals("John.3.16;John.3.18;Rom.8.28-Rom.8.30", set.osis)
        assertEquals("John 3:16, 18; Rom 8:28–30", set.human)
        assertEquals(3, set.ranges.size)
        assertEquals(Reference(book = "John", chapter = 3u, verse = 16u, subverse = null), set.ranges[0].start)
        assertEquals("John.3.18", set.ranges[1].osis)
        assertEquals(28u, set.ranges[2].start.verse)
        assertEquals(30u, set.ranges[2].end.verse)
        assertEquals("Rom 8:28–30", set.ranges[2].human)
        assertTrue(set.flags.isEmpty())

        assertEquals("John.3.16", formatOsis(set.ranges[0].start))
        assertEquals("John 3:16", formatHuman(set.ranges[0].start))

        assertEquals(listOf<ReviewFlag>(ReviewFlag.ChapterOnly(range = 0u)), parseReference("Ps 23").flags)

        val error = assertFailsWith<ReferenceException.UnknownBook> { parseReference("Nope 3:16") }
        assertEquals("Nope", error.name)
    }

    @Test
    fun mapsAVerseThroughTheFixtureTable() {
        val psalm = parseReference("Ps 9:1").ranges[0].start
        val result = mapVerse(psalm, "eng", "lxx", emptyList())
        assertEquals(MappingStatus.EXACT, result.status)
        assertEquals(listOf(Reference(book = "Ps", chapter = 9u, verse = 2u, subverse = null)), result.targets)
        assertEquals(listOf("eng", "org", "lxx"), result.path)

        val genesis = parseReference("Gen 1:1").ranges[0].start
        val unmapped = mapVerse(genesis, "eng", "org", emptyList())
        assertEquals(MappingStatus.UNMAPPED, unmapped.status)
        assertTrue(unmapped.targets.isEmpty())
    }

    /** Every selection of every text fixture round-trips, and the span indexes a Java string. */
    @Test
    fun utf16SelectionRoundTripsThroughAnAnchor() {
        var checked = 0
        for (fixture in textFixtures()) {
            for (selection in fixture.selections) {
                val segment = checkNotNull(fixture.segments[selection.segmentId]) {
                    "${fixture.id}: no segment ${selection.segmentId}"
                }
                val paragraph = Paragraph(
                    editionId = fixture.editionId,
                    contentRevision = fixture.contentRevision,
                    joiner = "",
                    segments = listOf(segment),
                )
                val anchors = selectionToAnchors(paragraph, selection.utf16)
                assertEquals(1, anchors.size, "${fixture.id}: ${selection.name}")
                val anchor = anchors[0]
                assertEquals(apiVersion(), anchor.apiVersion)
                assertEquals(selection.segmentId, anchor.segmentId)
                assertEquals(selection.scalar, anchor.range, "${fixture.id}: ${selection.name}")
                assertEquals(selection.exact, anchor.quote?.exact, "${fixture.id}: ${selection.name}")
                assertEquals(selection.tokens, anchor.tokens, "${fixture.id}: ${selection.name}")

                val back = anchorToUtf16(anchor, segment.text)
                assertEquals(selection.utf16, back, "${fixture.id}: ${selection.name}")
                // What a platform text view does with the span: Java strings count UTF-16 units.
                assertEquals(selection.exact, segment.text.substring(back.start.toInt(), back.end.toInt()))
                checked += 1
            }
        }
        assertTrue(checked >= 10, "the fixtures carry at least ten selections, saw $checked")
    }

    @Test
    fun hebrewSelectionIsTheThirdWord() {
        val fixture = textFixtures().first { it.id == "hebrew-cantillation-genesis-1" }
        val verse = checkNotNull(fixture.segments["Gen.1.1"])
        val paragraph = Paragraph(
            editionId = fixture.editionId,
            contentRevision = fixture.contentRevision,
            joiner = "",
            segments = listOf(verse),
        )
        // אֱלֹהִ֑ים: UTF-16 units 21 until 30 of the verse (no surrogates in Hebrew).
        val anchors = selectionToAnchors(paragraph, Utf16Range(start = 21u, end = 30u))
        assertEquals(1, anchors.size)
        assertEquals(listOf("wlc:01001001003"), anchors[0].tokens)
        assertEquals("אֱלֹהִ֑ים", anchors[0].quote?.exact)
        assertEquals(Utf16Range(start = 21u, end = 30u), anchorToUtf16(anchors[0], verse.text))

        // A second revision with a word inserted before the verse: the anchor follows its token id.
        val old = Revision(contentRevision = fixture.contentRevision, text = verse.text, tokens = verse.tokens)
        val shifted = verse.tokens.map {
            TokenSpan(id = it.id, range = ScalarRange(start = it.range.start + 4u, end = it.range.end + 4u))
        }
        val new = Revision(contentRevision = "r2", text = "xxx " + verse.text, tokens = shifted)
        val moved = assertIs<ReanchorResult.Moved>(reanchor(anchors[0], old, new))
        assertEquals(ScalarRange(start = 25u, end = 34u), moved.anchor.range)
        assertEquals("r2", moved.anchor.contentRevision)
    }

    @Test
    fun conformanceSuitePasses() {
        val report = runConformance()
        val failures = report.cases.filter { !it.passed }
            .joinToString("\n") { "${it.fixture} / ${it.name}: expected ${it.expected}, actual ${it.actual}" }
        assertEquals(0u, report.failed, failures)
        assertTrue(report.passed > 0u)
        assertEquals(report.cases.size, report.passed.toInt())
        println("conformance cases: ${report.passed} passed, ${report.failed} failed")
    }

    // B070: the workspace shell through the UniFFI handle.
    @Test
    fun shellCommandsUndoAndPalette() {
        val shell = ShellHandle.fixture()
        assertEquals(2, shell.session().workspaces.size)
        val outcome = shell.apply("column.new", ShellCommandArgs(), false)
        assertTrue(outcome.handled && outcome.recorded)
        assertEquals(listOf(1u, 2u, 3u, 4u), shell.session().workspaces[0].columns.map { it.number })
        assertEquals("New column", shell.session().undoTitle)
        shell.undo()
        assertEquals(3, shell.session().workspaces[0].columns.size)
        val error = assertFailsWith<ShellCommandException.Disabled> {
            shell.apply("column.focus", ShellCommandArgs(column = 9u), false)
        }
        assertEquals("no_such_column", error.kind)
        val context = ShellPaletteContext(hasSelection = false, platform = ShellPlatform.WEB)
        val result = shell.paletteQuery("Jn 3:16 @2", context)
        assertEquals(2u, result.targetColumn)
        val first = result.groups.first().items.first()
        assertEquals(ShellPaletteSection.PASSAGE, first.section)
        val navigated = shell.apply(first.commandId, first.args, false)
        assertEquals(listOf("t1", "t4"), navigated.followedTabIds)
        assertEquals(ShellPaletteSection.TEXT, shell.paletteQuery("#loved", context).groups.first().section)
        assertEquals(listOf("Alt+T"), shellRegistry().first { it.id == "tab.new" }.shortcuts.web)
        val records = shell.records()
        val reopened = ShellHandle.fromRecords(records.sessionJson, records.workspaceJsons)
        assertEquals(shell.session().workspaces, reopened.session().workspaces)
    }

    // Fixture decoding

    class LoadedSelection(
        val name: String,
        val segmentId: String,
        val scalar: ScalarRange,
        val utf16: Utf16Range,
        val exact: String,
        val tokens: List<String>,
    )

    class LoadedFixture(
        val id: String,
        val editionId: String,
        val contentRevision: String,
        val segments: Map<String, Segment>,
        val selections: List<LoadedSelection>,
    )

    private fun JSONObject.span(key: String): Pair<UInt, UInt> {
        val span = getJSONObject(key)
        return span.getLong("start").toUInt() to span.getLong("end").toUInt()
    }

    private fun JSONArray.objects(): List<JSONObject> = (0 until length()).map { getJSONObject(it) }

    private fun JSONArray.strings(): List<String> = (0 until length()).map { getString(it) }

    private fun textFixtures(): List<LoadedFixture> =
        JSONArray(conformanceFixturesJson()).objects()
            .filter { it.getString("kind") == "text" }
            .map { fixture ->
                val segments = fixture.getJSONArray("segments").objects().associate { segment ->
                    val id = segment.getString("segment_id")
                    id to Segment(
                        segmentId = id,
                        text = segment.getString("text"),
                        tokens = segment.getJSONArray("tokens").objects().map { token ->
                            TokenSpan(
                                id = token.getString("id"),
                                range = ScalarRange(
                                    start = token.getLong("start").toUInt(),
                                    end = token.getLong("end").toUInt(),
                                ),
                            )
                        },
                    )
                }
                val selections = fixture.getJSONArray("selections").objects().map { selection ->
                    val (scalarStart, scalarEnd) = selection.span("scalar")
                    val (utf16Start, utf16End) = selection.span("utf16")
                    LoadedSelection(
                        name = selection.getString("name"),
                        segmentId = selection.getString("segment_id"),
                        scalar = ScalarRange(start = scalarStart, end = scalarEnd),
                        utf16 = Utf16Range(start = utf16Start, end = utf16End),
                        exact = selection.getString("exact"),
                        tokens = selection.getJSONArray("tokens").strings(),
                    )
                }
                LoadedFixture(
                    id = fixture.getString("id"),
                    editionId = fixture.getString("edition_id"),
                    contentRevision = fixture.getString("content_revision"),
                    segments = segments,
                    selections = selections,
                )
            }
}
