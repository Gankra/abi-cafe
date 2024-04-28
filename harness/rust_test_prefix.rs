#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct WriteBuffer(*mut ());
unsafe impl Send for WriteBuffer {}
unsafe impl Sync for WriteBuffer {}

type WriteCallback = unsafe extern fn(WriteBuffer, *const u8, u32) -> ();
type FinishedValCallback = unsafe extern fn(WriteBuffer) -> ();
type FinishedFuncCallback = unsafe extern fn(WriteBuffer, WriteBuffer) -> ();

extern {
    pub static mut CALLER_INPUTS: WriteBuffer;
    pub static mut CALLER_OUTPUTS: WriteBuffer;
    pub static mut CALLEE_INPUTS: WriteBuffer;
    pub static mut CALLEE_OUTPUTS: WriteBuffer;
    pub static mut WRITE_FIELD: Option<WriteCallback>;
    pub static mut FINISHED_VAL: Option<FinishedValCallback>;
    pub static mut FINISHED_FUNC: Option<FinishedFuncCallback>;
}

#[repr(C, align(16))]
pub struct FfiI128 {
    low: i64,
    high: i64,
}
#[repr(C, align(16))]
pub struct FfiU128 {
    low: u64,
    high: u64,
}

impl FfiI128 {
    fn new(val: i128) -> Self {
        Self {
            low: val as u64 as i64,
            high: (val as u128 >> 64) as u64 as i64,
        }
    }
}

impl FfiU128 {
    fn new(val: u128) -> Self {
        Self {
            low: val as u64,
            high: (val as u128 >> 64) as u64,
        }
    }
}

unsafe fn write_field<T>(buffer: WriteBuffer, field: &T) {
    WRITE_FIELD.unwrap()(
        buffer,
        field as *const T as *const u8,
        core::mem::size_of_val(field) as u32
    );
}
unsafe fn finished_val(buffer: WriteBuffer) {
    FINISHED_VAL.unwrap()(buffer);
}
unsafe fn finished_func(inputs: WriteBuffer, outputs: WriteBuffer) {
    FINISHED_FUNC.unwrap()(inputs, outputs);
}
