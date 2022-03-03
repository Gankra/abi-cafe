#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct WriteBuffer(*mut ());
unsafe impl Send for WriteBuffer {}
unsafe impl Sync for WriteBuffer {}

type WriteValCallback = unsafe extern fn(WriteBuffer, *const u8, u32) -> ();

extern {
    pub static mut CALLER_INPUTS: WriteBuffer;
    pub static mut CALLER_OUTPUTS: WriteBuffer;
    pub static mut CALLEE_INPUTS: WriteBuffer;
    pub static mut CALLEE_OUTPUTS: WriteBuffer;
    pub static mut WRITE: Option<WriteValCallback>;
}