# Default Chinese and Japanese UI fonts

MIXEL Light vendors exactly two unmodified OFL-1.1 faces from the release-pinned
`storytold/craft-fonts` revision `8dcdacd5153e64560d109541a47d806f26f048c0`:
BIZ UDPGothic Regular (Morisawa / the BIZ UDGothic Project Authors) and Noto Sans
CJK SC Regular (the Noto Project Authors, Google and Adobe).

Noto CJK is the explicit maintainer-approved exception in `AGENTS.md`; no Adobe
Source Han, Source Sans, Source Serif, Source Code or other Adobe font is included.
The font binaries and their complete OFL notices are byte-identical to that pin.
`PROVENANCE.json` records the immutable source URLs, authors' font names and
SHA-256 hashes. The original upstream sources are pinned in `fonts/manifest.txt`.

Native builds embed both faces by default. WASM embeds only the Japanese face;
`cargo xtask web` ships the Chinese face and its licence beside the WASM.
An explicit `CRAFT_FONTS_DIR` keeps its existing external-checkout behavior.
This small fork-local subset makes normal builds reproducible without downloading
fonts or relying on another checkout. New document fonts still belong upstream.
