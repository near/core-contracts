#![no_std]
#![feature(core_intrinsics)]
#![allow(non_snake_case)]

#[panic_handler]
#[no_mangle]
pub fn on_panic(_info: &::core::panic::PanicInfo) -> ! {
    ::core::intrinsics::abort();
}

// #[global_allocator]
// static ALLOC: near_sdk::wee_alloc::WeeAlloc<'_> = near_sdk::wee_alloc::WeeAlloc::INIT;

const SUPPLY_KEY: &[u8] = b"S";
const LEN: u64 = 32;
const LEN_U64: u64 = 4;
const LEN_U64_USIZE: usize = LEN_U64 as _;

type U256 = [u64];

#[allow(unused)]
extern "C" {
    // #############
    // # Registers #
    // #############
    fn read_register(register_id: u64, ptr: u64);
    fn register_len(register_id: u64) -> u64;
    fn write_register(register_id: u64, data_len: u64, data_ptr: u64);
    fn panic();
    // ###############
    // # Context API #
    // ###############
    fn predecessor_account_id(register_id: u64);
    fn input(register_id: u64);

    fn sha256(value_len: u64, value_ptr: u64, register_id: u64);

    fn value_return(value_len: u64, value_ptr: u64);

    fn storage_write(
        key_len: u64,
        key_ptr: u64,
        value_len: u64,
        value_ptr: u64,
        register_id: u64,
    ) -> u64;
    fn storage_read(key_len: u64, key_ptr: u64, register_id: u64) -> u64;
    fn storage_remove(key_len: u64, key_ptr: u64, register_id: u64) -> u64;
    fn storage_has_key(key_len: u64, key_ptr: u64) -> u64;
}

unsafe fn add(a: &U256, b: &U256, res: &mut U256) {
    let mut old_overflow = false;
    for i in 0..LEN_U64_USIZE {
        let (v, overflow) = a[i].overflowing_add(b[i]);
        let (v, overflow2) = v.overflowing_add(old_overflow as u64);
        old_overflow = overflow || overflow2;
        res[i] = v
    }
    if old_overflow {
        // Overflow
        panic();
    }
}

unsafe fn sub(a: &U256, b: &U256, res: &mut U256) {
    let mut old_underflow = false;
    for i in 0..LEN_U64_USIZE {
        let (v, underflow) = a[i].overflowing_sub(b[i]);
        let (v, underflow2) = v.overflowing_sub(old_underflow as u64);
        old_underflow = underflow2 || underflow;
        res[i] = v
    }
    if old_underflow {
        // Underflow
        panic();
    }
}

/// Initializes the token contract with the total supply given to the owner.
/// Arguments (64 bytes):
/// - 0..32 - `sha256` of the owner address.
/// - 32..64 - U256 of the total supply
#[no_mangle]
pub unsafe fn init() {
    if storage_has_key(SUPPLY_KEY.len() as _, SUPPLY_KEY.as_ptr() as _) == 1 {
        panic();
    }
    let buf = read_input();
    // SUPPLY_KEY
    storage_write(
        SUPPLY_KEY.len() as _,
        SUPPLY_KEY.as_ptr() as _,
        LEN,
        buf.as_ptr() as u64 + LEN,
        0,
    );
    // OWNER BALANCE
    storage_write(LEN, buf.as_ptr() as _, LEN, buf.as_ptr() as u64 + LEN, 0);
}

unsafe fn read_input() -> [u64; LEN_U64_USIZE * 2] {
    input(0);
    let input_len = register_len(0);
    if input_len != LEN * 2 {
        panic();
    }
    let buf = [0u64; LEN_U64_USIZE * 2];
    read_register(0, buf.as_ptr() as _);
    buf
}

/// Transfer the amount from the `sha256(predecessor_account_id)` to the new receiver address.
/// Arguments (64 bytes):
/// - 0..32 - `sha256` of the receiver address.
/// - 32..64 - U256 is transfer amount
#[no_mangle]
pub unsafe fn transfer() {
    let buf = read_input();
    // Read hash of owner's account ID to register 0
    predecessor_account_id(0);
    sha256(u64::MAX, 0, 0);

    // Owner's balance to register 1
    if storage_read(u64::MAX, 0, 1) == 0 {
        // No balance
        panic();
    }

    let owner_balance = [0u64; LEN_U64_USIZE];
    read_register(1, owner_balance.as_ptr() as _);

    let transfer_balance = &buf[LEN_U64_USIZE..LEN_U64_USIZE * 2];
    let mut new_balance = [0u64; LEN_U64_USIZE];
    sub(&owner_balance, transfer_balance, &mut new_balance);

    // Write new owner balance
    storage_write(u64::MAX, 0, LEN, new_balance.as_ptr() as _, 1);

    let receiver_balance = [0u64; LEN_U64_USIZE];
    // Reading and filling receiver_balance.
    if storage_read(LEN, buf.as_ptr() as u64, 1) == 1 {
        read_register(1, receiver_balance.as_ptr() as _);
    }

    // Reusing `new_balance`, since it overwrites all bytes.
    add(&receiver_balance, transfer_balance, &mut new_balance);

    // Writing new owner balance
    storage_write(LEN, buf.as_ptr() as u64, LEN, new_balance.as_ptr() as _, 1);
}

/// Returns the balance of the given address.
/// Arguments (64 bytes):
/// - 0..32 - `sha256` of the address to check the balance.
#[no_mangle]
pub unsafe fn get_balance() {
    input(0);
    let input_len = register_len(0);
    if input_len != LEN {
        panic();
    }

    // Reading receiver_balance and returning it, or returning 0.
    if storage_read(u64::MAX, 0, 1) == 1 {
        value_return(u64::MAX, 1);
    } else {
        let receiver_balance = [0u64; LEN_U64_USIZE];
        value_return(LEN, receiver_balance.as_ptr() as u64);
    }
}
