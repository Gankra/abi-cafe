include!("../../harness/rust_test_prefix.rs");

extern "C" {
    fn i_am_opaque_to_the_test_harness(input: u64) -> u64;
}

#[no_mangle]
pub extern fn do_test() {
    unsafe {
        let val = 1234_5678_9876_5432u64;
        // println!("caller inputs: ");
        // println!("{val}");
        WRITE.unwrap()(CALLER_INPUTS, &val as *const _ as *const _, core::mem::size_of_val(&val) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        println!();

        let result = i_am_opaque_to_the_test_harness(val);

        // println!("caller outputs: ");
        // println!("{result}");
        WRITE.unwrap()(CALLER_OUTPUTS, &result as *const _ as *const _, core::mem::size_of_val(&result) as u32);
        FINISHED_VAL.unwrap()(CALLER_OUTPUTS);
        // println!();
        FINISHED_FUNC.unwrap()(CALLER_INPUTS, CALLER_OUTPUTS);
    }
}