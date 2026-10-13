# Vendored block 0.1.6

Author: Steven Sheldon. Upstream declares the MIT licence in `Cargo.toml`.
The original README and Rust sources retain their upstream text and credits.

- Source: https://github.com/SSheldon/rust-block
- Release tag: `0.1.6`
- Release commit: `47178790cfc9d4a8b092051d8b413b78bd31254a`
- Published archive: https://static.crates.io/crates/block/block-0.1.6.crate
- Archive SHA-256: `0d8c1fef690941d3e7788d328517591fecc684c084084702d6ff1641e993699a`
- Original `src/lib.rs` SHA-256: `eb31678adf63b53109d9b94eba23699fd5f9ebfdb950f6e1a57ad51bb6a146fa`
- Patched `src/lib.rs` SHA-256: `bf799f4d01bb497fdcffe7a5e28d998e721ed45c1be866ed1b454df39ce876a9`
- Original `src/test_utils.rs` SHA-256: `271bd579b67e2440134227cf8ee52c1b8c22854eb0c7923b46306b51810c2cf1`
- Original `README.md` SHA-256: `01c56329fa14f48ea1c9c7986c3660c41beeff2477f61c2d36bd8e666b9fc562`
- Original `Cargo.toml` SHA-256: `24df2f33b48c9e756a4f864f882c1ab07a159b6c2b790f45af2e8162499b6719`

The source files in the published archive are byte-for-byte identical to the release
commit above. Official `master` at `642ea4a4a5853a21b55b05c34832a5f1bb1af61c`
still has the same empty `Class` enum.

## Local change

The production-source corrections replace the uninhabited `enum Class { }` with an
inhabited, zero-sized `#[repr(C)]` opaque struct. `_NSConcreteStackBlock` remains an
external symbol whose address is used as a block's `isa` pointer; no class value is
read, no layout is interpreted, and the public API and existing unsafe operations
are unchanged. All formerly implicit C ABI declarations now explicitly say `extern "C"`,
which preserves their original default calling convention and fixes Rust 1.99
`missing_abi` warnings without suppression. The opaque class correction removes the future-incompatible declaration identified by
https://github.com/rust-lang/rust/issues/74840 without suppressing its warning.

The local manifest explicitly retains the original `2015` edition, adds an isolated
`[workspace]`, and removes the dev-only
`path = "test_utils"`: that C/Objective-C test harness was excluded from the
published archive. Its existing registry version declaration is retained. MIXEL
verifies the block API from an independent Rust regression fixture instead.

The upstream repository and archive declare MIT but omit its text. `LICENSE-MIT`
ships the standard MIT permission text with the declared author; it is an explicit
licence-text reconstruction, not an upstream file claimed to have been copied.
