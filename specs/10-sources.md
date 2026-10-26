# Primary-source inventory and reuse decisions

Checked 26 October 2026. These are research findings, not legal clearance. Production import must archive the exact license and artifact version. Source IDs in other documents refer here.

## S01 — Paperclip adapter and agent configuration

[Paperclip adapter and agent configuration](https://github.com/paperclipai/paperclip/blob/master/docs/adapters/overview.md) · [Additional primary source](https://github.com/paperclipai/paperclip/blob/master/docs/guides/board-operator/managing-agents.md)

Adapters bridge orchestration and runtimes; installed schemas and model support must be inspected. Agent budget/run policy varies by version.

Status: `implementation_reference`; no production artifact pinned yet.

## S02 — ESV publisher permissions

[ESV publisher permissions](https://www.crossway.org/permissions/) · [Additional primary source](https://api.esv.org/)

Copyrighted translation with specific quotation/digital terms; ordinary access does not grant the proposed open-corpus or AI translation workflow. Default exclusion; request explicit written scope if needed.

Status: `restricted`; no production artifact pinned yet.

## S03 — ESV API terms

[ESV API terms](https://api.esv.org/)

API imposes storage/display/redistribution limits and other conditions. A freely obtainable API is not an offline open-text license.

Status: `restricted`; no production artifact pinned yet.

## S04 — World English Bible

[World English Bible](https://ebible.org/eng-web/webfaq.htm) · [Additional primary source](https://ebible.org/study/content/texts/engwebp/about.html)

Publisher identifies the text as public domain; preserve edition and text integrity. Changed text must not be represented as the unchanged World English Bible.

Status: `candidate_open`; no production artifact pinned yet.

## S05 — STEPBible Data

[STEPBible Data](https://github.com/STEPBible/STEPBible-Data/blob/master/README.md)

Repository states CC BY 4.0 with attribution/change recording. Inspect each chosen file and its upstream dependencies; useful for tagged texts, lexical data and mappings.

Status: `candidate_open`; no production artifact pinned yet.

## S06 — SBL Greek New Testament

[SBL Greek New Testament](https://github.com/Faithlife/SBLGNT) · [Additional primary source](https://www.sblgnt.com/)

Current upstream describes CC BY 4.0. Pin exact text edition; critical editorial choices are not interchangeable with other Greek editions.

Status: `candidate_open`; no production artifact pinned yet.

## S07 — Open Scriptures Hebrew Bible

[Open Scriptures Hebrew Bible](https://github.com/openscriptures/morphhb) · [Additional primary source](https://github.com/openscriptures/morphhb/blob/master/LICENSE.md)

WLC underlying text is public domain; morphology/lemma layer is CC BY 4.0. Preserve upstream token IDs and normalization cautions.

Status: `candidate_open`; no production artifact pinned yet.

## S08 — unfoldingWord source-language resources

[unfoldingWord source-language resources](https://unfoldingword.org/for-translators/content/)

UHB/UGNT and related grammars/lexicons are open-licensed candidates, generally CC BY-SA 4.0 on the referenced page. Check the exact work/version and trademark terms before adaptation.

Status: `candidate_open`; no production artifact pinned yet.

## S09 — OpenBible cross references

[OpenBible cross references](https://openbible.info/labs/cross-references/)

Roughly 340,000 links, primarily derived from Treasury of Scripture Knowledge, with additional sources/votes. Page links a CC BY license; ESV quotations on the site are separate. Import reference links/attribution only after recording the exact linked license. Votes are not accuracy scores.

Status: `candidate_open`; no production artifact pinned yet.

## S10 — Perseus canonical Greek literature

[Perseus canonical Greek literature](https://github.com/PerseusDL/canonical-greekLit)

Repository default CC BY-SA 4.0 unless indicated otherwise, with varying object rights and unverified headers. Check each TEI edition/translation and preserve attribution.

Status: `per_item_review`; no production artifact pinned yet.

## S11 — CCEL and Early Church Fathers

[CCEL and Early Church Fathers](https://www.ccel.org/about/copyright.html) · [Additional primary source](https://ccel.org/s/schaff/ecf/index.html)

Ancient/patristic discovery source. Many underlying books are US public domain, but site content/editions and permission statements are mixed. Do not blanket-import; seek a clearly licensed artifact or separate public-domain transcription.

Status: `per_item_review`; no production artifact pinned yet.

## S12 — CrossWire SWORD and module rights

[CrossWire SWORD and module rights](https://www.crosswire.org/sword/about/license.jsp) · [Additional primary source](https://wiki.crosswire.org/Copyright)

Engine is GPL; each text module has its own rights. Distribution permission to CrossWire does not automatically authorize this project to redistribute everything.

Status: `per_item_review`; no production artifact pinned yet.

## S13 — Tailscale network layers

[Tailscale network layers](https://tailscale.com/docs/concepts/control-data-planes) · [Additional primary source](https://tailscale.com/security)

WireGuard data transport is distinct from coordination/identity/relay behavior. Mesh access is not app authorization or anonymity.

Status: `implementation_reference`; no production artifact pinned yet.

## S14 — Headscale DERP self-hosting

[Headscale DERP self-hosting](https://headscale.net/stable/ref/derp/)

Supports embedded/custom DERP; public-relay map exclusion must be explicit and connectivity tested. Self-hosting can create its own availability bottleneck.

Status: `implementation_reference`; no production artifact pinned yet.

## S15 — Keycloak OIDC

[Keycloak OIDC](https://www.keycloak.org/securing-apps/oidc-layers)

Reference for self-hosted OIDC and supported flows; use authorization code plus PKCE and carefully scoped redirects.

Status: `implementation_reference`; no production artifact pinned yet.

## S16 — SQLite browser and Expo storage

[SQLite browser and Expo storage](https://sqlite.org/wasm/doc/tip/persistence.md) · [Additional primary source](https://docs.expo.dev/versions/latest/sdk/sqlite/)

Browser VFS/worker support and cross-origin isolation requirements vary. Test chosen storage adapter and Safari rather than assuming native parity.

Status: `implementation_reference`; no production artifact pinned yet.

## S17 — W3C Web Annotation Data Model

[W3C Web Annotation Data Model](https://www.w3.org/TR/annotation-model/)

TextQuote and TextPosition selectors provide useful interoperable anchor conventions; immutable edition context and reanchoring policy remain project requirements.

Status: `implementation_reference`; no production artifact pinned yet.

## S18 — MorphGNT SBLGNT

[MorphGNT SBLGNT](https://github.com/morphgnt/sblgnt/blob/master/README.md)

Morphological analysis uses CC BY-SA 3.0; README references an older SBLGNT EULA whereas current SBLGNT upstream is CC BY 4.0. Resolve edition/version provenance and layered obligations, not one blanket license.

Status: `per_item_review`; no production artifact pinned yet.

## S19 — NixOS Android tooling

[NixOS Android tooling](https://wiki.nixos.org/wiki/Android)

androidenv supports composing SDK inputs. Exact SDK/NDK/license/build configuration remains an implementation verification task.

Status: `implementation_reference`; no production artifact pinned yet.

## S20 — AndBible

[AndBible](https://github.com/AndBible/and-bible) · [Additional primary source](https://docs.andbible.org/en/latest/faq.html)

GPL-3.0-or-later offline Android study app. Study panes, note/export and module workflows; no license-blind copying into differently licensed clients.

Status: `inspiration`; no production artifact pinned yet.

## S21 — Ezra Bible App

[Ezra Bible App](https://github.com/ezra-bible-app/ezra-bible-app)

SWORD-based cross-platform topical study with a GPL-2.0 repository label. Inspect exact licensing before reuse; useful workflow inspiration for tags and notes.

Status: `inspiration`; no production artifact pinned yet.

## S22 — Calibre

[Calibre](https://github.com/kovidgoyal/calibre)

Open-source ebook management/conversion reference. Useful for library ownership/export concepts; dependency and copied-code licenses require separate review.

Status: `inspiration`; no production artifact pinned yet.

## Initial corpus proposal

- First complete English text: one pinned World English Bible edition.
- Second parallel text: a separately reviewed historical English edition (e.g. ASV 1901) or a verified open modern edition. This inventory does not clear a particular ASV digital artifact. P0 resolves that choice; test fixtures may use synthetic content until then.
- Source languages: pinned OSHB/WLC and SBLGNT with separately licensed morphology. Prefer STEP's appropriate reviewed datasets where they reduce unnecessary invention.
- Cross references: OpenBible reference data with exact linked license and source attribution; preserve its TSK ancestry and distinguish voting/ranking data. Do not scrape ESV verse wording from its pages.
- Ancient work: a short text such as the Didache or a selected patristic work in a specifically cleared original/translation edition. Discover through Perseus/CCEL, but acquire only after artifact-level rights review. A modern English translation is not automatically free because the Didache is ancient.

## Next source-research tasks, deliberately not pre-cleared

Find usable editions of the Septuagint, Vulgate/Old Latin and Syriac/Peshitta; record textual scope, editor, translation layers and rights. Evaluate ancient Jewish and wider Greco-Roman context, Josephus/Philo, early fathers and later commentaries without mixing eras. Consider TEI/CTS conventions and USFM/USX/OSIS adapters after checking their official specifications and chosen parser licenses. Evaluate public-domain historical German Bibles separately from contemporary revisions.

No Nestle-Aland/UBS apparatus, BHS/BHQ apparatus, BDAG/HALOT, contemporary copyrighted commentary or Logos library content is assumed available. Public-domain lexicons may be useful historically but their age and scholarly limitations must be labeled.

## Inspiration scope

Start with public workflows and existing open readers rather than attempting to decode Logos internals. AndBible, Ezra and SWORD cover much of the desired offline library/study behavior. STEP informs original-language navigation. Calibre informs user-owned library management and export. A later interoperability project would have its own evidence, compatibility and rights review; this blueprint requires none.


# Revision 2 primary sources

Checked 26 October 2026. Provider documentation is version-sensitive; capability and terms checks are release gates.

## S23 — OpenAI ChatGPT plan usage for OSS

[OpenAI ChatGPT plan usage for OSS](https://developers.openai.com/siwc/token-sharing-open-source)

Official optional authorization path for eligible open-source/locally hosted apps; platform and deployment conditions require verification.

Status: `implementation_reference`.

## S24 — Codex app-server plan usage

[Codex app-server plan usage](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server)

Documented app-server integration exists; catalog listing alone does not prove model entitlement.

Status: `implementation_reference`.

## S25 — OpenAI plan-usage preview limitations

[OpenAI plan-usage preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)

The preview has distinct request/tool/history restrictions; adapter conformance must be tested rather than assuming ordinary API parity.

Status: `implementation_reference`.

## S26 — Obsidian Vault API

[Obsidian Vault API](https://github.com/obsidianmd/obsidian-developer-docs/blob/main/en/Plugins/Vault.md)

Vault API includes read and guarded process-style updates; use it for plugin file operations.

Status: `implementation_reference`.

## S27 — Obsidian mobile development

[Obsidian mobile development](https://docs.obsidian.md/Plugins/Getting%20started/Mobile%20development)

Node/Electron APIs are unavailable on mobile; platform support must be explicit.

Status: `implementation_reference`.

## S28 — Obsidian native block links

[Obsidian native block links](https://obsidian.md/help/links)

Native block IDs and #^ references target actual note blocks; materialized scripture can use the same mechanism.

Status: `implementation_reference`.

## S29 — perfect-freehand

[perfect-freehand](https://github.com/steveruizok/perfect-freehand)

MIT stroke geometry library; it is not a complete handwritten-note editor.

Status: `candidate_open`.

## S30 — Excalidraw source and license

[Excalidraw source and license](https://github.com/excalidraw/excalidraw/blob/master/LICENSE)

MIT editor candidate; validate the selected package, assets and platform behavior.

Status: `candidate_open`.

## S31 — tldraw licensing

[tldraw licensing](https://tldraw.dev/faq)

Current SDK is source-available/commercially licensed; excluded from the fully open-source default.

Status: `restricted`.

## S32 — Claude Code product integration terms

[Claude Code product integration terms](https://code.claude.com/docs/en/legal-and-compliance)

Current terms describe conditional integration of unmodified Claude Code with end-user authentication; review exact deployment obligations.

Status: `implementation_reference`.

## S33 — Claude Agent SDK authentication statement

[Claude Agent SDK authentication statement](https://platform.claude.com/docs/en/agent-sdk/overview)

SDK page still restricts third-party subscription login without approval; reconcile with the product-integration page before enabling the selected route.

Status: `implementation_reference`.

## S34 — Claude Code headless mode

[Claude Code headless mode](https://code.claude.com/docs/en/headless)

Programmatic mode exists; bare mode does not use subscription OAuth. Configuration isolation must be designed for the chosen auth mode.

Status: `implementation_reference`.

## S35 — OpenAI self-hosted VM plan usage

[OpenAI self-hosted VM plan usage](https://developers.openai.com/siwc/token-sharing-open-source/self-hosted-vms)

Documented personal VM path has credential/host-ID/refresh and current revocation-attribution limitations; do not generalize to a shared commercial relay.

Status: `implementation_reference`.

## S36 — Android Ink API

[Android Ink API](https://developer.android.com/develop/ui/views/touch-and-input/stylus-input/about-ink-api)

Official low-latency ink API candidate for a native adapter; verify exact library version/license and portable data conversion in the spike.

Status: `implementation_reference`.

## Revision 3 editorial and design references

**S37 — Westminster Confession, OPC edition.** Primary confessional reference; chapter 1 supports distinguishing Scripture and subordinate confession. Exact edition, proof texts and rights still require acquisition review. Source: https://opc.org/wcf.html

**S38 — Three Forms of Unity reference.** Primary church descriptions identify Belgic Confession, Heidelberg Catechism and Canons of Dort. No adoption of those churches’ other policies is implied. Search retrieval succeeded; the direct CRC page returned 403 during this audit. Source: https://www.crcna.org/welcome/beliefs?language=en Additional reference: https://www.frcna.org/our-beliefs

**S39 — 1689 London Baptist Confession, RBNet.** Primary Reformed Baptist confessional reference; offered as a possible identified collection, not silently selected as the project confession. Source: https://reformedbaptistnetwork.com/1689lbcf/

**S40 — WCAG 2.2.** Normative web accessibility requirements; product target sizes and chrome budgets are separate design proposals. Source: https://www.w3.org/TR/WCAG22/

**S41 — Obsidian workspace and ribbon.** Documented collapsible workspace elements and hideable ribbon inform independent compact-workspace prototypes. Source: https://obsidian.md/help/workspace Additional reference: https://obsidian.md/help/User%2Binterface/Ribbon

**S42 — Zed workspace actions.** Documented dock toggles and editor zoom offer relevant focus/restore interaction patterns; no copied implementation. Source: https://zed.dev/docs/all-actions

**S43 — Zotero collections and tags.** Documented nonduplicating collection membership informs source gathering. Zotero integration is not in scope. Source: https://www.zotero.org/support/collections_and_tags

These pages establish reference context, not permission to import their text. Screenshots are private user-provided workflow references and are not committed or used as corpus sources. The product interaction choices in specs/16-design-guide.md are our proposals; no usability or accessibility claim about a competitor is inferred from six screenshots.
