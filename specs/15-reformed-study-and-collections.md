# Reformed study and resource collections

## Audience and product scope

OpenLogos is designed around the study and resource-gathering needs of people of Reformed persuasion preparing to teach or preach. That audience determines which workflows and source collections receive development priority. It does not set a required interpretation of a passage, a doctrinal answer policy or a theological test for the reader.

Prioritize parallel Bible reading, original-language tools, cross references, useful commentaries and historical sources, personal annotations, source collections and Obsidian handoff. Service design, liturgical calendars and ordered prayer workflows, including Sunday Mass or Afternoon Prayer planning, are outside scope. The application does not need a religious-persona selector or a denominational onboarding questionnaire.

Reformed expository works, confessions and catechisms are useful acquisition candidates for this audience. Westminster, the Three Forms of Unity and the 1689 Baptist confession are distinct resources with their own claims and editions [S37–S39]. Including them is not adopting them as the application's doctrinal standard or instructing AI to agree with them. B055 checks the product boundary and resource priorities; it does not ratify a confession or require a theological governing board.

The initial navigation/catalog defaults can use the 66-book Protestant canon appropriate to the target audience. Imported works retain their actual divisions and canon metadata for accurate historical study and reference mapping. Ancient Jewish writings, church fathers and other historical witnesses remain available as contextual literature. Readers can intentionally search a broader imported collection. Library organization is distinct from deciding the meaning of a passage.

## The product stops at a useful handoff

The primary preparation loop is **read → compare → investigate → annotate → collect → hand off**. Drafting and delivering a sermon normally happen elsewhere. OpenLogos provides observations, highlights, resource collections and reliable references; it does not become a sermon word processor. Ordinary personal notes remain supported, but a sermon editor, manuscript lifecycle, delivery/teleprompter mode, slides and automatic sermon generation are outside the planned product.

Obsidian is the sole first-class external writing integration. This means a maintained plugin, direct vault workflow, native scripture block references and tested collection handoff. It does not mean the user must install Obsidian to read or collect. Other writing tools receive ordinary Markdown, citations and attachments through export/clipboard/share sheet; there are no parallel Word, Notion, Google Docs or Zotero integration tracks in this plan.

## Study collections

A named collection gathers resources for a passage, theme, study series or teaching assignment. It is a lightweight ordered set of evidence and personal observations, not a manuscript document or a second knowledge-management application. The same source item can appear in several collections without duplicating its source artifact. Removing a collection membership does not delete the underlying note or library resource.

The reader can add a passage reference, permitted quotation, commentary excerpt, lexical result, cross reference, PDF page locator, personal observation, margin sheet or selected research answer. P1 supports references, installed-text excerpts and observations; later item types appear only when their underlying feature exists. Items can be reordered, grouped under a few user-named headings, tagged and removed with undo. No nested project/task-management system is needed.

Each source-bearing item retains edition/work identity, source revision/hash, exact locator, applicable quotation rights and the original target anchor. Display author, title and translator/editor when known. A PDF item keeps page index and any printed page label separately. Personal observations and model output are distinct item types; neither becomes a verified source quotation by being collected.

Capture targets the visible active collection. A small confirmation shows the destination and offers Undo or Change collection without navigating away. No active collection means an unobtrusive chooser. Dragging is optional; keyboard, touch menu and selection actions do the same work. A duplicate capture focuses the existing item or offers a deliberate second observation; it does not silently create duplicate export blocks.

The collection drawer is normally closed. A small count/destination indicator gives feedback without another permanent column. Pin it when reviewing or arranging evidence; hide it again to restore reading space. The full notes/collections destination supports deliberate retrieval and editing without remaining open during every study session.

## Obsidian handoff

P1 includes a bounded first-class handoff: an approved package reader in the plugin, genuine materialized scripture blocks, and collection export/import with stable item IDs and links. Desktop and Obsidian mobile capabilities are tested separately. Standalone app-to-vault direct access depends on the OS/browser; file exchange remains a supported transport where direct access is unavailable. Live bidirectional vault editing can follow in P2 without turning the initial Obsidian experience into a generic untested export button.

