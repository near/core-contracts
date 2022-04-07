#![cfg_attr(target_arch = "wasm32", no_std)]
#![cfg_attr(target_arch = "wasm32", feature(alloc_error_handler))]

#[macro_use]
extern crate alloc;

use alloc::vec::Vec;

const ATOMIC_OP_REGISTER: u64 = 0;
const EVICTED_REGISTER: u64 = 8;

#[cfg(all(not(feature = "clean"), not(feature = "replace")))]
core::compile_error!("one of the `clean` or `replace` features must be set");

// Set up global allocator by default if in wasm32 architecture.
#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[cfg(target_arch = "wasm32")]
#[alloc_error_handler]
fn oom(_: core::alloc::Layout) -> ! {
    core::arch::wasm32::unreachable()
}

// Update panic handler in wasm32 environments
#[cfg(all(target_arch = "wasm32", not(feature = "std")))]
#[panic_handler]
#[allow(unused_variables)]
fn panic(info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

fn register_len(register_id: u64) -> Option<u64> {
    let len = unsafe { near_sys::register_len(register_id) };
    if len == core::u64::MAX {
        None
    } else {
        Some(len)
    }
}

#[cfg(feature = "replace")]
/// Writes key-value into storage.
fn storage_write(key: &[u8], value: &[u8]) {
    unsafe {
        near_sys::storage_write(
            key.len() as _,
            key.as_ptr() as _,
            value.len() as _,
            value.as_ptr() as _,
            EVICTED_REGISTER,
        )
    };
}

#[cfg(feature = "clean")]
/// Removes storage at given key.
fn storage_remove(key: &[u8]) {
    unsafe { near_sys::storage_remove(key.len() as _, key.as_ptr() as _, EVICTED_REGISTER) };
}

fn input() -> Option<Vec<u8>> {
    unsafe { near_sys::input(ATOMIC_OP_REGISTER) };
    let len = register_len(ATOMIC_OP_REGISTER)?;

    let buffer = vec![0u8; len as usize];

    // Read data from register into buffer
    unsafe { near_sys::read_register(ATOMIC_OP_REGISTER, buffer.as_ptr() as _) };

    Some(buffer)
}

#[cfg(feature = "replace")]
#[no_mangle]
pub fn replace() {
    #[derive(serde::Deserialize)]
    struct ReplaceInput<'a> {
        #[serde(borrow)]
        entries: Vec<(&'a str, &'a str)>,
    }

    let input = input().unwrap();
    let args: ReplaceInput = serde_json::from_slice(&input).unwrap();
    for (key, value) in args.entries {
        storage_write(
            &base64::decode(key).unwrap(),
            &base64::decode(value).unwrap(),
        );
    }
}

#[cfg(feature = "clean")]
#[no_mangle]
pub fn clean() {
    #[derive(serde::Deserialize)]
    struct CleanInput<'a> {
        #[serde(borrow)]
        keys: Vec<&'a str>,
    }

    let input = input().unwrap();
    let args: CleanInput = serde_json::from_slice(&input).unwrap();
    for key in args.keys {
        storage_remove(&base64::decode(key).unwrap());
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use arbitrary::{Arbitrary, Unstructured};
    use core::iter::Iterator;
    use rand::{Rng, RngCore, SeedableRng};
    use rand_xorshift::XorShiftRng;
    use std::collections::BTreeMap;
    use tokio::fs;
    use workspaces::{prelude::*, Contract, DevNetwork, Worker};

    const BUFFER_SIZE: usize = 8192;

    // Prop test that values can be replaced and cleaned
    async fn prop(
        worker: &Worker<impl DevNetwork>,
        contract: &Contract,
        bytes: &BTreeMap<Vec<u8>, Vec<u8>>,
    ) -> anyhow::Result<()> {
        let b64_bytes: Vec<(_, _)> = bytes
            .iter()
            .map(|(a, b)| (base64::encode(a), base64::encode(b)))
            .collect();

        // Replace generated keys and values
        contract
            .call(&worker, "replace")
            .args_json(&serde_json::json!({ "entries": &b64_bytes }))?
            .max_gas()
            .transact()
            .await?;

        // Check that state items passed in are in state
        let state_items = contract.view_state(worker, None).await?;
        for (k, v) in bytes {
            assert_eq!(state_items.get(k).unwrap(), v);
        }

        let keys: Vec<_> = b64_bytes.iter().map(|(k, _)| k.as_str()).collect();

        contract
            .call(&worker, "clean")
            .args_json(&serde_json::json!({ "keys": &keys }))?
            .max_gas()
            .transact()
            .await?;

        let state_items = contract.view_state(worker, None).await?;

        assert!(state_items.is_empty());

        Ok(())
    }

    fn generate_n_elements(
        n: usize,
        rng: &mut XorShiftRng,
        mut buf: &mut Vec<u8>,
    ) -> BTreeMap<Vec<u8>, Vec<u8>> {
        let mut result = BTreeMap::default();
        while result.len() < n {
            rng.fill_bytes(&mut buf);
            let mut u = Unstructured::new(&buf[0..(rng.gen::<usize>() % BUFFER_SIZE)]);
            result.extend(BTreeMap::<Vec<u8>, Vec<u8>>::arbitrary(&mut u).unwrap());
        }
        result
    }

    #[tokio::test]
    async fn workspaces_test() -> anyhow::Result<()> {
        let wasm = fs::read("res/state_manipulation.wasm").await?;

        let worker = workspaces::sandbox().await?;

        let contract = worker.dev_deploy(&wasm).await?;

        for i in [8, 64, 256, 1024] {
            let mut rng = XorShiftRng::seed_from_u64(8);
            let mut buf = vec![0; BUFFER_SIZE];
            let key_values = generate_n_elements(i, &mut rng, &mut buf);
            prop(&worker, &contract, &key_values).await?;
        }

        Ok(())
    }
}
