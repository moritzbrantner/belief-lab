# Decision Atlas

This file is the owner-controlled authority for what Belief Lab should do for its users. Its navigation follows product capabilities and user workflows. The only semantic entries are **Question** (unresolved) and **Decision** (explicitly owner-approved). Preserve entry IDs and the original Questions when recording answers; append a new Decision that names the Question it answers. Do not infer approval from an implementation or passing test.

Repository boundaries, safety policy, and technical implementation rules remain in `AGENTS.md`, the installed conventions, and the architecture documents.

## 1. Evidence-led investigations

### 1.1. Starting an investigation

#### Question 1.1.1 — Primary user experience

Should Belief Lab primarily start from a claim to evaluate (A), from supplied evidence and sources to analyze (B), or from an interactive map of existing beliefs (C)?

#### Decision 1.1.2 — Evidence-first is the primary workflow

**Answers:** 1.1.1  
**Approved by owner:** 2026-10-13, option B.

Users begin an investigation by supplying or referring to documents, research, or other sources. Belief Lab identifies propositions in that evidence, connects related support and contradictions, and constructs explainable beliefs whose underlying sources and judgments can be inspected.

A claim-first query or interactive belief map may complement this workflow, but neither defines the primary entry point. Source-relative statements are not automatically world truth; source ownership, provenance, policy authorization, and the distinction between model judgments and inferred beliefs remain intact.

**Implementation distinction:** The current Pages workbench primarily loads structured evidence/judgment JSON, and its examples use scripted judgments. Approval of this direction does not mean arbitrary document ingestion, automatic proposition extraction, or the complete evidence-first journey has already been implemented. Specific input methods and review behavior are not yet approved.

#### Question 1.1.3 — Continuity across investigations

When a user adds new evidence over time, should Belief Lab maintain one evolving knowledge base, keep analyses isolated, or allow separate evolving investigations?

#### Decision 1.1.4 — Separate persistent investigations

**Answers:** 1.1.3  
**Approved by owner:** 2026-10-13, option C.

Belief Lab supports multiple distinct investigations. Each investigation persists across work sessions and accumulates its own evidence, propositions, judgments, and inferred beliefs as more evidence is introduced. Users can return to an earlier investigation and continue its analysis without automatically combining unrelated investigations into a single global belief network.

This decides the **user-visible continuity model**, not a specific persistence technology, account system, cross-device synchronization mechanism, or sharing model. Existing in-memory store behavior does not demonstrate that persistent investigations have been implemented.

#### Question 1.1.5 — Availability of saved investigations

When an investigation is saved, where should the user be able to reopen it? On the same device only (A), through an account across devices (B), or by explicitly moving/importing a portable investigation file (C)?

#### Decision 1.1.6 — Portable investigation project files

**Answers:** 1.1.5  
**Approved by owner:** 2026-10-13, option C.

Each persistent investigation can be saved to a portable project file, explicitly reopened, and transferred to another device without requiring a user account or automatic cloud synchronization. The imported investigation remains independently navigable and can be continued by adding new evidence and updating beliefs, while preserving applicable provenance and explanation history.

This is a product portability requirement, not an authorization to change producer-owned evidence truth, bypass policy gates, or invent a second inference engine. Project file content, whether to embed original source material or only refer to upstream sources, and how inaccessible references appear upon reopening remain unresolved. Existing JSON request/result downloads and the current in-memory store do not constitute implementation of a full portable investigation.

#### Question 1.1.7 — Evidence packaged in a portable project

When exporting an investigation, should its project file contain the original source material for offline reopening (A), only provenance-bearing references to externally held sources (B), or an explicit choice between the two at export time (C)?

#### Decision 1.1.8 — Choose between full and reference-only project exports

**Answers:** 1.1.7  
**Approved by owner:** 2026-10-13, option C.

When exporting an investigation, the user chooses between two portable project-file modes:

- **Full bundle:** include the investigation and available, permitted source material so the evidence can be inspected after moving the project to another device, including offline when all needed material was bundled.
- **References-only:** include the investigation, its saved analysis, provenance, and references to externally held source material without duplicating originals. This keeps the export smaller but does not guarantee that the referenced originals can be accessed offline.

Both modes must preserve the investigation's evidence relationships, explanations, and ability to continue the investigation when the necessary evidence is available. Neither mode changes producer ownership, makes imported assertions automatically true, bypasses policy admission, or implies that a source which cannot legitimately be included is embedded. The precise treatment of missing external sources is still unresolved.

