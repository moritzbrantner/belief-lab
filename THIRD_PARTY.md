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

`belief-semif download-model` can fetch the GGUF artifacts listed by `belief-semif catalog`. The files remain external and are not redistributed by Belief Lab. Each artifact is addressed by an immutable upstream revision; upstream model/runtime licenses and terms apply.
