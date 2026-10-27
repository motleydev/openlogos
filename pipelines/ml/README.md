# pipelines/ml

Python experiments only: OCR, alignment, embeddings and evaluation (specs/06-nix-operations.md,
docs/adr/0001-rust-core-native-ui.md). Deterministic ingestion that produces published artifacts is
Rust and lives in `pipelines/build`. `nix develop .#pipeline` provides Python 3 with `jsonschema`;
further pinned dependencies are added with the first experiment.
