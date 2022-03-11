include!("../../harness/rust_test_prefix.rs");

#[repr(C, align(16))]
pub struct my_i128 {
    low: i64,
    high: i64,
}
#[repr(C)]
pub struct my_unaligned_i128 {
    low: i64,
    high: i64,
}

impl my_i128 {
    fn new(val: i128) -> Self {
        Self {
            low: val as u64 as i64, 
            high: (val as u128 >> 64) as u64 as i64,
        }
    }
}
impl my_unaligned_i128 {
    fn new(val: i128) -> Self {
        Self {
            low: val as u64 as i64, 
            high: (val as u128 >> 64) as u64 as i64,
        }
    }
}


extern {
    fn callee_native_layout(arg0: &i128, arg1: &my_i128, arg2: &my_unaligned_i128);
    fn callee_emulated_layout(arg0: &i128, arg1: &my_i128, arg2: &my_unaligned_i128);
    fn callee_unaligned_emulated_layout(arg0: &i128, arg1: &my_i128, arg2: &my_unaligned_i128);

    fn native_to_native(arg0: i128, arg1: i128, arg2: f32, arg3: i128, arg4: u8, arg5: i128);
    fn native_to_emulated(arg0: i128, arg1: i128, arg2: f32, arg3: i128, arg4: u8, arg5: i128);
    fn native_to_unaligned_emulated(arg0: i128, arg1: i128, arg2: f32, arg3: i128, arg4: u8, arg5: i128);

    fn emulated_to_native(arg0: my_i128, arg1: my_i128, arg2: f32, arg3: my_i128, arg4: u8, arg5: my_i128);
    fn emulated_to_emulated(arg0: my_i128, arg1: my_i128, arg2: f32, arg3: my_i128, arg4: u8, arg5: my_i128);
    fn emulated_to_unaligned_emulated(arg0: my_i128, arg1: my_i128, arg2: f32, arg3: my_i128, arg4: u8, arg5: my_i128);

    fn unaligned_emulated_to_native(arg0: my_unaligned_i128, arg1: my_unaligned_i128, arg2: f32, arg3: my_unaligned_i128, arg4: u8, arg5: my_unaligned_i128);
    fn unaligned_emulated_to_emulated(arg0: my_unaligned_i128, arg1: my_unaligned_i128, arg2: f32, arg3: my_unaligned_i128, arg4: u8, arg5: my_unaligned_i128);
    fn unaligned_emulated_to_unaligned_emulated(arg0: my_unaligned_i128, arg1: my_unaligned_i128, arg2: f32, arg3: my_unaligned_i128, arg4: u8, arg5: my_unaligned_i128);
}

