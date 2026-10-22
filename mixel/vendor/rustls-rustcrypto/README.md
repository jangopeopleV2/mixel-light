# rustls-rustcrypto

[![crate][crate-image]][crate-link]
[![Docs][docs-image]][docs-link]
[![Build Status][build-image]][build-link]
![Apache2/MIT licensed][license-image]
![Rust Version][rustc-image]
[![Project Chat][chat-image]][chat-link]

[RustCrypto]-based provider implementation for version 0.23 of [rustls], maintained by the RustCrypto organization.

## ⚠️USE THIS AT YOUR OWN RISK! DO NOT USE THIS IN PRODUCTION⚠️

Not only that this is incomplete that only few selected TLS suites implemented (it should be well enough to cover 70% of the usage), but the elephant in the room is that neither did rustls nor RustCrypto packages were formally verified and certified with FIPS compliance.

This local compatibility variant uses ring 0.17.14 for RSA private-key signing and public-key verification, avoiding the unpatched RustCrypto rsa advisory RUSTSEC-2023-0071. All other algorithms remain RustCrypto. The existing alloc, std, and TLS feature names are retained; ring adds C/assembly build requirements. RSA public verification accepts 2048–4096-bit keys, preserving the prior public-key upper limit. Private imports use ring's validated two-prime PKCS#1 and unencrypted PKCS#8 v1 parsers with their documented modulus/exponent limits.

## Supported Cipher Suites

- TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256
- TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384
- TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256
- TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256
- TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384
- TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256
- TLS13_AES_128_GCM_SHA256
- TLS13_AES_256_GCM_SHA384
- TLS13_CHACHA20_POLY1305_SHA256

## License

Licensed under either of:

- [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
- [MIT license](http://opensource.org/licenses/MIT)

at your option.

Some code authored by [@ctz](https://github.com/ctz) was adapted from upstream rustls. Licensed as above with permission.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[//]: # (badges)

[crate-image]: https://buildstats.info/crate/rustls-rustcrypto
[crate-link]: https://crates.io/crates/rustls-rustcrypto
[docs-image]: https://docs.rs/rustls-rustcrypto/badge.svg
[docs-link]: https://docs.rs/rustls-rustcrypto/
[build-image]: https://github.com/RustCrypto/rustls-rustcrypto/actions/workflows/rustls-rustcrypto.yml/badge.svg
[build-link]: https://github.com/RustCrypto/rustls-rustcrypto/actions/workflows/rustls-rustcrypto.yml
[license-image]: https://img.shields.io/badge/license-Apache2.0/MIT-blue.svg
[rustc-image]: https://img.shields.io/badge/rustc-1.75+-blue.svg
[chat-image]: https://img.shields.io/badge/zulip-join_chat-blue.svg
[chat-link]: https://rustcrypto.zulipchat.com/#narrow/stream/434751-TLS

[//]: # (links)

[RustCrypto]: https://github.com/RustCrypto/
[rustls]: https://github.com/rustls/rustls/