**Implementation distinction:** The current ability to download evidence or judgment JSON is not the complete portable-investigation export/import workflow.

#### Question 1.1.9 — Missing referenced evidence on import

When a reference-only project is opened on a different device but some original sources cannot be reached, should Belief Lab (A) open the saved investigation and flag missing sources, (B) refuse to open it until all sources are restored, or (C) open it with missing sources flagged and guide the user to reconnect or replace them before dependent results are recomputed?

#### Decision 1.1.10 — Open partial investigations and guide missing-source recovery

**Answers:** 1.1.9  
**Approved by owner:** 2026-10-13, option C.

Opening an investigation does not fail merely because some external evidence references are inaccessible. Belief Lab opens the saved investigation, visibly identifies each missing source, and guides the user to reconnect the original material or provide replacement evidence. Preserved conclusions, derivations and explanations remain inspectable as **saved historical results**, with their missing-source limitations made clear; they must not be presented as freshly verified current conclusions or silently recalculated.

Reconnection or replacement must preserve provenance and respect the existing authorization/invalidation boundaries. Replacing a missing source is new evidence rather than silently substituting material under an older authorized source identity. Beliefs that depend on missing evidence cannot be newly recomputed as if that evidence were present; after sources are restored or replacement evidence is admitted, updated results can be generated with traceable derivations, without erasing the original history.

**Scope distinction:** This is the owner-approved user experience for importing reference-only portable investigations; it does not authorize a particular persistence technology or automatically available external source resolver.

### 1.2. Discovering propositions from evidence

#### Question 1.2.1 — Proposition discovery and user review

After adding a source to an investigation, should Belief Lab (A) automatically extract propositions and run belief analysis with optional later correction, (B) propose candidate propositions and require human approval before analyzing them, or (C) automatically analyze with clearly reviewable suggestions that users can accept, correct or exclude without a mandatory approval gate?

#### Decision 1.2.2 — Automatically analyze reviewable proposition candidates from producer text

**Answers:** 1.2.1  
**Approved by owner:** 2026-10-13, option C, with the existing-project pipeline requirement.

Belief Lab's evidence-first workflow is an integrated pipeline: text produced or normalized by the owner's existing transcription, translation, corpus, and NLP projects is usable as source-grounded input for identifying proposition candidates. Belief Lab should reuse the upstream producer capabilities and their provenance-bearing outputs rather than reimplementing transcription, translation, OCR, corpus persistence, or a competing proposition extractor.

When a source is added and its text can be obtained through an authorized path, candidate propositions are discovered and eligible analysis proceeds **without requiring the user to approve every candidate first**. The user can inspect the proposed propositions, see where each came from, correct or exclude candidates, and have subsequent analyses respect those choices. Automatic analysis is conditional on existing policy authorization and does not convert a model-produced candidate into an endorsed truth claim.

The pipeline should retain source spans, the source and producer revisions, derived-text lineage (including translations and transcriptions), provider/model receipts when used, and the distinction between source-reported claims, model judgments, and Belief Lab's inferred beliefs. Editing a candidate is a user intervention with its own provenance: do not quietly rewrite the producer's source text or retain dependent judgments as if the edited proposition were unchanged.

**Existing relevant producers:** `audio-analysis` for transcription, `youtube-corpus` for corpus/ingest and source identities, `nlp-stack` for text capabilities, and existing proposition-candidate pipelines such as `philosophy-extractor` and `proposition-api` where applicable. Translation outputs are also valid text evidence when their producing component can supply appropriate provenance. This names candidate integrations, not a claim that their interoperability is already implemented or that each provider must run for every source.

#### Question 1.2.3 — How users start an evidence-processing pipeline

In the primary investigation experience, should users (A) import text or proposition artifacts prepared elsewhere, (B) submit original sources and have the existing producer pipeline process them automatically, or (C) use either prepared text/artifacts or original sources with automatic pipeline processing, depending on their needs?

#### Decision 1.2.4 — Prepared and original sources share one evidence-first investigation workflow

**Answers:** 1.2.3  
**Approved by owner:** 2026-10-13, option C.

An investigator can start from either **prepared inputs**—text, transcripts, translations, source-grounded proposition candidates, or other evidence artifacts from upstream producer projects—or **original sources** such as documents, recordings, videos, or source links that the relevant existing producer pipeline processes automatically. The user should not have to run a separate upstream command manually before using the original-source path when a supported producer is available.

