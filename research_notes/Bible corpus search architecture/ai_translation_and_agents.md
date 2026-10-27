# AI-Assisted Bible Translation, Translation-Research Pipelines, and Agents over Deterministic Scholarly Data

Research date: 2026-10-27. Claims marked **[UNVERIFIED]** come from background knowledge or secondary or aggregator sources that I could not confirm against a primary source in this session. About 20 tool calls were used, so the coverage is broad rather than exhaustive.

## How do SIL, unfoldingWord, ETEN, Clear Bible, and others use MT, alignment, and LLMs (2025–2026), and what review workflows do they use?

### Takeaway
The most mature production pipeline is SIL's: Serval (an MIT-licensed REST API) runs NMT, SMT, and word-alignment engines. Scripture Forge uses it to fine-tune Meta's NLLB on a project's completed books plus a "model text", and it produces rough drafts of the next book. Those drafts flow into Paratext and still go through the normal edit, team-check, and consultant-check stages. Every org I found frames AI output as a draft or resource that skilled humans edit, never as a finished translation.

### Cited Findings
- **Serval architecture.** The README calls Serval "an API supporting Computer Aided Translation for all the languages of the world!" It is a modular monolith with these modules: Serval.Translation (engine management and pretranslation assembly), Serval.WordAlignment (word alignment engine management), Serval.DataFiles (corpus management), and Serval.Machine.Shared. That last module implements NMT, SMT Transfer, Statistical, and Echo engines and queues ClearML GPU training jobs. Serval persists to MongoDB, stores models and training data in S3, and is MIT-licensed. — [sillsdev/serval GitHub](https://github.com/sillsdev/serval)
- **Scripture Forge drafting has two steps.** Step 1 fine-tunes Meta's NLLB base model on parallel texts: a "model text" (a source version with trusted exegesis and style) plus the project's completed books. Step 2 drafts the next book from the model text. The deck states: "Rough drafts contain errors and require editing by skilled translators." — [SIL Scripture Forge drafting presentation (Google Slides)](https://docs.google.com/presentation/d/1t2R8e5dpjoign9IkKMaFI-T_Wk6NpYZ0/htmlpresent)
- **Editing order for an AI draft.** (1) Read the primary source version. (2) Read other versions. (3) Edit for accuracy, clarity, naturalness, grammar, and spelling. (4) Verify the exegesis against exegetical resources. Team checks and consultant checks stay in the process. Drafts can also be compared against a completed book during checking. — [SIL deck](https://docs.google.com/presentation/d/1t2R8e5dpjoign9IkKMaFI-T_Wk6NpYZ0/htmlpresent)
- **Further drafting details.** Name translation pairs and other sentence pairs can be loaded as CSV, TSV, or XLSX. Paratext key terms are mostly not used for fine-tuning; only proper-name key terms are used automatically. Footnotes are not transferred. A draft takes about 2.5 hours. Teams sync with Paratext (Send/Receive) before and after drafting. — [SIL deck](https://docs.google.com/presentation/d/1t2R8e5dpjoign9IkKMaFI-T_Wk6NpYZ0/htmlpresent)
- **Back-translation drafting.** A model fine-tuned on the existing back translation generates starter back-translation drafts that keep the team's existing style and notation choices. — [SIL deck](https://docs.google.com/presentation/d/1t2R8e5dpjoign9IkKMaFI-T_Wk6NpYZ0/htmlpresent)
- **Data requirements (secondary sources).** SIL reportedly recommends about 8,000 verses, and one complete Gospel as the minimum for NT drafting. — [learnofchrist.com review](https://learnofchrist.com/resources/scripture-forge). The SIL deck uses 8,000 NT verses only as an example, not a stated requirement. — [SIL deck](https://docs.google.com/presentation/d/1t2R8e5dpjoign9IkKMaFI-T_Wk6NpYZ0/htmlpresent)
- **Adoption (secondary source).** As of mid-2025, AI drafting was implemented or in preparation in nearly 500 projects. Draft-using projects reportedly drafted about 1,000 more verses on average (Paratext data from Sep 2024 to Sep 2025). Faith Comes By Hearing and SIL are piloting oral Bible translation, in which recordings are transcribed and then drafted via Serval. Scripture Forge was reportedly at v5.60.2 in July 2026. — [faith.tools: Scripture Forge](https://faith.tools/app/scripture-forge) **[secondary; not confirmed with SIL]**
- **unfoldingWord BT Servant.** An AI assistant built on unfoldingWord's curated translation resources. It is delivered through WhatsApp and Telegram for low-connectivity users, was piloted in Kenya and Indonesia, and is part of the FoundationsBT.com hub (video training, coaching, and AI chat). — [MNN Online](https://www.mnnonline.org/news/unfoldingword-builds-hub-for-bible-translators-working-in-unreached-communities/); [990finder](https://990finder.com/141888977/) **[the MNN article's date header looked inconsistent; timing unverified]**
- **Gateway Languages.** unfoldingWord's model routes original-language resources through gateway languages (English, Spanish, French, Portuguese, Hindi, and others) into heart languages. The GL Manual describes this process without mentioning AI. — [docs.page uW Tools](https://docs.page/unfoldingWord/uW-Tools-Collab/1-why-unfoldingword-translation-resources); [GL Manual](https://gl-manual.readthedocs.io/)
- **Clear Bible.** Clear produces freely licensed linguistic datasets for the whole Bible in Hebrew, Aramaic, and Greek (Macula). It also aligns translations to the original-language words so that images, maps, articles, and other resources can be attached to original-language tokens. — [Robie, Balisage 2022](https://www.balisage.net/Proceedings/vol27/html/Robie01/BalisageVol27-Robie01.html)
- **Cost of manual alignment.** Manual tagging and alignment of a Bible "takes about a decade of a scholar's time" per language. This is the motivation for computational alignment. — [SBL abstract](https://sbl-site.org/meetings/abstract.aspx?id=44231)
- **ETEN Innovation Lab QA.** ETEN's quality-assurance recommendations put "the local church … at the center of decision-making, resourcing, and quality assessment processes" and emphasize multimodal processes. The position paper was drafted with feedback from SIL colleagues. — [ETEN: Innovation Lab QA Recommendations](https://etenlab.substack.com/p/innovation-lab-quality-assurance)
- **AQuA and back-translation tooling.** AQuA (Augmented Quality Assessment) is associated with tools that help consultants spot problematic verses using back-translations. Houghton University students built tools to support back-translation checking. — [Houghton University](https://www.houghton.edu/?p=23932) **[indirect; AQuA's method and ownership are unconfirmed]**
- **Human oversight in scholarship.** A 2026 study of LLMs in Bible translation, covering plain-language and literal translation types, concludes that AI can speed up the work, that its limitations differ by translation type, and that human oversight is indispensable. — [AUP Online, KT2026.1.005](https://www.aup-online.com/content/journals/10.5117/KT2026.1.005.AMER)

### Inferences
- The proven pattern is a separate drafting project: AI output goes into its own "SF Drafts" project and is only merged into the main project after humans edit it. OpenLogos should do the same, keeping AI translation output in a separate layer or edition that never mutates source editions.
- SIL conditions its drafts on a trusted "model text" plus pinned glossaries (name pairs and key terms). For an English normalized translation, the equivalent is to anchor on a public-domain base text (BSB or WEB), original-language lemmas and alignments, and a locked key-term glossary, and to have the LLM propose verse-level revisions against them.
- Word alignment is a first-class engine type in Serval. Alignments (source token to target token) are the deterministic backbone that lets reviewers check whether each source word is accounted for.

### Gaps
- I could not verify the details of Paratext 9.x and 10 ("Paratext 10 Studio") AI drafting from release notes. Assume, unverified, that Paratext consumes Scripture Forge and Serval drafts.
- I found no primary source for "BT Synergy". I also found nothing on Biblica's, Wycliffe's, or Seed Company's specific AI tooling in 2025–2026, the Clear Bible "Translators' Assistant", or Global Bible Tools. These remain unresearched.
- I did not confirm whether the "silnlp" research repo still backs Serval's NMT jobs (background knowledge says it does). **[UNVERIFIED]**
- I did not retrieve Macula's exact license. Background knowledge says CC BY 4.0 for Macula Greek and Hebrew. **[UNVERIFIED]**

## What evaluation metrics and methods are used, and what are the known LLM failure modes on biblical text?

### Takeaway
Automatic metrics (chrF/chrF++, BLEU) are standard in Bible MT research, but they miss meaning and theological drift. Practitioners rely on back-translation, team checks, and consultant checks. The documented failure modes are errors with names, numbers, and kinship terms, added or omitted words, inconsistent key terms, fluency winning over fidelity, and defaulting to traditional or theologically loaded readings.

### Cited Findings
- **Common errors in NLLB drafts** (from SIL): names, numbers, kinship terms, wrong word choice, added or missing words, repeated words, grammar, and word order. Novel non-proper-name key terms may be rendered inconsistently across verses. **Strengths:** naturalness, consistent style and key terms, consistency with previous books, and formatting and punctuation. — [SIL deck](https://docs.google.com/presentation/d/1t2R8e5dpjoign9IkKMaFI-T_Wk6NpYZ0/htmlpresent)
- **eBible Corpus.** SIL and partners released a benchmark corpus and model baselines for low-resource Bible translation. — [arXiv 2304.09919](https://arxiv.org/pdf/2304.09919)
- **chrF++ vs BLEU.** An extremely low-resource MT study (2026) compares the two on LLM and NMT output, looking at hallucination, repetition, source copying, and diacritics. It finds that BLEU adds lexical-precision information even though recent work often reports chrF++ alone. — [arXiv 2602.17425](https://arxiv.org/html/2602.17425v1)
- **Lexical metrics miss semantics.** Lexical metrics (BLEU, TER, chrF) lack semantic evaluation for low-resource languages. The proposed alternative is to pivot into a related high-resource language and apply neural metrics. — [Mukherjee et al., COLING 2025 (ACL author page)](https://aclanthology.org/people/ananya-mukherjee/unverified/)
- **Retrieval-augmented LLM refinement.** A hybrid for Dhao used a New Testament-only corpus: fine-tuned NMT drafts, then an LLM with retrieved examples refines them. Performance is "driven primarily by the number of retrieved examples rather than the choice of retrieval algorithm". The study targets the NT-to-OT domain shift. — [arXiv 2601.09982](https://arxiv.org/html/2601.09982v1)
- **Retrieved vs random examples.** In many-shot in-context learning for low-resource MT, BM25-retrieved examples are much more data-efficient: 50 retrieved examples roughly equal 250 random many-shot examples. — [arXiv 2604.02596](https://arxiv.org/html/2604.02596v2)
- **Why LLMs fail at low-resource translation.** Retrieval and understanding are both bottlenecks. — [arXiv 2406.15625](https://arxiv.org/pdf/2406.15625)
- **Bible text is not natural language data.** Translators sometimes choose non-idiomatic wording for theological reasons. — [PrOnto, arXiv 2305.12612](https://arxiv.org/pdf/2305.12612)
- **Theological bias.** ChatGPT, Copilot, and DeepSeek, when "not carefully guided", tend to reproduce traditional readings with conservative theological agendas, especially in controversial passages. — [Pistis & Praxis (PUCPR)](https://periodicos.pucpr.br/pistispraxis/article/download/33897/28040/87653)
- **Fluency over fidelity** (analogous sacred-text study, Qur'an): four AI models favored fluency at the expense of theological accuracy, especially in allegorical verses. BLEU stayed moderate even when the meaning departed from traditional readings. — [bcchj.uobabylon.edu.iq](https://bcchj.uobabylon.edu.iq/index.php/bcchj/en/article/view/4193)

### Inferences
- OpenLogos's evaluation suite should combine:
  - **Automatic scores:** chrF++ and BLEU against reference PD translations, used only as regression signals.
  - **Deterministic structural checks:** verse count and versification parity, numbers and names matching original-language tokens, and key-term glossary consistency (the same lemma rendered the same way unless a reviewer notes otherwise).
  - **Alignment-coverage checks:** every content lemma aligned, and no target words left unaligned (to catch additions).
  - **Back-translation or LLM-judge passes,** treated as advisory only.
  - **Human reviewer sign-off.**
- Harmonization (importing wording from parallel passages, such as the Synoptics or Kings and Chronicles) and versification errors are failure modes the grounding layer can detect deterministically. Diff the proposal's content against the aligned source tokens for that exact verse ID, and check the output's verse IDs against the edition's versification scheme. **[My inference; I found no study that quantifies these for LLMs.]**
- Theological-bias risk argues for showing alternatives with rationale and evidence, rather than a single "correct" rendering, and for logging the steering prompt as provenance.

### Gaps
- I found no peer-reviewed study that specifically quantifies hallucinated or fabricated verses or versification errors by frontier LLMs. The risk is widely discussed but uncited here.
- I could not retrieve ETEN's "Evaluating Bible Translation" post (it returned 404), so I could not confirm the AQuA metrics.

## What patterns exist for agentic research over structured corpora: tool calling, Bible MCP servers, evidence bundles, citation verification, and grounding checks?

### Takeaway
Several community Bible MCP servers already exist: passage retrieval, FTS5 search, Strong's, cross-references, and unfoldingWord Greek and Hebrew data. Anthropic's Citations API offers a strong grounding primitive: when the model cites, the API returns `cited_text` extracted from the supplied documents, with guaranteed-valid pointers. OpenLogos can expose its deterministic search as MCP tools and pass verse text back as citable document or search-result blocks. It should then re-verify each quote against the edition database itself.

### Cited Findings
- **Existing Bible MCP servers** (Glama listings; maintenance status unverified):
  - Hosted, no-key server with 35+ public-domain translations. — [Glama: Bible MCP Server](https://glama.ai/mcp/servers/gkqqsfww4o)
  - GetBible MCP, with streamable HTTP at `https://mcp.getbible.net/v2` and a stdio option. It notes that its software license does not relicense the scripture content. — [Glama: GetBible MCP](https://glama.ai/mcp/servers/cy8kdomsy8)
  - Cloudflare Workers server over API.Bible (needs an API key). — [Glama: sprider/cloudflare-mcp-server-bible](https://glama.ai/mcp/servers/sprider/cloudflare-mcp-server-bible)
  - Offline SQLite FTS5 server with Strong's definitions and cross-references. It claims 1,081 translations. — [Glama listing](https://glama.ai/mcp/servers/uu4vgy5cww)
  - Node-only research server with `word_study`, `compare_passages`, and similar tools. — [Glama: dpirek/bible](https://glama.ai/mcp/servers/dpirek/bible)
  - unfoldingWord-based Greek NT and Hebrew Bible server with morphology (CC BY-SA 4.0). — [Glama listing](https://glama.ai/mcp/servers/inmnryxkkq)
  - A "bible-translation-mcp" server. — [Glama: jeremymikkelsen/bible-translation-mcp](https://glama.ai/mcp/servers/jeremymikkelsen/bible-translation-mcp)
- **Anthropic Citations API.** Documents can be PDFs, plain text (chunked into sentences automatically), or custom content (your own blocks, used as-is with no further chunking). Citations return `char_location`, `page_location`, or content-block index ranges with `cited_text`. Because the API parses and extracts `cited_text`, "citations are guaranteed to contain valid pointers to the provided documents". `cited_text` does not count toward output tokens. — [Claude Docs: Citations](https://platform.claude.com/docs/en/build-with-claude/citations)
- **`search_result` blocks.** These can carry citations for RAG output. Citations are incompatible with structured outputs (`output_config.format`), and the API returns a 400 error if both are used. Source documents can be prompt-cached. — [Claude Docs: Citations](https://platform.claude.com/docs/en/build-with-claude/citations)

### Inferences
- **Recommended tool surface.** A local MCP server, or an in-process tool registry when no MCP host is present, wrapping OpenLogos's deterministic services:
  - `get_passage(edition, ref_range)`
  - `lemma_search(lemma|strongs, morph_filter, corpus)`
  - `get_alignments(ref, edition)`
  - `get_cross_references(ref)`
  - `lexicon_entry(lemma)`
  - `parallel_passages(ref)`
  - `verify_quote(edition, ref, text)`

  Every result should carry stable IDs (edition ID, versification scheme, OSIS or BCV ref, token IDs, and a content hash).
- **Evidence bundle.** An agent answer should be a list of claims. Each claim lists supporting evidence items (tool name, arguments, result hash, edition and ref, quoted span). A post-hoc verifier then re-fetches each quoted span by ref and checks for an exact (normalized) string match. Unverifiable quotes are flagged or removed. Do this even when using the Citations API, because models served locally or over BYOK OpenAI-compatible endpoints lack that guarantee.
- Return verses as custom-content document blocks, one block per verse, so citation indices map 1:1 to verse IDs. This makes grounding checks trivial.
- Because citations and structured outputs are incompatible, use a two-step flow: a cited prose answer first, then a separate structured extraction pass (or a client-side parse) to build the claim and evidence records.
- **Running without AI.** The same deterministic tools power the UI directly. The AI layer is an optional orchestrator over them, which makes it provider-agnostic across subscriptions, BYOK, and local models via MCP or OpenAI-style function calling.

### Gaps
- I did not review the code quality, licenses, or activity of the listed Bible MCP servers.
- I did not research OpenAI's or local-model (Ollama or llama.cpp) tool-calling specifics or MCP spec versions in this session.

## How should a pipeline store AI outputs?

### Takeaway
Practitioner systems keep AI drafts as a separate, editable layer that humans promote. SIL routes drafts into a dedicated "SF Drafts" project rather than the main project. OpenLogos should store every AI artifact as an immutable proposal or claim with full provenance and an explicit review state, and never overwrite source editions.

### Cited Findings
- **SIL's project split.** The SF Drafts project connects to Scripture Forge, syncs with Paratext, and receives drafts. The main Bible project stores the final book and submits draft requests. Drafts are "resources" or comparison material until a human edits them. — [SIL deck](https://docs.google.com/presentation/d/1t2R8e5dpjoign9IkKMaFI-T_Wk6NpYZ0/htmlpresent)
- **Engine state in Serval.** Serval tracks engines, builds, and pretranslations as separate persisted entities (MongoDB, with models in S3), so outputs are tied to a specific build. — [sillsdev/serval](https://github.com/sillsdev/serval)

### Inferences
- **Suggested schema.** `ai_proposal` with these fields:
  - `id`
  - `kind` (translation_rendering | gloss | claim | summary | alignment_suggestion)
  - `target_ref` (edition, ref, token span)
  - `content`
  - `evidence[]` (tool, args, result_hash, cited span)
  - `provenance`: provider, model ID and version, local vs remote, prompt template ID plus a SHA-256 of the full rendered prompt, system and steering-profile ID, tool-call transcript hash, temperature and seed, source edition content hashes, timestamp, app version
  - `status`
  - `supersedes`
  - `reviewer`
  - `review_note`

  Statuses: `proposed → (auto_checks_passed|auto_checks_failed) → under_review → accepted | accepted_with_edits | rejected | needs_consultant`.
- The accepted translation is a derived edition. It is materialized from accepted proposals plus human edits, and every verse points to the proposal IDs it came from. Because it is versioned, it can be rebuilt or reverted.
- Keep the provenance of the inputs used, including the license of each source text fed to the model (see the next section), so the derived edition's licensing obligations can be computed.

### Gaps
- I found no published schema from SIL, unfoldingWord, or ETEN for AI-proposal provenance. The above is a design inference.

## Licensing: which source texts can be LLM inputs or training data for a derived translation?

### Takeaway
The BSB has been public domain since April 30, 2023, and is the cleanest base for a derived, AI-assisted English translation. The only condition is a request not to use the "Berean" name on altered text. The WEB is generally public domain with a name restriction. The NET and LEB are copyrighted with quotation and distribution terms, so they are suitable as reference inputs at most and not as bases for a redistributed derivative. CC BY-SA sources (for example, some unfoldingWord data) would impose share-alike on derivatives.

### Cited Findings
- **BSB.** "The Berean Bible and Majority Bible texts are officially dedicated to the public domain" as of April 30, 2023, and "All uses are freely permitted." The texts can be "freely reproduced, integrated, and adapted for both free and commercial resources". The terms add: "For derivative works that vary from the official text, we respectfully request that the Berean name is not used." AI and training use is not mentioned (it is covered by "all uses"). — [berean.bible/terms.htm](https://berean.bible/terms.htm); [berean.bible/licensing.htm](https://berean.bible/licensing.htm); [eBible BSB copyright](https://ebible.org/engbsb/copyright.htm)
- **NET Bible.** The Scripture text (without notes) may be quoted in non-commercial contexts with a "(NET)" credit and copyright notice. Software apps must hyperlink "NET" to netbible.org. Free copies must be truly free. Commercial publishers must go to HarperCollins Christian Publishing, and other permissions go to Biblical Studies Press. — [netbible.com copyright](https://netbible.com/?p=44) **[search-snippet copies were garbled; verify the exact wording]**
- **LEB.** You may give it away but not sell it on its own. Works that are less than 25% LEB may be sold. Annual reporting applies above certain thresholds (for example, sold works with more than 1,000 verses). Attribution is required: "(LEB)" for quotations under 100 verses, a full credit line beyond that. "Lexham" is a Logos trademark and cannot appear in the name of a larger work. — [Lexham Press LEB License](https://lexhampress.com/LEB-License)
- **unfoldingWord data.** unfoldingWord-derived Greek and Hebrew data is distributed under CC BY-SA 4.0. — [Glama listing](https://glama.ai/mcp/servers/inmnryxkkq)
- **Content vs software licenses.** A software license does not relicense the scripture content it serves. — [Glama: GetBible MCP](https://glama.ai/mcp/servers/cy8kdomsy8)

### Inferences
- **Safe base texts** for a redistributable AI-assisted translation: BSB (PD), WEB (PD, but avoid the WEB name if modified **[UNVERIFIED]**), KJV/ASV and other older PD texts, and PD original-language editions. If the derived text substantially adapts a CC BY-SA source (for example, the ULT/UST or unfoldingWord glosses and alignments), the derivative must be released CC BY-SA with attribution. Using CC BY sources (Macula, if CC BY 4.0 **[UNVERIFIED]**) requires attribution only.
- Using copyrighted translations (NET, LEB, ESV, NIV) as LLM context for a derived text is legally gray. Their terms address quotation and distribution, not training or derivative-generation, and verbatim leakage into output could exceed quotation limits. **[Not legal advice; I found no source addressing AI use specifically.]** A conservative approach:
  - Restrict the "translation lab" inputs to PD, CC BY, and opted-in CC BY-SA sources.
  - Let copyrighted texts appear only as user-licensed reference views, never as model inputs for the derived edition.
  - Store a per-proposal list of input licenses so the share-alike or attribution obligations of the output are computable.
- For the BSB, honor the name request: call the derivative something like "OpenLogos Normalized English", not "Berean".

### Gaps
- Primary-source WEB terms were not retrieved (the ebible.org/web/copyright.htm fetch returned 404). Background knowledge says the WEB is public domain and that "World English Bible" is a trademark that should not be applied to altered text. **[UNVERIFIED]**
- I did not retrieve the exact Macula Greek and Hebrew or Clear alignment licenses.
- I found no authoritative guidance on whether model weights or outputs trained on CC BY-SA text are "adaptations" that trigger share-alike. This is unresolved legally.
- I did not verify current NET terms (post-2019) directly, or whether the NET or LEB terms address AI or ML use.
