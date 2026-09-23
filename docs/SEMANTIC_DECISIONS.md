# Semantic decisions

`semantic-decision` is the bounded provider boundary between admitted evidence and a model-produced `Judgment`.

```text
producer evidence references
        |
        v
DecisionRequest + evidence purposes
        |
        v
belief-policy authorization
        |
        v
AuthorizedDecisionRequest
        |
        +--> SemIf adapter today
        +--> other adapters later
        |
        v
Judgment + provider receipt
        |
        v
inference-core / inference-baseline
        |
        v
Belief
```

A decision provider never receives a store, corpus client, database handle, or unrestricted search capability from this crate. The caller resolves the exact bounded `state` it wants assessed and attaches the evidence references that justify that state. Policy checks the inference class, every evidence class/purpose, and cross-source joins before a provider can run.

## Score semantics

Semantic option scores are `conditional_option_probability`, not belief truth and not automatically a calibrated confidence estimate. A three-way support decision uses the exact option ids `supports`, `contradicts`, and `unknown`; the winning option becomes the judgment outcome and its score stays tagged as a conditional option probability.

Only an inference engine may turn judgments into `soft_truth` or `posterior_probability`. The deterministic baseline continues to produce only soft truth.

## SemIf adapter

`semif-provider` implements `SemanticDecisionEngine` against the open SemIf CLI. It is an independent adapter, not a Jev implementation and not a claim of Jev compatibility.

The adapter pins SemIf source revision:

`1f2dea3e25379f9dfc98cb83c324f00ab5deda37`

It also pins the source-model and GGUF revisions used by SemIf's published local model ladder.

```bash
cargo run -- setup
cargo run -- semantic-demo
```

The default setup uses the CPU-local `llama.cpp` backend and phone-tier model. It is idempotent and retry-safe: an existing SemIf checkout must point at the expected upstream repository, the exact pinned revision is restored, an existing virtual environment is reused, and incomplete GGUF downloads are resumed before exact-size validation and atomic promotion. The lower-level `belief-semif` binary remains available for provider development.

The model weights are not vendored into `belief-lab` or SemIf. Their upstream licenses remain authoritative.

## Provenance

Every accepted result retains:

- active policy profile and authorized evidence uses;
- SemIf source revision;
- model source and pinned revision;
- runtime backend;
- prompt SHA-256 and readout identity;
- complete option-score map and selected option.

For the llama.cpp backend, SemIf computes the local GGUF SHA-256 at load time. `semif-provider` requires the reported file name/size to match the pinned artifact and folds that SHA-256 into the stored model revision. This means an explanation identifies the actual quantized file that produced a judgment.

`belief-store::insert_semantic_judgment` persists this provider provenance beside the judgment. `explain_belief` returns semantic-decision provenance for selected semantic judgments, preserving the chain from belief back through inference, model decision, evidence, and producer revisions.