Both routes converge on the same reviewable proposition-candidate and belief-analysis experience defined by Decision 1.2.2. Candidates are discovered and may be analyzed automatically when authorized, while users can inspect supporting source spans, correct candidates, and exclude them without a mandatory approval step. Input preparation does not bypass policy. Each accepted source preserves producer/corpus ownership, exact source revisions, derivation lineage (including transcription or translation), evidence families, and provider receipts as applicable.

Belief Lab coordinates and consumes upstream outputs but must not duplicate ingestion, ASR, OCR, transcription, translation, NLP, corpus truth, or candidate-extraction implementations already owned elsewhere. Availability of individual producer formats and services must be accurately reported; an unavailable processing path must not be presented as successfully analyzed. This decides the **supported user entry paths**, not an assertion that all source kinds or adapters are already implemented.

#### Question 1.2.5 — Long-running source processing

When processing a long recording or collection, should Belief Lab (A) wait and show the final analysis only, (B) show step-by-step progress but only release results at completion, or (C) show progress and incrementally reveal clearly labeled preliminary propositions and results as they become available, without mistaking them for final conclusions?

#### Decision 1.2.6 — Incrementally reveal provisional findings during long-running processing

**Answers:** 1.2.5  
**Approved by owner:** 2026-10-13, option C.

For a long recording, document, or collection, Belief Lab shows meaningful processing progress and makes **already available, policy-authorized proposition candidates and findings visible incrementally**, rather than withholding all useful results until the complete pipeline finishes. Findings derived from incomplete evidence are clearly identified as **preliminary**; the user can inspect their source spans and start reviewing or correcting candidates while later work continues.

The interface distinguishes ongoing, completed, failed, and unavailable processing stages from final conclusions. New evidence may confirm, revise, or invalidate preliminary inferences; any changes retain traceable source/provenance relationships rather than quietly replacing earlier results. Completing a stage is not the same as completing the entire investigation. Partial failures do not fabricate a success status or turn incomplete evidence into confident truth.

This commits to **progressive user-visible results**, not a particular streaming transport, fabricated percentage, or requiring every upstream producer to yield sub-step events. A producer with an atomic output can still publish that output when it finishes while other stages continue. Reuse existing orchestration/capabilities where suitable; do not duplicate upstream processing or rerun unaffected completed work merely to update progress.

### 1.3. Evaluating propositions across sources

#### Question 1.3.1 — How contradictions should be presented

When sources disagree about a proposition, should Belief Lab (A) show the current derived belief score with its evidence links, (B) prioritize a side-by-side comparison of supporting and contradicting evidence, or (C) show both that evidence comparison and a history of how the belief changed as sources were added?

#### Decision 1.3.2 — Compare conflicting evidence and show belief evolution

**Answers:** 1.3.1  
**Approved by owner:** 2026-10-13, option C.

When an investigation includes support and contradiction for a proposition, Belief Lab presents the current belief assessment together with **side-by-side supporting and contradicting evidence**, each traceable to the originating source, relevant passage, judgment and provenance. Unknown, excluded, inaccessible and policy-denied material must not silently count as confirmation or refutation.

The user can also inspect a **history of how that belief changed** as evidence was added, corrected, withdrawn or re-evaluated. Historical snapshots remain distinguishable from the current assessment, including prior source/judgment contributions and the change that led to each revision. A user should be able to see what was known before a source arrived and what changed afterward, without mistaking a historical result for a freshly verified result.

The comparison is not a majority vote: three model judgments over the same evidence and multiple correlated excerpts do not become independent confirmations. Display the inference engine's actual declared score semantics (for example `soft_truth`) and the relevant dependency/authorization context; never turn counts or uncalibrated scores into an invented posterior probability. The user can inspect source identities and the explanation for included and omitted contributions.

This chooses the **product experience**, not a mandated chart style, time-series database, automatic credibility scale, or unsupported certainty. A current workbench report's single inference explanation does not by itself demonstrate the approved across-revision belief history.

#### Question 1.3.3 — Apparent contradictions across contexts

If two sources appear to disagree but describe different years, populations or definitions, should Belief Lab (A) compare them as a contradiction until the user manually separates them, (B) automatically treat them as distinct scoped propositions whenever it detects the difference, or (C) suggest the differing context and offer reviewable separate propositions instead of silently asserting a direct contradiction?