An exported collection contains a readable Markdown index, ordered evidence blocks, source attribution, selected personal observations and optional selected attachments. Every block includes a stable ID and a return-to-source link. Native Obsidian links/embeds point to real materialized blocks, with edition/revision pinned. Standard Markdown references and human-readable locators remain useful outside Obsidian. A proposed app URI is supplementary, not the only surviving citation.

Use an app-owned resource folder, selected by the user. The plugin inserts links or embeds into the user's writing note on request. It never replaces that note's manuscript body. Re-export reconciles managed collection blocks by IDs and last-export hashes, preserving unmanaged text and surfacing concurrent edits. It offers a new resource revision when a source quotation changes. A rename, deleted block or changed collection order must not silently retarget an existing scripture embed.

Export preview shows the destination, quotation text, included notes and attachments, and restricted or unresolved items. Where quotation permission is denied or unknown, export reference metadata only with a visible reason; do not silently drop the source or copy its text. Public sharing has its own rights/privacy decision. Local handoff does not make personal observations public or send them to an LLM.

## Source selection without interpretive steering

Distinguish the edition's text, source-backed linguistic/textual evidence, attributed interpretations, personal notes and generated synthesis. A confession's proof texts are that edition's cited support, not the application's judgment that the passage proves its proposition. Commentary selection determines which sources are available; it does not turn their conclusions into system policy.

Discovery can prioritize useful Reformed expository resources, confessions/catechisms and historical sources for this audience. Candidate acquisition includes exact editions of Calvin's commentaries, selected Puritan works and later expository resources. Naming an author does not clear a modern translation or scan, nor validate every quotation or interpretation. Resources outside this tradition remain eligible when they serve the study task; relevance, rights and source quality are explicit criteria.

Exact reference and lexical search return the matches in the declared scope. They do not hide inconvenient results or alter counts to fit a doctrine. Research follows the user's actual question and the selected evidence, with sources and scope visible. It must not infer that the user wants a Reformed conclusion simply from the target audience or a preferred library collection. If the user explicitly asks how a named author or confession interprets a passage, present that attributed perspective accurately.

Research turns record selected source collections, evidence and the explicit question, not an implicit theological persona. Where the retrieved sources disagree, identify the relevant disagreement and limitations. No generic survey of every tradition is required, and no unsupported consensus is invented. AI may help search, compare and organize resources after invocation; it cannot certify doctrine or designate a collection theologically approved.

## Acceptance and ownership

- B055: product scope prioritizes study and resource collection for the intended audience; no confessional charter, automatic interpretive persona or service/prayer-planning feature is introduced.
- B058: gather a passage, a source excerpt and an observation into a collection offline without moving the reading panes. Reorder and export them; restore stable IDs and distinguish permitted quotations from metadata-only restricted items.
- B043/B060: insert scripture blocks and a collection into an actual Obsidian vault; write a separate manuscript note; re-export after external edits and a rename; preserve the manuscript, unmanaged content and prior links. Test mobile fallback separately.
- B059: one cleared confessional resource collection has stable article/question locators and edition-specific proof-text attribution. Selecting it changes retrieval scope only; it neither rewrites Bible text nor injects an answer stance.
- B051/B052: human-reviewed cases include conflicting commentary, a question with no requested theological perspective, an explicitly requested author/confession perspective, false quotation, unsupported consensus and pressure to distort translation. Evaluate evidence, attribution and compliance with the actual question rather than agreement with a doctrine.

Keep the existing 14 agent roles. Product stewardship owns audience/workflow scope, source stewardship owns acquisition preparation, qualified reviewers assess evidence and rights, and client/Obsidian engineering owns the handoff. There is no additional doctrinal ratification role or confessional approval gate. Existing human source, language, security and release responsibilities remain.
