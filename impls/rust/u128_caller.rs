include!("../../harness/rust_test_prefix.rs");

extern "C" {
    fn u128_by_val(input: u64) -> u64;
}

#[no_mangle]
pub extern fn do_test() {
    unsafe {
        let val = 1234_5678_9876_5432u64;
        println!("caller inputs: ");
        println!("{val}");
        WRITE.unwrap()(CALLER_INPUTS, &val as *const _ as *const _, core::mem::size_of_val(&val) as u32);
        println!();

        let result = u128_by_val(val);

        println!("caller outputs: ");
        println!("{result}");
        WRITE.unwrap()(CALLER_OUTPUTS, &result as *const _ as *const _, core::mem::size_of_val(&result) as u32);
        println!();
    }
}