#### Decision 1.3.4 — Suggest and review contextual distinctions before asserting contradictions

**Answers:** 1.3.3  
**Approved by owner:** 2026-10-13, option C.

When sources appear to disagree but refer to different time periods, populations, locations, conditions or meanings of a term, Belief Lab identifies the **potential context mismatch**, shows which source passages support that interpretation, and offers **reviewable, separately scoped proposition candidates**. A suggested mismatch is not automatically approved by the user, nor is it a proven contradiction or proof that the sources are consistent.

Users can inspect, correct, accept or reject the suggested distinction. Until a comparison is justified for the same proposition under compatible context, Belief Lab should not silently merge the source claims into an unconditional support-versus-contradiction assessment. Uncertainty about whether contexts really differ remains visible rather than silently resolving either way.

Reviewing a context distinction updates or separates the *Belief Lab* propositions, relationships and dependent judgments with traceable revisions; it does not modify producer-owned original text, source revisions or already-stored historical explanations. Any resulting new inferences still pass policy admission and use the actual evidence-family/correlation and score semantics.

Example: A study describing an intervention in **adults** and one describing it in **children** can appear to disagree. Belief Lab proposes separately scoped propositions and lets the user compare them, without automatically claiming a genuine conflict or falsely declaring the findings equivalent.

#### Question 1.3.5 — Source reliability in belief assessments

When evidence comes from sources of different reliability, should Belief Lab (A) weigh all independent sources equally unless manually excluded, (B) automatically estimate source reliability and use it to influence the assessment, or (C) display reliability considerations transparently and allow the user to review or adjust their influence, rather than applying hidden automatic weights?

### 1.4. Attributing beliefs to people

#### Decision 1.4.1 — Map what people believe; do not adjudicate whether claims are true

**Answers:** 1.3.5 — rejects the question's premise that Belief Lab should weight sources in order to decide a proposition's truth.  
**Supersedes:** the proposition-truth-assessment interpretation of Decisions 1.1.2, 1.2.2, 1.3.2 and 1.3.4. Their evidence-first workflow, proposition discovery, comparison/history, and context-review choices remain approved **only as applied to belief attribution**.  
**Approved by owner:** 2026-10-13, explicit product-purpose correction.

Belief Lab's fundamental task is to **connect beliefs and propositions to the people who hold, express or are reported to hold them**, with reviewable source-grounded attribution. It must distinguish:

1. **A source says proposition P.** This records what a document, recording, transcript, or other source expresses and who is cited or speaking, where evidenced.
2. **Person A believes proposition P.** This is a separate, evidence-backed attribution of a belief to that person; it is not implied solely by seeing P quoted in a source. Attribution may have uncertainty and context requiring user review.
3. **Proposition P is true of reality.** This is a separate epistemic question, *not* Belief Lab's product goal. It may be handled by another project; Belief Lab must not silently turn its attribution evidence or person/source agreement into proof of P.

For example, the useful comparison is **what the blogger believes and what the individual scientists believe**. A peer-reviewed article is not automatically more truthful than a blog post in Belief Lab's model, and the product does not rank the truth of their underlying factual claims. Reliability of evidence about *who asserted or believes what* may still matter to attribution, but global truth-reliability weighting is out of scope.

Previously approved evidence processing remains valuable: the ingestion/transcription/translation/NLP pipeline finds source-grounded proposition candidates; the review interface helps users distinguish quotations, reported viewpoints and actual endorsements; belief comparisons can show how different people relate to the same proposition; historical views track how an attributed person's expressed stance changed or how its supporting attribution evidence changed. They must **not** present a time series of changing objective truth for P.

The existing Rust model and baseline inference expose proposition-level `soft_truth` semantics, which do **not** establish this person-to-belief product contract. These current implementation mechanics are not to be rebranded as an attribution score or treated as acceptance for the revised product without an explicit domain/acceptance reconciliation. Keep the existing evidence provenance, policy admission, correlation and authorization boundaries. No hidden global identity resolution or sensitive-trait inference is authorized by this Decision; the repository's current restrictions remain in force. Person-identity and attribution rules that materially change scope require further owner decisions.

This is a substantive revision to earlier product intent, not a silent edit to the historical Decisions.

#### Question 1.4.2 — Evidence that someone holds a belief

