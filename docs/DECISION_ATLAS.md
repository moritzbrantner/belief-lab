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
