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
