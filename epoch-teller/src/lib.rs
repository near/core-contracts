#![no_std]
#![feature(core_intrinsics)]
#![allow(non_snake_case)]

#[panic_handler]
#[no_mangle]
pub fn on_panic(_info: &::core::panic::PanicInfo) -> ! {
    unsafe {
        ::core::intrinsics::abort();
    }
}

// #[global_allocator]
// static ALLOC: near_sdk::wee_alloc::WeeAlloc<'_> = near_sdk::wee_alloc::WeeAlloc::INIT;

#[allow(unused)]
extern "C" {
    fn epoch_height() -> u64;
    fn value_return(value_len: u64, value_ptr: u64);
}

#[no_mangle]
pub unsafe fn e() {
    let epoch = epoch_height();
    let a = [b'"', b'0' + ((epoch / 100) % 10) as u8, b'0' + ((epoch / 10) % 10) as u8,  b'0' + (epoch % 10) as u8, b'"'];
    value_return(a.len() as _, a.as_ptr() as _);
}
