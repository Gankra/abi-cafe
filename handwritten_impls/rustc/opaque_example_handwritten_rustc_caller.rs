include!("../../harness/rust_test_prefix.rs");

#[repr(C)]
pub struct MyStruct<'a> {
    field0: u64,
    field1: Option<&'a u32>,
}

extern "C" {
    fn i_am_opaque_to_the_test_harness(arg0: u64, arg1: &MyStruct, arg2: MyStruct) -> bool;
}

#[no_mangle]
pub extern fn do_test() {
    unsafe {
        let arg0 = 0x1234_5678_9876_5432u64;
        WRITE_FIELD.unwrap()(CALLER_INPUTS, &arg0 as *const _ as *const _, core::mem::size_of_val(&arg0) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);

        let temp1 = 0xa8f0_ed12u32;
        let arg1 = MyStruct { field0: 0xaf3e_3628_b800_cd32, field1: Some(&temp1) };
        WRITE_FIELD.unwrap()(CALLER_INPUTS, &arg1.field0 as *const _ as *const _, core::mem::size_of_val(&arg1.field0) as u32);
        WRITE_FIELD.unwrap()(CALLER_INPUTS, &arg1.field1 as *const _ as *const _, core::mem::size_of_val(&arg1.field1) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);

        let arg2 = MyStruct { field0: 0xbe10_2623_e810_ad39, field1: None };
        WRITE_FIELD.unwrap()(CALLER_INPUTS, &arg2.field0 as *const _ as *const _, core::mem::size_of_val(&arg2.field0) as u32);
        WRITE_FIELD.unwrap()(CALLER_INPUTS, &arg2.field1 as *const _ as *const _, core::mem::size_of_val(&arg2.field1) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);

        let output = i_am_opaque_to_the_test_harness(arg0, &arg1, arg2);

        WRITE_FIELD.unwrap()(CALLER_OUTPUTS, &output as *const _ as *const _, core::mem::size_of_val(&output) as u32);
        FINISHED_VAL.unwrap()(CALLER_OUTPUTS);

        FINISHED_FUNC.unwrap()(CALLER_INPUTS, CALLER_OUTPUTS);
    }
}