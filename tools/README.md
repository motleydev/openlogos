# Blueprint validation

`python3 tools/validate.py` uses Python 3 and `jsonschema` (Draft 2020-12). It has no network or provisioning side effects. The infrastructure agent will pin these dependencies in the target repository's Nix shell.

Validation checks schema shape, role references, dependency cycles, safe paths, budgets, source IDs and synthetic hashes. Negative cases verify rejection of an unknown agent field, task cycle, absent dependency, traversal path unsigned production manifest, exported credential fields, mismatched ink coordinates, fabricated citations and out-of-scope research evidence.

It does not validate the complete intended application protocol. Production schemas must grow alongside implementations, and a runtime importer must enforce all semantic and authority boundaries in `specs/00-import-contract.md`.

Revision 2 validates 12 schema/data pairs and 9 negative cases, including workspace, ink and research context boundaries.
