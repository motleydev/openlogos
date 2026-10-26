# Decisions the scaffold must resolve with evidence

| Decision | Proposed default | Evidence needed / owner |
|---|---|---|
| Rendering stack | React Native mobile + DOM web | Native arbitrary selection, RTL and accessibility spike; client/design |
| Second Bible / first ancient edition | Small approved artifact set | Exact edition/license/hash; source steward and human reviewer |
| App/content licenses | MPL client + AGPL services, separate data | Owner choice and upstream/app-store compatibility review |
| Models and budget | Capability tiers; two coding runs | Installed adapter/model probe and 24-task quality/cost benchmark |
| Storage on Safari | Tested OPFS VFS or IndexedDB fallback | Quota/eviction/auth header behavior; client/QA |
| Crypto library/protocol | Existing reviewed implementation | Platform support, maintenance and independent security assessment |
| Early-translation corpus | Individually cleared editions | Language competence, provenance and rights, before translation pilot |
| Corpus distribution territories | Explicit declared policy | Qualified rights review; never infer global PD from US status |
| Hosting size and inference spend | Local/minimal deployment | Measured build/runtime/OCR/pilot usage; infrastructure |

These do not require stopping all scaffolding. Independent schema, UX and environment work can proceed with synthetic fixtures. A decision blocks only the work that depends on it.

## Human capacities required

The project owner chooses product scope and public licenses. A source steward makes rights decisions with appropriate advice. Qualified Hebrew/Aramaic and Greek reviewers adjudicate linguistic/translation claims; early-version arguments may need additional language expertise. A security reviewer evaluates crypto, recovery and sensitive network flows. AI role names in agents.json do not imply these people exist or can be replaced.

## Revision 2 decisions

- Three-pane navigation/link groups and responsive layouts: first-class requirement; validate readable minimum sizes and note-dock interaction.
- Obsidian: select exact vault access capabilities per OS and implement real block materialization; direct filesystem parity is not assumed.
- Ink: compare perfect-freehand plus a small editor with Excalidraw and optional native adapters; retain neutral editable data.
- Subscription inference: test the documented OpenAI OSS route first; resolve the Claude CLI/SDK documentation discrepancy for the chosen integration mode. No token extraction or unofficial endpoint adapter.
- Companion trust: separate ciphertext-only sync from user-approved plaintext inference; verify multi-user credential isolation and host deployment restrictions.

## Revision 3 decisions and remaining blockers

- Product direction is decided: Reformed study and resource collection, drafting elsewhere, Obsidian as the sole first-class writing integration. A particular confession and disputed denominational positions have not been chosen.
- B055 names the human editorial maintainer and ratifies the detailed charter. Technical scaffolding can use synthetic fixtures before content approval.
- B004 compares compact UI alternatives, including no permanent rail, shared controls and transient drawers. No final visual identity or measured usability is asserted.
- B042/B060 establish the actual desktop/mobile Obsidian transport and preservation behavior. File transfer is acceptable when direct access is unavailable; an untested universal vault bridge is not.
- First-party licensing remains unresolved. This public specification repository does not yet contain an adopted LICENSE. B037 must settle the code/documentation terms before reuse/public code release; public visibility alone is not an open-source license.
- A reviewed cross-reference artifact, original-language alignment coverage, and confessional edition permissions remain acquisition/review work. No copied screenshot content supplies these datasets.
