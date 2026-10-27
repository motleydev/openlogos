# Vector Embeddings, Hybrid Retrieval and On-Device Inference for RAG over Biblical and Ancient-Language Texts

Scope note: research done 2026-10-27, about 17 tool calls. License and parameter metadata come from the Hugging Face Hub API (repo metadata), which is more reliable than third-party directories. Anything drawn from the researcher's background knowledge rather than a source fetched in this session is marked **[unverified]**.

## 1. Which embedding models handle Koine Greek, Biblical Hebrew, English and German? Licenses and evaluations

### Takeaway
No public benchmark evaluates general multilingual embedders (e5, BGE-M3, Qwen3, EmbeddingGemma, and the rest) on Ancient/Koine Greek or Biblical Hebrew retrieval. None of these models' card language lists includes "grc" or "hbo" either: they list modern Greek ("el") and modern Hebrew ("he"). The license-clean, multilingual, on-device-sized shortlist is Qwen3-Embedding-0.6B (Apache-2.0), BGE-M3 (MIT), multilingual-e5 (MIT), Snowflake Arctic-embed-l-v2.0 (Apache-2.0), nomic-embed-text-v2-moe (Apache-2.0) and EmbeddingGemma-300m (Gemma license, gated). Avoid Jina v3 (CC-BY-NC-4.0). Ancient-language performance has to be measured in-house. The only ancient-domain retrieval evidence found shows that small adapted encoders (Logion/GreBerta-based, or distilled Greek models) beat generic multilingual encoders on patristic material.