#[no_mangle] pub extern "C" fn do_test() {
    unsafe {
        let arg0: i128 = 34784419711585546284254720952638769794;
        let arg1 = my_i128::new(34784419711585546284254720952638769794);
        let arg2 = my_unaligned_i128::new(34784419711585546284254720952638769794);

        WRITE.unwrap()(CALLER_INPUTS, &arg0 as *const _ as *const _, core::mem::size_of_val(&arg0) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg1 as *const _ as *const _, core::mem::size_of_val(&arg1) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg2 as *const _ as *const _, core::mem::size_of_val(&arg2) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);

        callee_native_layout(&arg0, &arg1, &arg2);

        FINISHED_FUNC.unwrap()(CALLER_INPUTS, CALLER_OUTPUTS);
    }
    unsafe {
        let arg0: i128 = 34784419711585546284254720952638769794;
        let arg1 = my_i128::new(34784419711585546284254720952638769794);
        let arg2 = my_unaligned_i128::new(34784419711585546284254720952638769794);

        WRITE.unwrap()(CALLER_INPUTS, &arg0 as *const _ as *const _, core::mem::size_of_val(&arg0) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg1 as *const _ as *const _, core::mem::size_of_val(&arg1) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg2 as *const _ as *const _, core::mem::size_of_val(&arg2) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);

        callee_emulated_layout(&arg0, &arg1, &arg2);

        FINISHED_FUNC.unwrap()(CALLER_INPUTS, CALLER_OUTPUTS);
    }
    unsafe {
        let arg0: i128 = 34784419711585546284254720952638769794;
        let arg1 = my_i128::new(34784419711585546284254720952638769794);
        let arg2 = my_unaligned_i128::new(34784419711585546284254720952638769794);

        WRITE.unwrap()(CALLER_INPUTS, &arg0 as *const _ as *const _, core::mem::size_of_val(&arg0) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg1 as *const _ as *const _, core::mem::size_of_val(&arg1) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg2 as *const _ as *const _, core::mem::size_of_val(&arg2) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);

        callee_unaligned_emulated_layout(&arg0, &arg1, &arg2);

        FINISHED_FUNC.unwrap()(CALLER_INPUTS, CALLER_OUTPUTS);
    }
    unsafe {
        // native -> native (expected fail)
        let arg0: i128 = 34784419711585546284254720952638769794;
        let arg1: i128 = 34784419711585546284254720952638769794;
        let arg2: f32 = 1234.456;
        let arg3: i128 = 34784419711585546284254720952638769794;
        let arg4: u8 = 235;
        let arg5: i128 = 34784419711585546284254720952638769794;

        WRITE.unwrap()(CALLER_INPUTS, &arg0 as *const _ as *const _, core::mem::size_of_val(&arg0) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg1 as *const _ as *const _, core::mem::size_of_val(&arg1) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg2 as *const _ as *const _, core::mem::size_of_val(&arg2) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg3 as *const _ as *const _, core::mem::size_of_val(&arg3) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg4 as *const _ as *const _, core::mem::size_of_val(&arg4) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg5 as *const _ as *const _, core::mem::size_of_val(&arg5) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);

        native_to_native(arg0, arg1, arg2, arg3, arg4, arg5, );

        FINISHED_FUNC.unwrap()(CALLER_INPUTS, CALLER_OUTPUTS);
    }
    unsafe {
        // native -> emulated (expected fail)
        let arg0: i128 = 34784419711585546284254720952638769794;
        let arg1: i128 = 34784419711585546284254720952638769794;
        let arg2: f32 = 1234.456;
        let arg3: i128 = 34784419711585546284254720952638769794;
        let arg4: u8 = 235;
        let arg5: i128 = 34784419711585546284254720952638769794;

        WRITE.unwrap()(CALLER_INPUTS, &arg0 as *const _ as *const _, core::mem::size_of_val(&arg0) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg1 as *const _ as *const _, core::mem::size_of_val(&arg1) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg2 as *const _ as *const _, core::mem::size_of_val(&arg2) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg3 as *const _ as *const _, core::mem::size_of_val(&arg3) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg4 as *const _ as *const _, core::mem::size_of_val(&arg4) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg5 as *const _ as *const _, core::mem::size_of_val(&arg5) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);

        native_to_emulated(arg0, arg1, arg2, arg3, arg4, arg5, );

        FINISHED_FUNC.unwrap()(CALLER_INPUTS, CALLER_OUTPUTS);
    }
    unsafe {
        // native -> unaligned_emulated (expected pass?)
        let arg0: i128 = 34784419711585546284254720952638769794;
        let arg1: i128 = 34784419711585546284254720952638769794;
        let arg2: f32 = 1234.456;
        let arg3: i128 = 34784419711585546284254720952638769794;
        let arg4: u8 = 235;
        let arg5: i128 = 34784419711585546284254720952638769794;

        WRITE.unwrap()(CALLER_INPUTS, &arg0 as *const _ as *const _, core::mem::size_of_val(&arg0) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg1 as *const _ as *const _, core::mem::size_of_val(&arg1) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg2 as *const _ as *const _, core::mem::size_of_val(&arg2) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg3 as *const _ as *const _, core::mem::size_of_val(&arg3) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg4 as *const _ as *const _, core::mem::size_of_val(&arg4) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg5 as *const _ as *const _, core::mem::size_of_val(&arg5) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);

        native_to_unaligned_emulated(arg0, arg1, arg2, arg3, arg4, arg5, );

        FINISHED_FUNC.unwrap()(CALLER_INPUTS, CALLER_OUTPUTS);
    }
    unsafe {
        // emulated -> native (expected fail?)
        let arg0 = my_i128::new(34784419711585546284254720952638769794);
        let arg1 = my_i128::new(34784419711585546284254720952638769794);
        let arg2: f32 = 1234.456;
        let arg3 = my_i128::new(34784419711585546284254720952638769794);
        let arg4: u8 = 235;
        let arg5 = my_i128::new(34784419711585546284254720952638769794);

        WRITE.unwrap()(CALLER_INPUTS, &arg0 as *const _ as *const _, core::mem::size_of_val(&arg0) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg1 as *const _ as *const _, core::mem::size_of_val(&arg1) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg2 as *const _ as *const _, core::mem::size_of_val(&arg2) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg3 as *const _ as *const _, core::mem::size_of_val(&arg3) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg4 as *const _ as *const _, core::mem::size_of_val(&arg4) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg5 as *const _ as *const _, core::mem::size_of_val(&arg5) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);

        emulated_to_native(arg0, arg1, arg2, arg3, arg4, arg5, );

        FINISHED_FUNC.unwrap()(CALLER_INPUTS, CALLER_OUTPUTS);
    }
    unsafe {
        // emulated -> emulated (expected pass?)
        let arg0 = my_i128::new(34784419711585546284254720952638769794);
        let arg1 = my_i128::new(34784419711585546284254720952638769794);
        let arg2: f32 = 1234.456;
        let arg3 = my_i128::new(34784419711585546284254720952638769794);
        let arg4: u8 = 235;
        let arg5 = my_i128::new(34784419711585546284254720952638769794);

        WRITE.unwrap()(CALLER_INPUTS, &arg0 as *const _ as *const _, core::mem::size_of_val(&arg0) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg1 as *const _ as *const _, core::mem::size_of_val(&arg1) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg2 as *const _ as *const _, core::mem::size_of_val(&arg2) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg3 as *const _ as *const _, core::mem::size_of_val(&arg3) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg4 as *const _ as *const _, core::mem::size_of_val(&arg4) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg5 as *const _ as *const _, core::mem::size_of_val(&arg5) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);

        emulated_to_emulated(arg0, arg1, arg2, arg3, arg4, arg5, );

        FINISHED_FUNC.unwrap()(CALLER_INPUTS, CALLER_OUTPUTS);
    }
    unsafe {
        // emulated -> unaligned_emulated (expected fail?)
        let arg0 = my_i128::new(34784419711585546284254720952638769794);
        let arg1 = my_i128::new(34784419711585546284254720952638769794);
        let arg2: f32 = 1234.456;
        let arg3 = my_i128::new(34784419711585546284254720952638769794);
        let arg4: u8 = 235;
        let arg5 = my_i128::new(34784419711585546284254720952638769794);

        WRITE.unwrap()(CALLER_INPUTS, &arg0 as *const _ as *const _, core::mem::size_of_val(&arg0) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg1 as *const _ as *const _, core::mem::size_of_val(&arg1) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg2 as *const _ as *const _, core::mem::size_of_val(&arg2) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg3 as *const _ as *const _, core::mem::size_of_val(&arg3) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg4 as *const _ as *const _, core::mem::size_of_val(&arg4) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg5 as *const _ as *const _, core::mem::size_of_val(&arg5) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);

        emulated_to_unaligned_emulated(arg0, arg1, arg2, arg3, arg4, arg5, );

        FINISHED_FUNC.unwrap()(CALLER_INPUTS, CALLER_OUTPUTS);
    }
    unsafe {
        // unaligned_emulated -> native (expected fail?)
        let arg0 = my_unaligned_i128::new(34784419711585546284254720952638769794);
        let arg1 = my_unaligned_i128::new(34784419711585546284254720952638769794);
        let arg2: f32 = 1234.456;
        let arg3 = my_unaligned_i128::new(34784419711585546284254720952638769794);
        let arg4: u8 = 235;
        let arg5 = my_unaligned_i128::new(34784419711585546284254720952638769794);

        WRITE.unwrap()(CALLER_INPUTS, &arg0 as *const _ as *const _, core::mem::size_of_val(&arg0) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg1 as *const _ as *const _, core::mem::size_of_val(&arg1) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg2 as *const _ as *const _, core::mem::size_of_val(&arg2) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg3 as *const _ as *const _, core::mem::size_of_val(&arg3) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg4 as *const _ as *const _, core::mem::size_of_val(&arg4) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg5 as *const _ as *const _, core::mem::size_of_val(&arg5) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);

        unaligned_emulated_to_native(arg0, arg1, arg2, arg3, arg4, arg5, );

        FINISHED_FUNC.unwrap()(CALLER_INPUTS, CALLER_OUTPUTS);
    }
    unsafe {
        // unaligned_emulated -> emulated (expected fail?)
        let arg0 = my_unaligned_i128::new(34784419711585546284254720952638769794);
        let arg1 = my_unaligned_i128::new(34784419711585546284254720952638769794);
        let arg2: f32 = 1234.456;
        let arg3 = my_unaligned_i128::new(34784419711585546284254720952638769794);
        let arg4: u8 = 235;
        let arg5 = my_unaligned_i128::new(34784419711585546284254720952638769794);

        WRITE.unwrap()(CALLER_INPUTS, &arg0 as *const _ as *const _, core::mem::size_of_val(&arg0) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg1 as *const _ as *const _, core::mem::size_of_val(&arg1) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg2 as *const _ as *const _, core::mem::size_of_val(&arg2) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg3 as *const _ as *const _, core::mem::size_of_val(&arg3) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg4 as *const _ as *const _, core::mem::size_of_val(&arg4) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg5 as *const _ as *const _, core::mem::size_of_val(&arg5) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);

        unaligned_emulated_to_emulated(arg0, arg1, arg2, arg3, arg4, arg5, );

        FINISHED_FUNC.unwrap()(CALLER_INPUTS, CALLER_OUTPUTS);
    }
    unsafe {
        // unaligned_emulated -> unaligned_emulated (expected fail?)
        let arg0 = my_unaligned_i128::new(34784419711585546284254720952638769794);
        let arg1 = my_unaligned_i128::new(34784419711585546284254720952638769794);
        let arg2: f32 = 1234.456;
        let arg3 = my_unaligned_i128::new(34784419711585546284254720952638769794);
        let arg4: u8 = 235;
        let arg5 = my_unaligned_i128::new(34784419711585546284254720952638769794);

        WRITE.unwrap()(CALLER_INPUTS, &arg0 as *const _ as *const _, core::mem::size_of_val(&arg0) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg1 as *const _ as *const _, core::mem::size_of_val(&arg1) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg2 as *const _ as *const _, core::mem::size_of_val(&arg2) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg3 as *const _ as *const _, core::mem::size_of_val(&arg3) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg4 as *const _ as *const _, core::mem::size_of_val(&arg4) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);
        WRITE.unwrap()(CALLER_INPUTS, &arg5 as *const _ as *const _, core::mem::size_of_val(&arg5) as u32);
        FINISHED_VAL.unwrap()(CALLER_INPUTS);

        unaligned_emulated_to_unaligned_emulated(arg0, arg1, arg2, arg3, arg4, arg5, );

        FINISHED_FUNC.unwrap()(CALLER_INPUTS, CALLER_OUTPUTS);
    }
}