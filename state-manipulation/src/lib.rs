#![cfg_attr(target_arch = "wasm32", no_std)]
#![cfg_attr(target_arch = "wasm32", feature(alloc_error_handler))]

#[macro_use]
extern crate alloc;

use alloc::vec::Vec;

const ATOMIC_OP_REGISTER: u64 = 0;
const EVICTED_REGISTER: u64 = 8;

#[cfg(all(not(feature = "cleanup"), not(feature = "replace")))]
core::compile_error!("one of the `cleanup` or `replace` features must be set");

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

#[cfg(feature = "cleanup")]
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
    let input = input().unwrap();
    let args: Vec<(&str, &str)> = serde_json::from_slice(&input).unwrap();
    for (key, value) in args {
        storage_write(
            &base64::decode(key).unwrap(),
            &base64::decode(value).unwrap(),
        );
    }
}

#[cfg(feature = "cleanup")]
#[no_mangle]
pub fn clean() {
    let input = input().unwrap();
    let args: Vec<&str> = serde_json::from_slice(&input).unwrap();
    for key in args {
        storage_remove(&base64::decode(key).unwrap());
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use arbitrary::{Arbitrary, Unstructured};
    use rand::{Rng, RngCore, SeedableRng};
    use rand_xorshift::XorShiftRng;
    use tokio::fs;
    use workspaces::{prelude::*, Contract, DevNetwork, Worker};

    const BUFFER_SIZE: usize = 4096;

    // Prop test that values can be replaced and cleaned
    async fn prop(
        worker: &Worker<impl DevNetwork>,
        contract: &Contract,
        bytes: &[(Vec<u8>, Vec<u8>)],
    ) -> anyhow::Result<()> {
        let b64_bytes: Vec<_> = bytes
            .iter()
            .map(|(a, b)| (base64::encode(a), base64::encode(b)))
            .collect();

        // Replace generated keys and values
        contract
            .call(&worker, "replace")
            .args_json(&b64_bytes)?
            .transact()
            .await?;

        // TODO workspaces hasn't released functional state viewing yet -- verify state is
        // TODO     equivalent to the bytes above with 0.2 released
        // let mut state_items = worker
        //     .view_state(contract.as_account().id().clone(), None)
        //     .await?;

        let keys: Vec<_> = b64_bytes.iter().map(|(k, _)| k.as_str()).collect();

        contract
            .call(&worker, "clean")
            .args_json(keys)?
            .transact()
            .await?;

        let state_items = worker.view_state(contract.id().clone(), None).await?;

        assert!(state_items.is_empty());

        Ok(())
    }

    fn generate_n_elements(
        n: usize,
        rng: &mut XorShiftRng,
        mut buf: &mut Vec<u8>,
    ) -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut result = Vec::new();
        while result.len() < n {
            rng.fill_bytes(&mut buf);
            let mut u = Unstructured::new(&buf[0..(rng.gen::<usize>() % BUFFER_SIZE)]);
            result.append(&mut Vec::<(Vec<u8>, Vec<u8>)>::arbitrary(&mut u).unwrap());
        }
        result.truncate(n);
        result
    }

    #[tokio::test]
    async fn workspaces_test() -> anyhow::Result<()> {
        let wasm = fs::read("res/state_manipulation.wasm").await?;

        let worker = workspaces::sandbox();

        let contract = worker.dev_deploy(wasm).await?;

        // TODO add more when workspaces more efficient (slow test)
        for i in [8, 16] {
            let mut rng = XorShiftRng::seed_from_u64(8);
            let mut buf = vec![0; BUFFER_SIZE];
            let key_values = generate_n_elements(i, &mut rng, &mut buf);
            prop(&worker, &contract, &key_values).await?;
        }

        Ok(())
    }
}
