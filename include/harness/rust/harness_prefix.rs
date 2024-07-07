#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct WriteBuffer(*mut ());
unsafe impl Send for WriteBuffer {}
unsafe impl Sync for WriteBuffer {}

type SetFuncCallback = unsafe extern fn(WriteBuffer, u32) -> ();
type WriteValCallback = unsafe extern fn(WriteBuffer, u32, *const u8, u32) -> ();

extern {
    pub static mut CALLER_VALS: WriteBuffer;
    pub static mut CALLEE_VALS: WriteBuffer;
    pub static mut SET_FUNC: Option<SetFuncCallback>;
    pub static mut WRITE_VAL: Option<WriteValCallback>;
}

unsafe fn write_val<T>(vals: WriteBuffer, val_idx: u32, val: &T) {
    WRITE_VAL.unwrap()(
        vals,
        val_idx,
        val as *const T as *const u8,
        core::mem::size_of_val(val) as u32
    );
}
unsafe fn set_func(vals: WriteBuffer, func_idx: u32) {
    SET_FUNC.unwrap()(vals, func_idx);
}
