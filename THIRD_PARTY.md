# Third-party components

## SemIf

Belief Lab can invoke an external SemIf checkout through `semif-provider`.

- Project: `TheoLeeCJ/SemIf`
- Pinned source revision: `1f2dea3e25379f9dfc98cb83c324f00ab5deda37`
- License: MIT
- Copyright: Copyright (c) 2026 TheoLeeCJ

Belief Lab does not copy SemIf source into this repository. The `belief-semif bootstrap` command clones the upstream project at the pinned revision, preserving its own license and notices.

SemIf is independent of Jev and TypeSafe. Belief Lab does not describe the adapter as an official Jev implementation or as reproducing Jev's undisclosed model or training.

## Model artifacts

`belief-semif download-model` can fetch the GGUF artifacts listed by `belief-semif catalog`. The files remain external and are not redistributed by Belief Lab. Each artifact is addressed by an immutable upstream revision and exact SHA-256 identity; upstream model/runtime licenses and terms apply.

| Tier | Artifact | Upstream revision | SHA-256 |
| --- | --- | --- | --- |
| phone | `Qwen/Qwen3-0.6B-GGUF / Qwen3-0.6B-Q8_0.gguf` | `23749fefcc72300e3a2ad315e1317431b06b590a` | `9465e63a22add5354d9bb4b99e90117043c7124007664907259bd16d043bb031` |
| desktop | `openbmb/MiniCPM5-2B-GGUF / MiniCPM5-2B-Q4_K_M.gguf` | `2079a22f3beaa4e306449978533478fe0522f4b3` | `ec2d5801640099e97d8d7e8003ad4d81f336e757811f03a26173dddf386602fd` |
| high-memory | `bartowski/Qwen_Qwen3.5-4B-GGUF / Qwen_Qwen3.5-4B-Q4_K_M.gguf` | `4168f45a16a1290d65a4ec0fa312ae917a4c15d6` | `13c16f426047e2de38cd075bdade4a7bcbc8c774384876f677740cda65f8a983` |
