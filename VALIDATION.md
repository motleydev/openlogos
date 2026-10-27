# Validation report — revision 4

Date: 26 October 2026. Passed using Python 3.12 and jsonschema 4.25.1.

- Twelve JSON Schema/data validations.
- Fourteen unique agent roles; sixty tasks with valid owners and no dependency cycles.
- Role reviewer references, source IDs, safe paths and budget weights totaling 100.
- Synthetic package and snapshot asset hashes verified.
- Three-pane workspace fixture, ink coordinate association and research evidence scope validated.
- Nine negative tests reject unknown agent fields, cycles, missing dependencies, traversal paths, unsigned production manifests, secret fields in exported threads, mismatched ink coordinates, fabricated evidence IDs and unselected source evidence.

These are project-authored specification schemas, not the installed Paperclip API schema. All agents remain paused and model bindings/budgets unresolved. Provider documentation research is not evidence of a working integration or a legal determination.

Not implemented or tested: the app, Nix flake/modules, native rendering, handwriting engines, Obsidian plugin, production importer, provider sign-in/inference, transfer cryptography, identity/sync, corpus acquisition/rights clearance, model benchmarks or Paperclip deployment. The backlog scopes those tasks.

All example content is synthetic. No Bible corpus, provider credentials or private notes are included.

Revision 3 adds a screenshot/design audit, audience-driven resource scope, collection/handoff requirements and corresponding task dependencies. The existing validator checks their task/spec/source references and DAG. Collection/export entities remain prose contracts, not implemented protocols or additional validated JSON schemas. The audit is not a runtime, security, theological or accessibility certification.

Revision 4 removes the proposed theological persona/editorial-profile contract and doctrinal ratification gate. Source selection, the actual user question and evidence govern research. This is a specification correction, not a tested model-behavior change.

Revision 5 (27 October 2026): validation passes with Python 3 and jsonschema provided by `nix-shell -p "python3.withPackages(ps:[ps.jsonschema])"`: 12 schema/example validations, 14 roles, 68 acyclic tasks, 71 sources, 9 negative tests. The stack, platform and retrieval changes are specification decisions. No engine, binding, renderer, embedding model, benchmark or build host has been implemented or measured; B005, B061, B064, B066 and B068 carry that evidence. The graph-engine comparison in specs/19 separates verified platform support from expected support. The research report is a desk synthesis of public sources with stated gaps, not a benchmark or legal clearance.
