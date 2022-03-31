#![cfg_attr(target_arch = "wasm32", no_std)]
#![cfg_attr(target_arch = "wasm32", feature(alloc_error_handler))]

#[macro_use]
extern crate alloc;

use alloc::vec::Vec;

const ATOMIC_OP_REGISTER: u64 = 0;
const EVICTED_REGISTER: u64 = 8;

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

#[no_mangle]
pub extern "C" fn replace() {
    let input = input().unwrap();
    let stream = serde_json::Deserializer::from_slice(&input);
    for item in stream.into_iter() {
        let (key, value): (&str, &str) = item.unwrap();
        storage_write(
            &base64::decode(key).unwrap(),
            &base64::decode(value).unwrap(),
        );
    }
}

#[no_mangle]
pub extern "C" fn clean() {
    let input = input().unwrap();
    let stream = serde_json::Deserializer::from_slice(&input);
    for item in stream.into_iter() {
        let key: &str = item.unwrap();
        storage_remove(&base64::decode(key).unwrap());
    }
}
