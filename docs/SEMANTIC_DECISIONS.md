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

The default setup uses the CPU-local `llama.cpp` backend and phone-tier model. It is idempotent and retry-safe: an existing SemIf checkout must point at the expected upstream repository, the exact pinned revision is restored, an existing virtual environment is reused, and incomplete GGUF downloads are resumed before exact-size and SHA-256 validation and atomic promotion. The lower-level `belief-semif` binary remains available for provider development.

The model weights are not vendored into `belief-lab` or SemIf. Their upstream licenses remain authoritative.

## Provenance

Every accepted result retains:

- active policy profile and authorized evidence uses;
- SemIf source revision;
- model source and pinned revision;
- runtime backend;
- prompt SHA-256 and readout identity;
- complete option-score map and selected option.

For the llama.cpp backend, SemIf computes the local GGUF SHA-256 at load time. `semif-provider` requires the reported file name, byte size, and SHA-256 to match the exact pinned artifact before accepting the result, then folds the pinned digest into the stored model revision. A same-sized substituted GGUF therefore cannot acquire valid Belief Lab provenance merely by reporting an arbitrary digest.

`belief-store::insert_semantic_judgment` persists this provider provenance beside the judgment. `explain_belief` returns semantic-decision provenance for selected semantic judgments, preserving the chain from belief back through inference, model decision, evidence, and producer revisions.

## Machine interface

`cargo run -- decide <request.json|->` executes exactly one `belief_semantic_request@1` and returns one `belief_semantic_result@1` on stdout. See the complete requests in `examples/decisions/`. Inputs are limited to 1 MiB and reject unknown fields. Policy is explicit in the request; ambient policy environment variables are ignored. `observe_only` remains the default when no profile is supplied.

The envelope includes bounded state, a three-way question/options, inference class, evidence-interchange batch, evidence uses/purposes, proposition, judgment specification, explicit policy settings, and provider selection. Providers are `semif` with a supported model tier or `fixture` with explicitly simulated scores. All external inputs and policy gates pass before the resolver can acquire or execute a model. No arbitrary executable path or remote model identifier is accepted.

Success exits 0; failures exit 2 with a structured error code (`usage`, `input_io`, `invalid_request`, `unsupported_schema`, `policy_denied`, `invalid_provider`, `setup_failed`, `provider_failed`, `invalid_receipt`, `store_failed`, or `inference_failed`). Diagnostics and setup progress use stderr. Results retain the submitted request, provider/model revisions, runtime, prompt identity, all option scores, selected judgment, and the stored inference explanation. Unknown judgments succeed with a null belief. Fixture results explicitly identify their simulated provider and do not claim a real prompt digest.

Workflow hosts own scheduling, retries, cancellation, and fan-out/fan-in. They should persist the full result outside workflow history and return compact decision/judgment identifiers and summaries. Belief Lab imports no workflow runtime.

## Model panels

`ModelPanel` groups judgments over one exact authorized request, proposition, and judgment specification. A member receipt must match the entire request, including evidence and state, even when IDs match. Members sort by judgment ID. `summary()` reports support, contradiction, and unknown counts and disagreement; these are descriptive counts, not belief scores.

`belief_cli::pipeline::panel_bases` assigns all members the same deterministic source-family correlation group. Changing the provider or panel ID does not create a new source contribution. Distinct source families remain distinct contributions; producers/callers still own truthful family attribution. Callers combining overlapping but nonidentical multi-family targets must assign a common correlation group before inference: the baseline does not establish statistical independence automatically.

`insert_model_panel` stores every member atomically after validating dependencies and request evidence. Belief explanations retain complete panels touching the selected derivation, including correlated alternatives and unknowns. Historical panel receipts remain inspectable after evidence invalidation.

Local llama.cpp scoring uses only the tokenizer cache prepared by setup, and provider execution has a 10-minute deadline. Model downloads have a one-hour bound and bounded retries; initial native dependency compilation streams progress until completion or user interruption.