### Cited Findings
**General multilingual models (licenses and specs from HF Hub metadata):**
- BAAI/bge-m3: XLM-RoBERTa architecture, MIT license, ONNX weights available on the Hub — [HF bge-m3](https://hf.co/BAAI/bge-m3). It does dense, sparse (lexical weights) and multi-vector (ColBERT-style) retrieval in one model, with 8192-token inputs, 568M params and 1024-dim dense output — [NVIDIA model card](https://build.nvidia.com/baai/bge-m3/modelcard); [ExecuTorch port, one-pass dense+sparse+multivector](https://huggingface.co/mlboydaisuke/bge-m3-ExecuTorch).
- jinaai/jina-embeddings-v3: 572.3M params, **license cc-by-nc-4.0** (non-commercial). Its language tags include el, he, la and sa — [HF jina-embeddings-v3](https://hf.co/jinaai/jina-embeddings-v3). One third-party directory claims Apache-2.0, which conflicts with the HF metadata — [thegtmdirectory](https://thegtmdirectory.com/models/jinaai-jina-embeddings/jina-embeddings-v3).
- jinaai/jina-embeddings-v4: 3.75B params, built on Qwen2.5-VL-3B-Instruct, 30+ languages, 2048-dim single-vector (Matryoshka down to 128) plus 128-dim multi-vector, 32k context, GGUF available. The README excerpt read in this session states no license, and the HF metadata lists none — [HF jina-embeddings-v4](https://hf.co/jinaai/jina-embeddings-v4). **[unverified]**: the researcher recalls v4 also being CC-BY-NC/Qwen-research-licensed, so it should be treated as non-commercial until checked. At 3.8B it is too large for phones anyway.
- nomic-ai/nomic-embed-text-v2-moe: 475.3M params (MoE), Apache-2.0. Language tags include el, he and la — [HF nomic-embed-text-v2-moe](https://hf.co/nomic-ai/nomic-embed-text-v2-moe).
- Qwen/Qwen3-Embedding-0.6B: 595.8M params, Apache-2.0, based on Qwen3-0.6B-Base — [HF](https://hf.co/Qwen/Qwen3-Embedding-0.6B); [arXiv 2506.05176](https://arxiv.org/abs/2506.05176v3). Reported MMTEB (multilingual) score 64.33 and MTEB English v2 score 70.70 — [Modal library page](https://modal.com/library/qwen/qwen3-embedding-0-6b) (secondary source, consistent with the [Accenture catalog](https://sdk.airefinery.accenture.com/distiller/model_catalog/Embedding/Qwen/Qwen3-Embedding-0.6B/)). 4B and 8B siblings exist **[unverified in this session]**.
- google/embeddinggemma-300m: 302.9M params, `license:gemma`, gated — [HF](https://hf.co/google/embeddinggemma-300m). Trained on 100+ languages, 768-dim output with Matryoshka truncation to 512/256/128, 2048-token context — [Google AI docs](https://ai.google.dev/gemma/docs/embeddinggemma?hl=en). Google reports under 15 ms for 256 tokens on EdgeTPU and under 200 MB RAM with QAT quantization; no independent benchmark was found — [Google Developers Blog](https://developers.googleblog.com/introducing-embeddinggemma/). Paper: arXiv 2509.20354 (from HF tags).
- Snowflake/snowflake-arctic-embed-l-v2.0: 567.8M params, XLM-RoBERTa, Apache-2.0, ONNX and transformers.js tags. Languages include el and he — [HF](https://hf.co/Snowflake/snowflake-arctic-embed-l-v2.0).
- intfloat/multilingual-e5-large-instruct: 559.9M params, MIT, ONNX available. Languages include el, he, la and sa — [HF](https://hf.co/intfloat/multilingual-e5-large-instruct). The small (~118M, 384-dim) and base variants are also MIT **[unverified in this session]**.
- Rerankers: BAAI/bge-reranker-v2-m3 has 567.8M params and Apache-2.0 — [HF](https://hf.co/BAAI/bge-reranker-v2-m3). Qwen/Qwen3-Reranker-0.6B has 595.8M params and Apache-2.0 — [HF](https://hf.co/Qwen/Qwen3-Reranker-0.6B).

**Ancient-language models:**
- GreBerta/GreTa (monolingual Ancient Greek, RoBERTa-base and T5) and PhilBerta/PhilTa (trilingual Greek, Latin and English), from Riemenschneider & Frank, ACL 2023. Reported as SoTA on Ancient Greek tagging and parsing (GreBerta on Perseus: 95.83 XPoS, 88.20 UAS). These are token-level models, not sentence-embedding models — [arXiv 2305.13698](https://arxiv.org/pdf/2305.13698); [HF bowphs/GreBerta](https://huggingface.co/bowphs/GreBerta).
- Krahn, Tate & Lamicela (2023) distill multilingual sentence-embedding models into Ancient Greek BERTs using Greek–English aligned parallel sentences. They evaluate translation search, semantic similarity and semantic retrieval, and investigate translation bias. Datasets are at github.com/kevinkrahn/ancient-greek-datasets — [arXiv 2308.13116](https://arxiv.org/abs/2308.13116). A related Hub model, `kevinkrahn/shlm-grc-en` (sentence-similarity, grc–en), exists — [HF](https://hf.co/kevinkrahn/shlm-grc-en). Its license and benchmark numbers were not checked.
- de la Selle et al. (arXiv, 27 July 2026), "From transcription to semantic corpus analysis": they adapt a specialized token-level Greek/Latin LM into sentence encoders via TSDAE and contrastive CSE. The gold set is 2,935 expert-verified parallels of biblical reuse in patristic literature (Augustine, Jerome, Athanasius; Latin and Greek). The adapted encoders beat generic multilingual encoders, distilled models and supervised baselines on both detection and correspondence retrieval. CSE peaks with only about 4–8k in-domain sentences (tens of seconds on a laptop GPU), and the pipeline is released as the "Paraphrasis" tool — [arXiv 2607.24542](https://arxiv.org/abs/2607.24542). A search snippet says Logion and GreBerta were the base models, with Logion-CSE trained on 6,000 Athanasian sentences, but the abstract page did not confirm the model names — [arXiv PDF](https://arxiv.org/pdf/2607.24542).
- BEREL (Shmidman et al. 2022) is a BERT for **Rabbinic** Hebrew, released "for unrestricted use". Its authors note that HeBERT and AlephBERT are modern-Hebrew models — [BEREL paper](https://astro.paperswithcode.com/paper/introducing-berel-bert-embeddings-for). DictaBERT and NeoDictaBERT are modern-Hebrew models. NeoDictaBERT improves Hebrew retrieval NDCG@20 from 0.397 to 0.404 when fine-tuned only on English — [arXiv 2510.20386](https://arxiv.org/pdf/2510.20386).
- A Hub search for "biblical hebrew" models returned nothing. "koine greek" returned only `ABeZet/Koine-Greek-BERT` (69 downloads, unvetted) — [HF](https://hf.co/ABeZet/Koine-Greek-BERT).
- SIGTYP 2024 shared task: word embeddings for ancient languages, including grc and hbo, evaluated on POS, lemmatization and morphology rather than retrieval. The organizers say a universal benchmark for ancient-language embeddings is needed — [ACL Anthology](https://aclanthology.org/2024.sigtyp-1.19.pdf).
- On a modern-Greek retrieval benchmark (104 queries), multilingual embeddings beat Greek-specific models and hybrid retrieval was best overall — [arXiv 2607.21274](https://arxiv.org/html/2607.21274v1). This is suggestive, but it covers modern Greek, not Koine.

### Inferences
- The modern-language part of the corpus (English and German translations, commentaries, lexicon definitions) is well served by any of the license-clean multilingual models. Qwen3-Embedding-0.6B and EmbeddingGemma-300m are the strongest small candidates. BGE-M3 is attractive because one model provides dense, sparse and multi-vector output.
- For Koine Greek and Hebrew, generic models will tokenize polytonic Greek and pointed Hebrew (niqqud and cantillation) poorly, and they saw little ancient text in training. Normalize before embedding: strip accents and breathings, strip vowel points, or use lemma sequences. Then measure.
- A small fine-tuned ancient encoder is a realistic option on the evidence of 2607.24542: CSE on about 5k sentences, starting from GreBerta/PhilBerta or a distilled Krahn model. A Hebrew equivalent from BEREL would need verifying. It would be a second, optional index rather than the main one.
- Gemma license terms are acceptable for many apps but are not OSI-open, and the weights are gated. Bundling them in an open-source app needs a license review.

### Gaps
- No MTEB/MMTEB task for Ancient Greek or Biblical Hebrew was found. The MTEB leaderboard was not queried directly.
- The BEREL license, the Logion license and availability, and the Krahn model licenses and numeric results were not verified.
- The jina-embeddings-v4 license was not confirmed.

## 2. Embed original-language text directly, or rely on aligned translations and glosses?

### Takeaway
Do both, but anchor semantics in the translation. The primary semantic index should cover verse-aligned English and German. Original-language passages then inherit hits through verse alignment, which is exact and free. Original-language text should also be embedded, as normalized surface or lemma text, for Greek-to-Greek and Hebrew-to-Hebrew similarity (for example, allusion and reuse detection), but its quality should not be assumed.

### Cited Findings
- Krahn et al. explicitly investigate "translation bias" in cross-lingual Ancient Greek embeddings trained on English-aligned parallel data. Distillation from an English teacher makes Greek vectors land near their translations — [arXiv 2308.13116](https://arxiv.org/abs/2308.13116).
- Biblical-reuse detection in patristic Greek and Latin is best done with in-domain adapted encoders, not generic ones — [arXiv 2607.24542](https://arxiv.org/abs/2607.24542).
- NeoDictaBERT shows cross-lingual transfer from English-only fine-tuning to Hebrew retrieval, but it is a modern-Hebrew model and the gain is small (0.397 to 0.404 NDCG@20) — [arXiv 2510.20386](https://arxiv.org/pdf/2510.20386).

### Inferences
- Scripture is already verse-aligned across translations (versification mapping is handled in other workstreams), so a query in English can find a verse in English and then show Greek or Hebrew via the alignment. That is more reliable than a cross-lingual vector match.
- Lexicon entries should be embedded through their English or German glosses and definitions, with the headword lemma kept as a sparse/BM25 key. Lemma and Strong's IDs should be exact-match filters, not vectors.
- Multiple translations of the same verse can be embedded and pooled (mean or max of scores). This reduces any single translation's phrasing bias and is a cheap gain. **[unverified: no source measured this for Bible text]**
- Concept relations across texts, such as an LXX word reused in NT, should come from lemma and morphology data (exact lexical links). Embeddings should only suggest candidates.

### Gaps
- No study comparing "embed original" vs "embed translation" retrieval quality for Bible verses was found.

## 3. Chunking for scripture and commentary; preserving citation anchors

### Takeaway
Use verse-level units as the atomic, citable record. Retrieve over a pericope or window-level "parent" (about 3–8 verses or the section heading's span) for semantic recall, then resolve to verse anchors. For commentaries, chunk by paragraph and attach the verse references the paragraph is about as structured metadata. Every chunk carries stable IDs (work, edition, canonical verse range, char offsets) so the assistant cites anchors, never free text.

### Cited Findings
- No source specific to scripture chunking was retrieved in this session. The recommendations below are inferences from general RAG practice **[unverified]**.
- EmbeddingGemma has a 2048-token context, and BGE-M3 has 8192, so pericope-sized chunks fit easily — [Google AI docs](https://ai.google.dev/gemma/docs/embeddinggemma?hl=en); [NVIDIA bge-m3 card](https://build.nvidia.com/baai/bge-m3/modelcard).
- jina-embeddings-v4 Spaces demonstrate "late chunking", which embeds a long passage and pools per sub-span — [HF jina-embeddings-v4](https://hf.co/jinaai/jina-embeddings-v4).

### Inferences
- Single verses are often too short and context-dependent ("And he said unto them…") for good dense matching. Pericopes match topics better but are poorly citable. Parent-child retrieval solves this: embed (a) each verse with a small context prefix (book, chapter, section heading) and (b) each pericope or 5-verse sliding window with stride 2–3. Score both and map window hits back to their best-scoring child verse.
- Pericope boundaries should come from a fixed, licensed segmentation dataset so IDs are deterministic across packages, not from per-translation headings.
- Commentary: keep one paragraph per chunk, with a heading-path prefix ("Calvin, Comm. on Romans, 3:21–26"). Store `refs[]` parsed from the text so a commentary chunk can be retrieved by passage as well as by meaning.
- Lexicon: one chunk per sense or definition, keyed by lemma and sense ID.
- Citation rule for the research assistant: the LLM only receives chunks with IDs and may only cite those IDs. Rendered citations come from the database, not from model output.

### Gaps
- No empirical comparison of verse, pericope and window chunking on Bible retrieval was found.

## 4. Hybrid retrieval: BM25 + dense + RRF, learned sparse, rerankers, on-device feasibility

### Takeaway
Hybrid retrieval should be the default. Combine BM25/FTS (always available, works with exact names and lemmas) with a dense index, using reciprocal rank fusion. On a modern-Greek benchmark, hybrid beat both BM25 and dense alone. BGE-M3 can supply dense and learned-sparse scores in one pass. A cross-encoder reranker (bge-reranker-v2-m3 or Qwen3-Reranker-0.6B, both Apache-2.0, ~0.57–0.6B params) is feasible on Mac and iPad and borderline on phones for top-20 to top-50 reranking. Make it optional, or server-side when a server exists.

### Cited Findings
- On the Greek CUP benchmark, hybrid retrieval performed best overall. BM25 scored nDCG 0.544–0.652 against up to 0.748 for the best dense setup — [arXiv 2607.21274](https://arxiv.org/html/2607.21274v1).
- BGE-M3 produces dense, sparse and multi-vector representations simultaneously. The sparse head outputs lexical term weights — [NVIDIA model card](https://build.nvidia.com/baai/bge-m3/modelcard); [ExecuTorch port](https://huggingface.co/mlboydaisuke/bge-m3-ExecuTorch).
- The Ancient Graffiti semantic search (Greek and Latin) combined embeddings, keyword matching and LLM reranking. Its evaluation is preliminary — [ACH Anthology vol. 3](https://anthology.ach.org/volumes/vol0003/semantic-search-for-ancient-inscriptions/).
- Reranker licenses and sizes: bge-reranker-v2-m3 has 568M params and Apache-2.0. Qwen3-Reranker-0.6B has 596M params and Apache-2.0 — [HF](https://hf.co/BAAI/bge-reranker-v2-m3); [HF](https://hf.co/Qwen/Qwen3-Reranker-0.6B).

### Inferences
- RRF (score = Σ 1/(k + rank), k≈60) needs no score calibration, which is ideal for fusing SQLite FTS5 BM25 with vector kNN on device. The k=60 convention comes from Cormack et al. 2009 **[unverified, not fetched]**.
- For ancient-language text, BM25 over normalized lemma tokens usually matters more than dense retrieval. Run it over lemma, surface and Strong's fields.
- Learned sparse (BGE-M3 sparse, SPLADE) adds little on top of BM25 if the dense model is BGE-M3 anyway. If another dense model is chosen, skip learned sparse on device.
- Reranker cost: a 0.6B cross-encoder scoring 30 pairs of about 200 tokens is roughly 6k tokens of forward passes. That is likely sub-second on M-series hardware and 1–3 s on recent iPhones with int8 **[unverified estimate; no on-device reranker benchmark found]**. Quantized cross-encoders are about 300–600 MB, a large optional download.
- Alternative: use the Apple Foundation Model or another on-device LLM to judge relevance only for the final top-5 in the assistant flow, not in search.

### Gaps
- No measured on-device latency for bge-reranker-v2-m3 or Qwen3-Reranker on iPhone or Android was found. The Jina reranker license was not checked.

## 5. On-device inference: Core ML, Apple Foundation Models, MLX, ONNX Runtime, llama.cpp, candle, ort, WASM/WebGPU

### Takeaway
The most portable path for a Rust core is ONNX models run via the `ort` crate on iOS, macOS and Android, with candle as a pure-Rust fallback and transformers.js / ONNX Runtime Web (WebGPU, else WASM q8) on the web, all using the **same** exported model so query vectors match the shipped corpus vectors. Apple's Foundation Models framework (iOS 26) provides an on-device ~3B LLM for generation and grounding on eligible devices. No embeddings API from it was found, so retrieval must not depend on it.

### Cited Findings
- `ort` is a Rust wrapper for ONNX Runtime (current release wraps ORT 1.28). Execution providers such as CoreML must be enabled via Cargo features. Prebuilt downloads only cover a few EPs (CUDA, TensorRT), so CoreML or NNAPI may need custom-built ORT binaries. If an EP is unavailable, ORT falls back to CPU — [docs.rs ort](https://docs.rs/crate/ort/latest).
- The ONNX Runtime CoreML EP requires iOS 13+ or macOS 10.15+, and prebuilt iOS binaries with CoreML ship via CocoaPods — [ONNX Runtime CoreML EP docs](https://onnxruntime.ai/docs/execution-providers/CoreML-ExecutionProvider.html).
- Transformers.js v3 runs on WebGPU via ONNX Runtime Web (`device: 'webgpu'`). Default dtypes are fp32 on WebGPU and q8 on WASM, with fp16 and q4 available. Global WebGPU support was about 70% in October 2024 — [HF blog transformers.js v3](https://www.huggingface.co/blog/transformersjs-v3); [WebGPU guide](https://huggingface.co/docs/transformers.js/en/guides/webgpu).
- Snowflake arctic-embed-l-v2.0 is tagged `onnx` and `transformers.js`. bge-m3 and multilingual-e5-large-instruct are tagged `onnx`. Jina v4 has GGUF builds — [HF arctic](https://hf.co/Snowflake/snowflake-arctic-embed-l-v2.0); [HF bge-m3](https://hf.co/BAAI/bge-m3); [HF e5-large-instruct](https://hf.co/intfloat/multilingual-e5-large-instruct); [HF jina v4](https://hf.co/jinaai/jina-embeddings-v4).
- EmbeddingGemma was designed for on-device use: under 200 MB RAM with QAT and under 15 ms for 256 tokens on EdgeTPU (Google's claim) — [Google Developers Blog](https://developers.googleblog.com/introducing-embeddinggemma/). A secondary source says about 300 MB RAM, which conflicts — [36kr](https://eu.36kr.com/en/p/3453305267820162).
- BGE-M3 has an ExecuTorch port that outputs dense, sparse and multi-vector in one pass, which suggests mobile deployment is practical — [HF](https://huggingface.co/mlboydaisuke/bge-m3-ExecuTorch).
- The Apple Foundation Models framework (iOS 26) exposes the ~3B on-device Apple Intelligence LLM, works offline, and costs nothing per inference. It requires Apple Intelligence-capable hardware (iPhone 15 Pro and later, M-series iPad and Mac), so check availability at runtime. Context-size APIs arrived in iOS 26.4, with an 8192 default — [createwithswift](https://createwithswift.com/exploring-the-foundation-models-framework/); [blakecrosley.com](https://blakecrosley.com/blog/apple-foundation-models-framework); [stora.sh](https://stora.sh/blog/2026-04-21-apple-foundation-models-framework-ios-26-integration-guide) (secondary sources).
- Apple's NaturalLanguage `NLContextualEmbedding` (iOS 17+) offers per-script contextual token embeddings. It is not a retrieval-tuned sentence embedder and is not available on Android or web, so it would break cross-platform vector compatibility **[unverified, from background knowledge; search returned nothing]**.

### Inferences
- Rule: corpus vectors and query vectors must come from the identical model and weights (and ideally the same quantization). Ship the ONNX model as a versioned package; the vector-set manifest records `model_id`, revision, dims, normalization, prefix and instruction strings (e5 "query:"/"passage:", Qwen3 instruction prompts, EmbeddingGemma task prompts), and quantization.
- Approximate model download sizes (params × bytes): EmbeddingGemma-300m is ~1.2 GB fp32, ~300 MB int8 and ~150–200 MB at 4-bit. multilingual-e5-small (~118M) is ~120 MB int8. Qwen3-Emb-0.6B and BGE-M3 are ~600 MB int8. **[arithmetic estimates]** For phones, EmbeddingGemma int8 or e5-small is the practical tier. Qwen3-0.6B and BGE-M3 suit Mac and iPad or the server.
- Query embedding is one short sequence (under 64 tokens), so latency even for 0.3–0.6B models on modern phone NPUs and CPUs should be tens to low hundreds of ms **[unverified estimate]**.
- Web: transformers.js with q8 WASM as the baseline and WebGPU when available. A 300 MB model download is heavy for a web app, so offer server-side query embedding as an option, or a smaller model tier.
- MLX is Apple-only (Python and Swift). It suits Mac-side corpus building but is not a cross-platform runtime. llama.cpp/GGUF supports embedding models (Qwen3-Embedding and Jina v4 GGUF exist) and is an alternative engine if an LLM runtime is already embedded **[partly unverified]**.
- Candle (Hugging Face's Rust ML framework) can run BERT, XLM-R and Gemma-family encoders in pure Rust, including WASM, which avoids ORT binary packaging **[unverified in this session]**.
- Two-tier strategy: Tier A ships precomputed corpus vectors plus a small query model for all devices. Tier B uses Qwen3-0.6B or BGE-M3 vectors, a reranker and an LLM on Mac, iPad or the server. Keep tiers in separate vector spaces; never mix them.

### Gaps
- No measured latency figures for e5, EmbeddingGemma or Qwen3 embeddings on specific iPhone, iPad or Android devices were found. Google's EdgeTPU number is the only one.
- No confirmation of an embedding API in Apple's Foundation Models framework, and no Rust-specific example of `ort` with CoreML on iOS.

## 6. Size estimates for shipping precomputed vectors; Matryoshka; binary quantization + rescoring

### Takeaway
Shipping precomputed vectors is clearly feasible. One translation's 31,102 verses cost about 12–32 MB at int8 and 1.5–4 MB as binary. Matryoshka truncation to 256 dims plus binary coarse search with int8 or float rescoring keeps even multi-translation and commentary packages to tens of MB. The device still needs the same model to embed queries, which is the real size cost (~100–300 MB).

### Cited Findings
- sqlite-vec supports `bit[]` binary vectors. Its guide's pattern stores a float column plus a binary coarse column, queries the coarse column for about 8× the final limit as candidates, then re-ranks with float distance; "oversampling and re-scoring will help a lot." Binary quantization maps positive values to 1 and the rest to 0 — [sqlite-vec binary quant guide (mirror)](https://https-alexgarcia_xyz.proxy.bugsmash.io/sqlite-vec/guides/binary-quant.html).
- sqlite-vec Matryoshka: `vec_slice()` truncates (for example, to 256 dims) and `vec_normalize()` re-normalizes, which is required after truncation — [sqlite-vec Matryoshka guide (mirror)](https://https-alexgarcia_xyz.proxy.bugsmash.io/sqlite-vec/guides/matryoshka.html).
- Binary embeddings give a reported 15–45× retrieval speedup while retaining over 95% of retrieval accuracy, citing Mixedbread and HF benchmarks — [ikyle.me](https://ikyle.me/blog/2025/binary-quantized-embeddings) (secondary). Scalar (int8) quantization is said to retain quality better than binary — [skills summary, secondary](https://openskillindex.com/skills/comeonoliver-skillshub-sqlite-vec-skilld).
- Models with Matryoshka support: EmbeddingGemma (768 down to 512/256/128) — [Google AI docs](https://ai.google.dev/gemma/docs/embeddinggemma?hl=en); jina-embeddings-v4 (2048 down to 128) — [HF](https://hf.co/jinaai/jina-embeddings-v4). Qwen3-Embedding supports custom output dims via MRL **[unverified in this session]**.

### Size table (arithmetic; 31,102 verses = Protestant canon in KJV versification; LXX and deuterocanon add more)
| Dims | float32 | int8 | binary (1 bit/dim) |
|---|---|---|---|
| 256 (MRL) | 31.8 MB | 8.0 MB | 1.0 MB |
| 384 | 47.8 MB | 11.9 MB | 1.5 MB |
| 768 | 95.5 MB | 23.9 MB | 3.0 MB |
| 1024 | 127.4 MB | 31.9 MB | 4.0 MB |

- Sliding-window or pericope parents add about 30–50% more vectors (for example, a 5-verse window with stride 2 gives ~15.5k windows).
- Commentaries dominate: 100k paragraphs × 768 dims int8 ≈ 77 MB; binary ≈ 9.6 MB.
- Indexes: brute-force binary Hamming search over 50k vectors is trivially fast, so no HNSW is needed per translation. USearch or HNSW only matters at commentary scale (hundreds of thousands of vectors) **[inference]**.

### Inferences
- Recommended storage: binary (or int8 at 256 dims) for the coarse scan and int8 at full dims for rescoring, shipped in the content package's SQLite file next to FTS5. Float32 is unnecessary on device.
- Embedding every translation is optional. Embedding 2–3 representative translations per language and mapping by verse ID covers most needs.
- Query-time requirement: the device must run the identical encoder. When the model is unavailable (low-end device, web without download), fall back to BM25 only, or to server query embedding if a self-hosted server is configured.

### Gaps
- No official sqlite-vec doc was fetched; only mirrors were. Retention numbers for binary and int8 come from secondary sources and are not tested on ancient or biblical text.

## 7. Evaluation practice: building a gold set; not treating similarity as a documented relationship

### Takeaway
Build a small, expert-curated gold set from existing scholarly cross-reference data: OT quotations in the NT, LXX–NT parallels, patristic citations and topical index entries. Add 100–300 natural-language topical queries with graded relevance judgments. Report nDCG@10 and Recall@50 per language and per corpus. In the UI and data model, label embedding hits as "semantically similar" and keep them strictly separate from curated, sourced relations such as cross-references, quotations and lexical links.

### Cited Findings
- The patristic reuse benchmark uses 2,935 expert-verified biblical parallels with gold verse references. It separates binary detection from correspondence retrieval and tests robustness to OCR/HTR noise — [arXiv 2607.24542](https://arxiv.org/abs/2607.24542). This is directly reusable as a model for OpenLogos's Greek/Latin evaluation, and possibly as a dataset if licensing allows.
- The CUP Greek benchmark used 104 expert-annotated queries and compared BM25, dense, hybrid and LLM-assisted retrieval with nDCG — [arXiv 2607.21274](https://arxiv.org/html/2607.21274v1). This shows a ~100-query gold set is enough to rank systems.
- The Krahn et al. evaluation design covers translation search (finding the aligned translation), semantic similarity and retrieval, plus a translation-bias analysis — [arXiv 2308.13116](https://arxiv.org/abs/2308.13116).
- The AGREE benchmark (human judgments of Ancient Greek semantic relatedness) was used to evaluate distributional models — [Stopponi et al., ALP 2023](https://preview.aclanthology.org/setup/2023.alp-1.6).
- The SIGTYP 2024 organizers say there is no universal retrieval benchmark for ancient-language embeddings — [ACL Anthology](https://aclanthology.org/2024.sigtyp-1.19.pdf).

### Inferences
- Gold set components:
  - (a) Quotation retrieval: given an NT quotation, retrieve the OT/LXX source verse.
  - (b) Cross-lingual alignment: given a Greek verse, retrieve the same verse in English or German. This tests translation search.
  - (c) Topical queries ("passages about hospitality to strangers") judged 0–3 by two annotators, with agreement measured.
  - (d) Commentary grounding: given a verse, retrieve commentary paragraphs that actually discuss it, using parsed `refs[]` as silver labels.
  - (e) Negatives: lexically similar but unrelated verses.
- Run the eval in CI over every candidate model, quantization (fp32, int8, binary with rescoring) and MRL dimension. Quantization regressions on Greek and Hebrew may differ from English.
- Data model: keep `relation_type` (curated cross-reference, quotation, allusion from a named source, lexical/lemma link) separate from `similarity_score` (model-derived, with model version). The assistant must say "is semantically similar to" and never "quotes" or "references" unless a curated source asserts it. Show provenance for every relation.
- LLM-as-judge can expand silver labels, but the final gold labels should be human-reviewed, especially for theological concept queries where tradition-specific framing matters.

### Gaps
- No public Bible-specific semantic retrieval benchmark (English or German topical queries over verses) was found. The license and availability of the 2607.24542 parallels dataset were not checked.
