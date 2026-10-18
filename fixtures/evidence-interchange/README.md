# Evidence interchange fixtures

`youtube-multimodal-v1.json` is an illustrative, consumer-authored fixture. It does
not prove compatibility with a real corpus exporter.

`youtube-corpus-export-v1.json` is copied byte-for-byte from
[`youtube-corpus/tests/fixtures/exports/expected/belief-evidence.json`](https://github.com/moritzbrantner/youtube-corpus/blob/ae69211c175eb8817f467559a4cb936b7a39de2d/tests/fixtures/exports/expected/belief-evidence.json)
at producer commit `ae69211c175eb8817f467559a4cb936b7a39de2d` (PR #91).
Its SHA-256 is `71dff7304361e2fe592edab9417ab7ac307b525b4c4878feaaffc35e1df578f8`.
The producer owns the synthetic source snapshot and independent fixture generator;
this copy is a consumer compatibility input, not a new producer implementation.
The content is synthetic; the references do not assert real-world source truth.

The real CLI golden case is `fixtures/explain/producer-youtube-corpus`. The native
`producer_compatibility` test additionally checks authorization, exact evidence
and receipt retention in the store, and idempotent re-import. Neither test needs
a producer checkout, external service, model or network connection.

The producer keeps release labels separate from recorded immutable processor
source revisions. Historical streams lacking that provenance are rejected by the
producer; the consumer's strict import validation remains unchanged. No source
text, detector payload or fabricated semantic judgment is added here.

`philosophy-extractor-export-v1.json` is copied byte-for-byte from
[`philosophy-extractor/packages/exporters/tests/fixtures/belief-evidence.json`](https://github.com/moritzbrantner/philosophy-extractor/blob/73a5c218a871ea681f43d03a45af22dbe1251761/packages/exporters/tests/fixtures/belief-evidence.json)
at producer fixture commit `73a5c218a871ea681f43d03a45af22dbe1251761` (PR #15). Its SHA-256 is
`dbc6e797364f1a4acd1c5f269c81b6da912d1d7e99fb228cb9accb73d7961790`.
The producer's real corpus-core intake consumes the existing document-search
fixture; the independent Python oracle checks native record hashes and derives
the expected export. Source identities and upstream processor receipts remain
those of document-search, separately from the Philosophy Extractor exporter.

Three representation descriptors are metadata; seven document passages are
`source_text`, each dependent on its representation descriptor. No philosophical
candidate, extraction score or worldview assertion is relabeled as evidence.
The real CLI golden case and native store test cover this export as well. These
fixtures prove reference interoperability, not automatic proposition discovery or
the original-source upload workflow under #36.
