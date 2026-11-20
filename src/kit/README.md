# Passive UI 0.1.0

Copied from Passive UI (https://github.com/PassiveDesign/passive-ui) by
its `scripts/vendor.mjs`. Do not edit these files here: change Passive UI,
then copy it in again, so every app stays on the same design language.

To update: clone Passive UI beside this repo and run
`node scripts/vendor.mjs --esm <path to this folder>` from it.

- `passive-ui.css`: tokens, fonts and the shared components.
- `passive-ui.mjs`: the timeline, as an ES module (`import { timeline } from "./kit/passive-ui.mjs"`), with types in `passive-ui.d.mts`.
- `tokens.json`: the tokens resolved per mode and accent.
- `fonts/`: IBM Plex Sans and JetBrains Mono, under the SIL Open Font
  License 1.1 (see the licence files and `fonts/README.md`).
