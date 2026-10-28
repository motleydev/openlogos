# Blueprint validation

`python3 tools/validate.py` uses Python 3 and `jsonschema` (Draft 2020-12). It has no network or provisioning side effects. The infrastructure agent will pin these dependencies in the target repository's Nix shell.

Validation checks schema shape, role references, dependency cycles, safe paths, budgets, source IDs and synthetic hashes. Negative cases verify rejection of an unknown agent field, task cycle, absent dependency, traversal path unsigned production manifest, exported credential fields, mismatched ink coordinates, fabricated citations and out-of-scope research evidence.

It does not validate the complete intended application protocol. Production schemas must grow alongside implementations, and a runtime importer must enforce all semantic and authority boundaries in `specs/00-import-contract.md`.

Revision 2 validates 12 schema/data pairs and 9 negative cases, including workspace, ink and research context boundaries.

B062 adds the archived 1.0 workspace example as a thirteenth pair and `check_workspace`: every layout profile places every pane exactly once, within its class's visible limit, with widths only for visible panes and collapsed groups that exist. Seven negative cases cover a profile that drops a pane, one that places a pane twice, one that carries another class's layout, an unknown device class, a compact profile with two visible panes, a 1.0 file with profiles and a 1.1 file without them.
