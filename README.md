# Klause

[![Crates.io](https://img.shields.io/crates/v/klause.svg)](https://crates.io/crates/klause)
[![Docs.rs](https://docs.rs/klause/badge.svg)](https://docs.rs/klause)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

<!-- TODO: One-line description of what Klause does and who it is for. -->
A short, one-sentence description of the library.

## Overview

<!-- TODO: Explain the problem Klause solves, its main ideas, and when to use it. -->
Klause is a Rust library that _does something useful_. It is designed to be
small, fast, and easy to embed in other projects.

- **Feature one** — what makes it valuable.
- **Feature two** — what makes it valuable.
- **Feature three** — what makes it valuable.

## Installation

Add Klause to your `Cargo.toml`:

```toml
[dependencies]
klause = "0.1.0"
```

Or with [cargo-add](https://github.com/rust-lang/cargo):

```sh
cargo add klause
```

## Usage

<!-- TODO: Replace with a real, runnable example. -->
```rust
use klause::Klause;

fn main() {
    let klause = Klause::new();
    println!("{klause:?}");
}
```

For more examples, see the [`examples/`](./examples) directory and the
[API documentation](https://docs.rs/klause).

## Feature flags

<!-- TODO: Document or remove this section if the crate has no optional features. -->

| Feature | Default | Description |
| ------- | ------- | ----------- |
| `std`   | yes     | Enables the standard library. Disable for `no_std` support. |
| `serde` | no      | Implements `Serialize` and `Deserialize` for public types. |

## Minimum supported Rust version (MSRV)

<!-- TODO: Set the MSRV and keep it in sync with rust-version in Cargo.toml. -->
Klause requires Rust 1.85 or newer.

## Contributing

Contributions are welcome! Please open an issue to discuss a change before
submitting a pull request, and make sure the following pass locally:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as below, without any additional terms or conditions.

## License

Licensed under either of

- [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
- [MIT license](http://opensource.org/licenses/MIT)

at your option.

<!-- TODO: Add LICENSE-APACHE and LICENSE-MIT files, or update this section to
     match the license you choose. -->