When a source contains a proposition, should Belief Lab (A) attribute it to a person only when they explicitly endorse it, (B) infer likely beliefs from authored or reported statements automatically, or (C) distinguish explicit endorsement from reported or inferred attribution, showing the uncertainty and allowing user review so quotations are not automatically mistaken for beliefs?

#### Decision 1.4.3 — Reviewable attributed beliefs with explicit uncertainty and confidence

**Answers:** 1.4.2  
**Approved by owner:** 2026-10-13, option C, with an explicit confidence-score requirement.

Belief Lab supports **explicitly endorsed**, **reported**, and **inferred** person-to-proposition belief attributions. These are distinct attribution types, not interchangeable evidence of endorsement. Seeing a person quote, discuss, translate, criticize, or report a proposition does not automatically imply that the person believes it. Users can inspect, correct, reject, and review candidate attributions; lack of manual approval does not prevent authorized, source-grounded automatic analysis.

**Every asserted belief attribution must include a confidence score about the attribution**—the strength or confidence of the claim that the named person holds the proposition, *not* a score for whether that proposition conforms to reality. Show the attribution type, source passages, how the attribution was reached, source and producer revisions, model/judgment provenance when used, and relevant uncertainty alongside that score. Distinguish model-output confidence, evidence about what was said, and any derived attribution confidence; do not silently reinterpret the current proposition-level `soft_truth` value or an uncalibrated model score as a probability that a person believes P.

When evidence is too weak, ambiguous, missing or unauthorized to justify an attribution confidence, preserve it as an unresolved candidate or unknown outcome instead of assigning a fabricated score. User corrections remain provenance-bearing revisions; prior attributed beliefs remain historically inspectable. All attributions remain subject to existing fail-closed policy gates and source/corpus-local identity restrictions. No real-world identity resolution or sensitive-trait inference is authorized by this Decision.

The precise meaning and semantics of the confidence score, including whether confidence in *who said something* must be tracked separately from confidence in *whether the person believes it*, are not yet owner-approved and must not be silently chosen during implementation.

#### Question 1.4.4 — What attribution-confidence scores measure

When a claim is connected to a person, should Belief Lab assign (A) one score estimating how likely the person really believes the proposition, (B) one score for how confidently the available source establishes the person's endorsement or reported stance, or (C) distinct scores for identifying the correct speaker/author and for whether that person actually endorses or believes the proposition?

#### Decision 1.4.5 — Canonical confidence and separate conviction for person-belief attribution

**Answers:** 1.4.4  
**Approved by owner:** 2026-10-13, option C, with the owner's naming choice: the first score is canonical `confidence`, the second is `conviction` (or an equivalent clear name if needed).

A person-to-proposition belief attribution has **two distinct evidence-grounded assessments**, never a truth judgment about the proposition:

- **`confidence` (canonical):** How confident Belief Lab is that the underlying statement, quotation, authored passage or reported position has been attributed to the **correct person**. This assesses the provenance of the speaker/author/source-person association within the identity scope currently permitted by policy; it is not real-world identity resolution.
- **`conviction`:** How confident Belief Lab is that the correctly identified person **actually holds or endorses the scoped proposition**. A statement may be accurately attributed but merely quoted, criticized, described hypothetically, or reported on someone else's behalf; in these cases source attribution can be strong while belief conviction is weak or unresolved. This assessment is not a measurement of how strongly the person *feels* the belief.

The second score deliberately uses the owner's preferred **conviction** terminology. Both have independently inspectable reasons and evidence, with explicit/reported/inferred attribution types, source spans, provenance and uncertainty. They are not interchangeable and may change independently. A missing or unsupported component remains unresolved rather than receiving an invented number. User corrections retain prior attribution and assessment history. Neither score establishes whether the underlying proposition is true of reality, and neither is the current proposition-level `soft_truth` value or automatically a calibrated posterior probability.

This specifies **product semantics and names**, not a particular numeric scale, inference formula, model/provider, storage representation or mandatory extra infrastructure. Preserve the existing authorization and source/corpus-local identity gates. No sensitive-trait inference or unrestricted real-world identity matching is authorized.

#### Question 1.4.6 — Representing a person's strength of belief

Should Belief Lab (A) measure only `confidence` and `conviction` as defined above, without estimating how strongly a person feels the belief, (B) add a **third, separate** strength-of-belief measure when evidence supports it, or (C) show only qualitative intensity clues from the source without another scored metric?

#### Decision 1.4.7 — Observation confidence, person confidence, and conviction are separate

