# AI-assisted translation research track

## Goal and boundary

Produce an openly inspectable experimental translation with traceable choices, never an automatic replacement for reviewed source editions. It is an opt-in library work carrying explicit machine/human review status per passage. The reader's usefulness and release schedule do not depend on this research succeeding.

The source languages include Biblical Hebrew, Biblical Aramaic and Koine Greek. No surviving complete autograph is available to feed a model. Select a documented base edition for each book and preserve its textual decisions. Variants remain evidence with explicit editorial policy; do not average different editions into an undocumented text.

## Source policy

Initial candidates: OSHB/WLC for Hebrew/Aramaic and a pinned SBLGNT edition for Greek, with STEP linguistic data after per-file review. Alternatives such as UHB/UGNT have their own textual and license implications. Every translation draft names exact source editions and revisions.

Early translations (specific Septuagint, Old Latin/Vulgate, Syriac/Peshitta and later other witnesses) can be major interpretive/text-critical evidence. Their age alone does not make them superior witnesses to every passage. Distinguish original ancient translation, modern critical edition and modern English rendering. Do not simply back-translate an English Septuagint or Syriac rendering and call it original-language analysis. Add a witness only when a reliable edition, rights evidence and competent reviewer are available. The initial witness acquisition task remains open; no freely reusable complete apparatus is assumed.

Modern open translations can provide a separate readability/comparison benchmark. World English Bible is an initial candidate [S04]. Do not optimize for closeness to one modern translation. The grammar and semantic coverage of the source, transparent textual policy, and comprehensible English are the primary objectives.

**ESV and similar proprietary translations:** standard permissions/API terms do not establish permission for this project's full-corpus storage, redistribution, model training or derivative translation workflow [S02,S03]. No ESV text in prompts, training, fine-tuning sets, embeddings, published datasets, generated benchmarks or distributed packages by default. If written permission expressly covers a comparison use, create a restricted evaluation adapter and policy for that purpose only. An API key or user-owned book is not the permission record. Human reviewers may use lawfully obtained resources within applicable permissions, without copying restricted material into project outputs.

The ESV can inform a proposed high-level goal such as contemporary, readable English without providing wording to the generator. Permissioned comparative evaluation should happen after an independently drafted translation, with safeguards against copying. A modern control must not suppress genuine source ambiguity.

## Workflow per passage

1. Freeze the source edition, token sequence, morphology alternatives, relevant context and approved witness evidence. Hash the input bundle.
2. Draft a literal semantic coverage analysis: predicates, participants, negation, tense/aspect, discourse connections and uncertainty. Separate lexical evidence from theological interpretation.
3. Produce an English candidate with token/phrase alignments, explanatory rationale, supplied-word flags and alternative renderings. Output a structured record; no hidden unsupported expansions.
4. Run a separately configured critic to find omissions, additions, number/person errors, improper harmonization, lost ambiguity and stylistic problems. Model disagreement becomes review work; model agreement is not validation.
5. Review source fidelity by a qualified Hebrew/Aramaic or Greek reader; consult relevant early-language expertise for substantive witness arguments. A separate English editor assesses readability.
6. Adjudicate disagreements, record names/roles and citations, and keep rejected alternatives. Significant textual or doctrinal ambiguity requires a second qualified human review before a 'reviewed' label.
7. Publish a versioned research release with draft/partial/reviewed status, change notes, base-text policy, evaluation results and limitations. Human approval is passage-specific, not inferred from a book-level toggle.

Do not generate or store purported hidden chain-of-thought. Store concise translation rationales, evidence and decision records suitable for public audit.

## Pilot and evaluation

Start with approximately 100–150 verses sampled across narrative, poetry, law, prophecy, discourse and epistle, including Aramaic and disputed/ambiguous passages. Selection and evaluation sets are fixed before tuning; qualified reviewers establish rubrics without seeing model/provider names. A smaller 20-passage technical spike precedes this pilot.

Score fidelity (omissions/additions/negation/participants), lexical sense, syntax/discourse, handling of variants, preserved ambiguity, idiomatic English and consistency. Report severe-error counts separately from averages. A critical meaning error blocks passage publication as reviewed. Inter-reviewer disagreement and abstention remain visible. Back-translation and automatic similarity metrics are secondary diagnostics only.

A new model or prompt is evaluated against the same held-out set, plus adversarial cases, before broader generation. Training-data contamination cannot be disproved just because prompts omit modern Bibles; disclose it and inspect suspicious wording overlap within available rights. No automatic plagiarism percentage proves independence.

## Output and rights

Each draft records source hashes, model/provider or local weights and license, prompt/config version, generation date, context window, alignment, rationale and review events. Provider runs may not be exactly reproducible; preserve artifacts and parameters. A local model option must also have verified weight/output-use terms; open weights are not automatically open source.

Determine the release license after reviewing the base-text/annotation dependencies, share-alike obligations and rights in human editing. Do not assert exclusive copyright in unedited AI output or promise global public-domain status. Keep reusable source layers distinct; license reviewed human contributions explicitly and preserve upstream attribution.

The project may ultimately publish a useful draft, a reviewed translation, or only research tooling. None is represented as divinely authoritative, unbiased, or more accurate merely because it was generated from original-language inputs.

## Editorial frame and source fidelity

The product's Reformed editorial identity is explicit, while source-language fidelity remains the translation evaluation criterion. A generated reading cannot earn a higher fidelity score simply because it supports a preferred doctrine. Confessional arguments belong in attributed commentary and review notes; they cannot silently add words, erase ambiguity or override the recorded textual evidence. Include deliberate doctrinal-pressure cases in the blind human review.
