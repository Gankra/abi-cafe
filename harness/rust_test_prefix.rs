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
    pub static mut WRITE: Option<WriteCallback>;
    pub static mut FINISHED_VAL: Option<FinishedValCallback>;
    pub static mut FINISHED_FUNC: Option<FinishedFuncCallback>;
}