**Answers:** 1.4.6 — the owner's corrected scheme includes a separately scored estimate of the person's certainty about the proposition.  
**Supersedes:** Decision 1.4.5's two-score semantics and its prior meanings of canonical `confidence` and `conviction`. Retain Decision 1.4.5 as historical approved intent, but use these corrected definitions going forward.  
**Approved by owner:** 2026-10-13, explicit correction with three named measures.

Belief Lab distinguishes **three assessments** associated with a source-grounded person-to-proposition belief attribution:

1. **`confidence (observation)`:** *How certain we are that this specific observation implies the attributed belief.* It concerns the evidence from an individual quote, report, authored statement or other authorized observation **for a scoped person and proposition**. A person merely quoting or criticizing P must not receive a high observation-to-belief score simply because the quotation was recorded or attributed correctly.
2. **`confidence (person)`:** *How certain we are that this person actually has that belief*, taking into account the available admissible observations and their dependencies. This is the **canonical person-belief attribution confidence**, not confidence merely that a statement came from the correct speaker. Multiple observations of the same underlying content or from correlated sources must not be counted as independent confirmation.
3. **`conviction`:** *How certain the person themselves is about the proposition.* This concerns the **person's own degree of certainty in P**, not our confidence that the person endorses P. A person may clearly hold a tentative belief: `confidence (person)` can then be high while `conviction` is low.

Each measure must be shown with its meaning and evidence/derivation, kept distinct in storage, explanations and historical comparisons, and allowed to be unknown when the available evidence cannot justify a value. The numbers are **not interchangeable** and none estimates whether P conforms to reality. Source-to-person attribution and source provenance remain essential evidence and authorization checks, but **speaker-identification confidence is not one of these three defined belief scores**. Real-world identity resolution and sensitive-trait inference are not authorized by this Decision.

This establishes the owner's **product meanings**, not an implementation formula, calibration guarantee, mandatory numeric range, or a permission to invent evidence. The previous `soft_truth` baseline is not silently any of these values, and an inference provider's raw model score is not automatically a calibrated person-belief confidence or conviction. Belief attribution stays explicit/reported/inferred and reviewable as in Decision 1.4.3.

#### Question 1.4.8 — How conviction should be inferred

For estimating how certain someone is about a proposition, should Belief Lab (A) assign conviction only when the person directly expresses their certainty, leaving it unknown otherwise, (B) infer conviction automatically from wording, repeated actions or other observations, or (C) use both but clearly distinguish self-reported conviction from inferred conviction and make the estimate reviewable?

#### Decision 1.4.9 — Distinguish self-reported and inferred conviction

**Answers:** 1.4.8  
**Approved by owner:** 2026-10-13, option C.

Belief Lab may derive the **conviction** measure from either a person's **direct expression of their own certainty** about a proposition or an **inference** supported by their wording, repeated statements, behavior or other policy-admitted observations. Every conviction assessment must distinguish **self-reported** from **inferred** certainty, show the relevant source passages and provenance, and be reviewable and correctable. A person's own stated degree of certainty is a report about their conviction; the model's inference is a separate interpretation, and neither establishes whether the proposition is objectively true.

Do not silently promote indirect clues into a self-report, presume that repetition proves high conviction, or fabricate a numeric value when the evidence is insufficient. A single assessed conviction can have multiple supporting or conflicting observations; preserve their dependencies, allow changes as evidence is added, and retain historical explanations. Context matters: quotation, satire, hypothetical statements, changed stance and the date or subject of a claim must not be mistaken for unqualified current personal certainty.

This decision operates within Decision 1.4.7's three distinct measures: `confidence (observation)` assesses how strongly one observation implies a person-belief attribution; `confidence (person)` assesses whether the person holds that belief across admissible observations; **`conviction` concerns how certain the person themselves is of the proposition**. The inference must not use the prior proposition-level `soft_truth` value as a shortcut or bypass the existing policy and source-identity constraints.

#### Question 1.4.10 — Linking person records across sources

When two documents appear to involve the same person, should Belief Lab (A) keep source-local people separate unless the user manually links them, (B) permit explicit user-confirmed links from trusted source identities without automatically proposing matches, or (C) suggest possible links from authorized source context but require review before combining their attributed beliefs, subject to the existing identity-resolution restrictions?

#### Decision 1.4.11 — Suggest cross-source person links with mandatory human confirmation

