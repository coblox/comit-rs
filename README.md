# COMIT-rs

[![CircleCI](https://circleci.com/gh/comit-network/comit-rs.svg?style=svg)](https://circleci.com/gh/comit-network/comit-rs)
[![Safety Dance](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
[![Bors enabled](https://bors.tech/images/badge_small.svg)](https://app.bors.tech/repositories/20717)
[![Gitter chat](https://badges.gitter.im/gitterHQ/gitter.png)](https://gitter.im/comit-network/community)

COMIT is an open protocol facilitating trustless cross-blockchain applications.
This is a reference implementation for the COMIT protocol. 

## DISCLAIMER: This is not mainnet ready!

- Extensive testing on mainnet from our side still remains to be done
- Several features for achieving production-ready robustness are still under works
- Once we consider comit-rs production ready, we will release version 1.0.0

## Structure

This repository is a cargo workspace:

- Crates at the top level are where the main work happens. Consult the respective `Cargo.toml` for a description of what they do.  
- Crates inside the [internal](./internal) folder are considered to be private to this repository. They are used for sharing code between other crates in this repository.
There is an [ongoing effort](https://github.com/comit-network/comit-rs/issues/626) to get rid of these crates because path dependencies block us from releasing any of the other crates to crates.io.
- `libp2p-comit`: implementation of the comit messaging protocol on top of libp2p


## Setup build environment

All you need is ~love~ rust: `curl https://sh.rustup.rs -sSf | sh` 

## Build & Run

1. `cargo install --path .`
2. startup bitcoin node with `-regtest` and `-rest`
3. startup ethereum node with JSON-RPC interface exposed at `localhost:8545`
4. startup cnd: `cnd` (or `./target/release/cnd` if you do not have the `~/.cargo/bin` folder in your `$PATH`)

Keep in mind that in order to do a swap locally you will need to start two instances of cnd.
Check out `cnd --help` for options.

## Setup testing/dev environment

1. Install `docker` & `docker-compose`
2. Install `node` (check the version required in api_tests/package.json) & `yarn`
3. Install Rust `nightly-2019-07-31`: `rustup install nightly-2019-07-31` (this one is only used for `rustfmt`)
4. Install `rustfmt` for `nightly-2019-07-31`: `rustup component add rustfmt --toolchain nightly-2019-07-31`
5. Install `cargo-make`: `cargo install cargo-make`
6. Run `cargo make` in the root folder of the repository, this will install various crates & tools such as clippy
   
## Testing

- `cargo make` runs the whole test suite including integration tests but not end-to-end.
- `cargo make all` also runs the whole test suite, including end-to-end tests.
- `cargo make format` to format Rust code
- `cargo make ts-fix` to format Typescript code
- `cargo make dry` to run cnd dry tests
- `cargo make api` to run all API tests
- `cargo make e2e` to run cnd end-to-end tests
- `cargo make e2e *btc*` to run cnd end-to-end tests with `btc` in the folder name (supports shell glob on the name)

Alternatively, you can run the end-to-end tests and TypeScript related actions using `yarn` (careful! It does not recompile Rust for you):
- `yarn run test`: run all tests
- `yarn run test <directory>`: run all tests in the directory
- `yarn run test <path to test file>`: run all tests in this test file, supports shell glob on the path
- `yarn run fix`: run prettier and linter to fix format
- `yarn run check`: run tsc (to check validity of TypeScript code) and verify format

## Contributing

Contributions are welcome, please visit [CONTRIBUTING](CONTRIBUTING.md) for more details.

If you have any question please [reach out to the team in our Gitter chat](https://gitter.im/comit-network/community)!

## License

This project is licensed under the terms of the [GNU GENERAL PUBLIC LICENSE v3](LICENSE.md).
