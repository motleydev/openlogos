# Accuracy, rights and governance

## Accuracy charter

Canonical source text is preserved, every enrichment has provenance, and uncertainty is a supported result. The product distinguishes sourced fact, scholarly analysis, editorial theology and machine proposal. A reader can inspect evidence, report errors, disable an enrichment layer and roll back a package without losing notes.

Accuracy includes correct attribution and edition identity, not just spelling. A cross reference can be traditional, topical, lexical, explicit quotation or suggested allusion; its category and source appear in the UI. Theme vocabularies are versioned and attributed. Conflicting Christian/Jewish scholarly traditions are not flattened into a supposedly neutral ontology.

## Release gates

| Gate | Required evidence | Decision owner |
|---|---|---|
| Reader | Offline/selection/a11y/performance checks; no lost annotations | Product owner with QA |
| Corpus | Source hashes, rights record, parse fixtures, no known source corruption | Human source steward |
| Scholarly overlay | Evidence, adjudicated samples, confidence/coverage report | Qualified human reviewer |
| Private sync / P2P | Threat-model review, protocol tests, recovery tests | Human security maintainer |
| Translation | Source fidelity and editorial review, license clearance | Qualified language/editorial panel |
| Public release | Build provenance, dependency licenses, backups, changelog | Human project maintainer |

Agents prepare evidence; they cannot impersonate these humans. A passing automated test does not substitute for language competence or rights adjudication. Qualified volunteers are viable; availability is a capacity constraint.

## Rights registry

Per artifact and derived layer record: work/edition/translator/editor, publication date, source URL and hash, retrieval date, exact license document/version, holder, public-domain reasoning and jurisdictions, attribution text, modifications, and explicit allowed operations (local use, redistribution, transformation, public indexing, embedding, model prompting/training and translation evaluation).

Allow, deny and unknown are distinct states. The software policy is deliberately conservative when evidence is incomplete; it is not an automated worldwide legal opinion. Surface territory-limited claims and choose a distribution policy with a qualified reviewer before public release. Maintain separate policies for private import and official redistributable packages. Sharing never silently overrides rights because a work is old or a user bought it.

Historical KJV/Luther/ASV editions, lexicons such as Strong's/BDB/Thayer, church fathers, Josephus and other ancient works require edition-level checks. Modern scans, fonts, editorial notes and apparatus may carry additional rights. Contemporary lexicons and critical editions are not assumed open. Check database and platform terms as well as work-level copyright where applicable.

No blanket 'all CCEL is public domain', 'all CrossWire is freely redistributable', or 'all biblical data is unprotected' rule. Exact source scope determines policy. ESV standard quotation allowances are not a blanket permission for a Bible-reference application or AI translation.

## Clean implementation and reuse

Maintain an inspiration log: observed workflow, public source URL, desired behavior and independently designed implementation. Source reuse records package license, version, copied files and obligations. Logos interoperability is a future separately reviewed scope, not an MVP dependency. Public-domain texts available from independent sources need no proprietary library extraction.

The product charter prohibits paid feature gates, ads and vendor account requirements in the maintained project. An open-source license may still allow commercial forks; do not claim it prevents them. Funding can come from voluntary donations and optional hosting while the full self-host path remains documented.

Contributors use a DCO or similarly clear contribution process; choose governance and code/content licenses before first public code release. Publish a security policy, correction policy, decision log and release ownership. Protect review/signing keys. External contributions cannot auto-promote themselves into trusted corpora.

## Test hierarchy

Unit/property tests cover reference mapping, Unicode offsets, package parsing and sync convergence. Golden fixtures verify source fidelity. Integration tests cover offline flows, migration/restore and isolated deployment. Device tests cover selection/RTL/accessibility/background interruptions. Corpus evaluations cover citation precision, OCR and linguistic decisions. Security tests cover untrusted artifacts, auth boundaries and transport failures.

Tests include negative cases: no alignment, unknown license, unavailable source, contested morphology, contradictory witnesses, orphaned annotation and malformed package. Do not make the UI look complete by filling missing evidence with generated content.

## Revision 2 release gates

Three-pane workspace tests are part of the reader gate. Obsidian/vault release requires conflict, rename, offline embed and non-destructive file tests. Ink release requires coordinate stability across devices, acknowledged-stroke recovery and editable export. Research release requires grounded citation tests, context isolation, provider-scope/terms validation and human security review of companion credentials/tool access. Provider account permissions do not authorize corpus use beyond its rights record. Research output can become a personal note after user acceptance, but cannot self-publish into scholarly overlays.

## Audience scope and handoff gates

The intended audience shapes resource acquisition and workflow priorities. Product stewardship does not ratify an interpretive policy or confession for users. Source/language reviewers assess attribution, textual and linguistic evidence, and rights under the existing release gates. Collection publication does not mean endorsement of its theology. Model output cannot replace required human review, and audience preferences never alter exact-search counts or manufacture source evidence.

The reader gate now includes deterministic search, a declared attributable cross-reference layer, source collections and the bounded Obsidian handoff. B060 tests real vault behavior and protection of external writing. Broader bidirectional editing remains separately gated by B045. The audit records design proposals and missing implementations explicitly; no screenshot-based performance, accessibility or scholarly accuracy certification is claimed.