**Answers:** 1.4.10  
**Approved by owner:** 2026-10-13, option C, explicitly requesting relaxation of existing identity-resolution restrictions.

When two permitted sources appear to refer to the **same person**, Belief Lab should offer **reviewable suggestions** for linking the source-scoped person records. Show the grounds and uncertainty for each suggested link. A potential match is *not* an established shared identity and must **not automatically combine the persons' attributed beliefs**. Only a user's explicit confirmation establishes the cross-source person link for use in the investigation. The user can reject or undo a link, and Belief Lab preserves the original source identities, source/producer revisions, attribution evidence and historical link decisions rather than silently rewriting earlier records.

The owner wants to **relax the previous blanket product restriction** that would prevent useful cross-source identification of a person, including cases where the same real-world author or speaker appears in several sources. This is a **change of product intent**, not a claim that the current code or policy already supports it. The existing `belief-policy` and `AGENTS.md` limitations still govern running software until an explicitly reviewed implementation updates the authorization boundaries. Cross-source joining, person linking, and any optional source evidence must remain separately checked; an `.env` override cannot bypass deployed hard policy denials. This Decision does **not** grant unreviewed automatic identity merges, biometric identification, sensitive-trait inference, or unbounded profiling; their scope and authorization have not been approved.

After an owner-approved link, relevant person-belief attributions may be considered together **only as evidence that these source records refer to that person**, keeping `confidence (observation)`, `confidence (person)` and `conviction` distinct as approved by Decision 1.4.7, and applying self-reported versus inferred conviction distinctions from Decision 1.4.9. Correlated observations remain correlated; resolving an identity does not establish the proposition's truth. Existing Decisions about separate investigations still apply; no global merging of investigations is implied.

**Implementation distinction:** A future issue must independently verify the proposal → source inspection → user confirmation/rejection → link/unlink → attribution history flow in the real workbench, including lookalike names and conflicting evidence. A separately reviewed policy/domain change is required for any formerly prohibited cross-source real-person operation. Do not treat this documentation-only decision as a live policy relaxation.

#### Question 1.4.12 — Evidence allowed for matching people across sources

Which evidence should Belief Lab use to **suggest** that two source-local person records refer to the same individual: (A) explicit stable author/person identifiers only; (B) identifiers plus non-biometric contextual evidence such as names, affiliations and source relationships; or (C) also face or voice similarity where a producer supplies it, subject to separately reviewed permission and human confirmation? None of these permits automatic linking without review.

#### Decision 1.4.13 — Suggest person matches using identifiers and non-biometric context

**Answers:** 1.4.12  
**Approved by owner:** 2026-10-13, option B.

For source-local person records that may refer to the same person, Belief Lab can propose cross-source links using **explicit identifiers and non-biometric contextual evidence**, such as names, author profiles, affiliations, source relationships, publication context or other legitimately available textual metadata. Matching names or occupations are only clues, not sufficient proof that two person records represent the same individual.

For every suggested link, the investigator can inspect the original person records and the particular evidence supporting the proposal, including source/producer revisions, possible conflicting identifiers, and uncertainty. Belief Lab must **not merge identities or their attributed beliefs automatically**. The user explicitly confirms or rejects each link, can undo confirmed links, and can later inspect why an identity connection was made. Confirmed links retain their original source-scoped record identities and do not erase historical belief-attribution evidence.

**No biometric matching** is authorized as a person-link suggestion method by this Decision: do not use facial similarity, voiceprints or face/voice matching as an alternative to non-biometric textual/metadata evidence. The product direction in Decision 1.4.11 allows useful user-reviewed cross-source person linking, but the presently enforced `belief-policy` and `AGENTS.md` hard denials remain effective until separately reviewed code/policy changes. An environment setting alone must not enable prohibited operations. No sensitive-trait inference, unreviewed real-world identification, or unrestricted mass profiling is approved.

After confirmation, `confidence (observation)`, `confidence (person)`, and `conviction` retain their independent provenance and meaning. A match proposal's evidence or confidence must not be substituted for confidence that the person holds a proposition. Matching people is not evidence that their propositions are true.

#### Question 1.4.14 — Reuse of confirmed person links across investigations

When the same person appears in different persistent investigations, should Belief Lab (A) keep their confirmed identity links entirely separate per investigation, (B) automatically reuse confirmed links and person identities across every investigation, or (C) keep investigations independent by default but let users explicitly transfer or reuse approved person links between them?
