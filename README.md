# protobuf-build


[![Crates.io][cr-svg]][cr-url]
[![Crates.io][cv-svg]][cr-url]
[![Docs.rs][dv-svg]][dv-url]
[![dependency status][dep-svg]][dep-rs]
[![Build Status][tv-svg]][tv-url]

 [cr-svg]: https://img.shields.io/crates/d/protobuf-build.svg
 [cr-url]: https://crates.io/crates/protobuf-build
 [cv-svg]: https://img.shields.io/crates/v/protobuf-build.svg
 [dv-svg]: https://docs.rs/protobuf-build/badge.svg
 [dv-url]: https://docs.rs/protobuf-build
 [tv-svg]: https://travis-ci.org/tikv/protobuf-build.svg?branch=master
 [tv-url]: https://travis-ci.org/tikv/protobuf-build
 [dep-rs]: https://deps.rs/repo/github/tikv/protobuf-build
 [dep-svg]: https://deps.rs/repo/github/tikv/protobuf-build/status.svg

Utility functions for generating Rust code from protobuf specifications.

Can use [rust-protobuf](https://github.com/stepancheg/rust-protobuf) and
[Prost](https://github.com/danburkert/prost).

The `prost-codec` feature uses Prost 0.14 and requires Rust 1.85 or later.
The `grpcio-prost-codec` feature uses the generator from `grpcio-compiler` and still targets Prost 0.11.

The default `protobuf-codec` feature cannot be combined with either Prost feature, because both
code paths define `Builder::generate_files`. Disable the default features when you use Prost:

```toml
[build-dependencies]
protobuf-build = { version = "0.16", default-features = false, features = ["prost-codec"] }
```

For gRPC Prost code, use `features = ["grpcio-prost-codec"]` instead. It enables `prost-codec` itself.
Do not combine standalone Prost 0.14 generation and gRPC Prost 0.11 generation in one Cargo build.
Cargo merges features across the dependency graph. If one crate enables `grpcio-prost-codec`, all
crates use the Prost 0.11 generator. Their enum accessors then call the deprecated `from_i32`, and
builds that deny warnings fail.
