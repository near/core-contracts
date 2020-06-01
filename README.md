# Initial contracts

- [Staking Pool / Delegation contract](./staking-pool/)
- [Lockup / Vesting contract](./lockup/)
- [Whitelist Contract](./whitelist/)
- [Staking Pool Factory](./staking-pool-factory/)
- [Multisig contract](./multisig/)

**Note**: observe the usage of the file `rust-toolchain` in the project root. This file contains toolchain information for nightly, while the `build.sh` scripts in respective contract subdirectories may override this with `cargo +stable`. Refer to the documentation on [the toolchain file and override precedence](https://github.com/rust-lang/rustup#the-toolchain-file). Keep in mind that the build scripts may use `stable` while `cargo test` may use nightly.