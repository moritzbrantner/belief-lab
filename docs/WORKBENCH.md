# Evidence workbench

The [GitHub Pages workbench](https://moritzbrantner.github.io/belief-lab/) loads downloadable evidence and judgment JSON, runs the actual Rust explanation pipeline through WebAssembly, and shows admission decisions, beliefs, correlated judgments, and provenance. Files are processed in the browser and are not uploaded. The example selector supports shareable `?example=correlated-evidence` URLs.

Examples are deliberately scripted: their scores are not live model predictions. The local `decide` command executes real models; Pages offers its request file and accepts the saved result for inspection. Raw audio/video extraction remains the producer repositories' responsibility. Upload evidence-interchange JSON, not arbitrary media.

## Run a model on a file

From a fresh clone:

```sh
cargo run -- decide examples/decisions/preference.json > result.json
```

This authorizes the request before installing or invoking SemIf. First use downloads the pinned provider and phone-tier model. Later calls reuse them. Progress goes to stderr; stdout contains exactly one JSON result. Open `result.json` in the workbench to inspect it. Change `provider.tier` to `desktop` or `high-memory` to select another pinned model. `examples/decisions/fixture.json` runs the same boundary entirely offline with explicitly simulated scores.

## Build and verify Pages locally

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.128 --locked
scripts/build-pages.sh
python3 -m http.server --directory target/pages 8765
```

Open http://localhost:8765. Assets and downloadable examples are assembled under `target/pages`; no generated bundle is committed. Node is only needed for browser acceptance tests:

```sh
npm --prefix site ci
(cd site && npx playwright install chromium)
npm --prefix site test
```

The Pages workflow builds and tests pull requests and deploys `main` after its browser checks pass. Rust workspace checks continue independently of browser tools and models.

## Roadmap coverage

- #11: declared ownership in `repository-purpose.json`, strict reference-only imports, offline core, checksum-verified setup, and an intentional Pages workbench. The corpus fixture is illustrative; producer-export compatibility remains conditional on real producer exports becoming available.
- #13: exact-target model panels, deterministic descriptive summaries, source-family correlation through `panel_bases`, and preservation of all members in store explanations. Panels do not schedule providers or turn votes into probabilities.
- #14: versioned one-decision JSON/stdio command with validation and authorization before provider resolution, stable errors, exact provenance, and offline fixture-provider tests.

Verification convention source revision: `e6acb5310afaf15c0cba24f87108f5f4ad1bedc3`. This records the policy used for this implementation; it does not pin future policy consumption.

Real-provider acceptance (explicitly downloads any missing resources, then executes all three CPU model tiers):

```sh
cargo test -p belief-cli --test real_models -- --ignored --nocapture
```

After setup, repeat with `HF_HUB_OFFLINE=1 PIP_NO_INDEX=1` to verify reuse of the prepared resources. The ordinary workspace test command skips this acceptance test.
