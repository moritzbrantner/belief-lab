# Belief Lab

`belief-lab` experiments with turning heterogeneous, provenance-bearing evidence into explainable derived claims without making detector confidence, model confidence, soft truth, and posterior probability interchangeable.

The first milestone is intentionally small:

- consume the `source_span_interchange@1` and `media_evidence@1` shapes already used across the corpus/extraction work;
- dogfood `moenarch-corpus-core` typed source/asset/segment identities at the import boundary;
- keep object/face/voice/NLP observations as evidence rather than facts;
- derive one deterministic baseline claim through an explicit entity-link rule;
- expose `belief explain` so every derived claim can be traced back to source and producer revisions;
- showcase the model on GitHub Pages using `github-pages-template` in augment mode.

## Try the fixture

```bash
cargo run -p belief-cli -- validate fixtures/multimodal-person.json
cargo run -p belief-cli -- infer fixtures/multimodal-person.json
cargo run -p belief-cli -- explain \
  fixtures/multimodal-person.json \
  claim:person:alice:uses:product:thinkpad
```

The derived score is deliberately `heuristic_strength`. It is **not** represented as a Bayesian posterior.

## Authority boundaries

- `moenarch-foundation` owns portable corpus identity/provenance primitives.
- `scenedetect-rs` owns scene detection.
- `visual-analysis` owns object/face/OCR analysis.
- `audio-analysis` owns reusable audio/ASR/speaker capabilities; `native-whisperx` owns WhisperX-style composition.
- `nlp-stack` owns linguistic extraction and semantic graph structure.
- corpus applications such as `youtube-corpus` own persisted/aligned producer evidence.
- `belief-lab` owns uncertain links, inference experiments, explanations, and later belief semantics.

See [docs/architecture.md](docs/architecture.md) and [ROADMAP.md](ROADMAP.md).
