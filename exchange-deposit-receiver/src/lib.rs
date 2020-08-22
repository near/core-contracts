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

#[no_mangle]
pub fn exchange_deposit() {}
