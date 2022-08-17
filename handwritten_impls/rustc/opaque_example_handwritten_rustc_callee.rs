include!("../../harness/rust_test_prefix.rs");

#[repr(C)]
pub struct MyStruct<'a> {
    field0: u64,
    field1: Option<&'a u32>,
}

#[no_mangle]
pub unsafe extern fn i_am_opaque_to_the_test_harness(arg0: u64, arg1: &MyStruct, arg2: MyStruct) -> bool {
    unsafe {
        WRITE_FIELD.unwrap()(CALLEE_INPUTS, &arg0 as *const _ as *const _, core::mem::size_of_val(&arg0) as u32);
        FINISHED_VAL.unwrap()(CALLEE_INPUTS);

        WRITE_FIELD.unwrap()(CALLEE_INPUTS, &arg1.field0 as *const _ as *const _, core::mem::size_of_val(&arg1.field0) as u32);
        WRITE_FIELD.unwrap()(CALLEE_INPUTS, &arg1.field1 as *const _ as *const _, core::mem::size_of_val(&arg1.field1) as u32);
        FINISHED_VAL.unwrap()(CALLEE_INPUTS);

        WRITE_FIELD.unwrap()(CALLEE_INPUTS, &arg2.field0 as *const _ as *const _, core::mem::size_of_val(&arg2.field0) as u32);
        WRITE_FIELD.unwrap()(CALLEE_INPUTS, &arg2.field1 as *const _ as *const _, core::mem::size_of_val(&arg2.field1) as u32);
        FINISHED_VAL.unwrap()(CALLEE_INPUTS);

        let output = true;

        WRITE_FIELD.unwrap()(CALLEE_OUTPUTS, &output as *const _ as *const _, core::mem::size_of_val(&output) as u32);
        FINISHED_VAL.unwrap()(CALLEE_OUTPUTS);

        FINISHED_FUNC.unwrap()(CALLEE_INPUTS, CALLEE_OUTPUTS);
        
        return output;
    }
}