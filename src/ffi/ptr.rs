use std::slice::{self};

use crate::vmem::{self, ReadFromMem};

#[repr(C)]
pub struct Res<T> {
    err: i64,
    val: T,
}

macro_rules! mk_read {
    ($fn_name:ident, $type:ty) => {
        #[unsafe(no_mangle)]
        pub fn $fn_name(res: &mut Res<$type>, pid: u32, base: usize) {
            *res = match <$type>::from_mem(pid, base) {
                Ok(val) => Res { err: 0, val },
                Err(_) => Res { err: -1, val: 0 },
            };
        }
    };
}

mk_read!(read_usize, usize);

mk_read!(read_u8, u8);
mk_read!(read_u16, u16);
mk_read!(read_u32, u32);
mk_read!(read_u64, u64);

mk_read!(read_i8, i8);
mk_read!(read_i16, i16);
mk_read!(read_i32, i32);
mk_read!(read_i64, i64);

#[repr(i32)]
enum IoResult {
    Ok = 0,
    Err = -1,
}

impl From<nix::Result<()>> for IoResult {
    fn from(value: nix::Result<()>) -> Self {
        match value {
            Ok(()) => Self::Ok,
            Err(_) => Self::Err,
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn read_byte_array(mem: *mut u8, len: usize, pid: u32, base: usize) -> IoResult {
    let mem = unsafe { slice::from_raw_parts_mut(mem, len) };
    vmem::read_exact(pid, base, mem).into()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn read_c_string(
    mem: *mut u8,
    max_size: usize,
    pid: u32,
    base: usize,
) -> IoResult {
    const CHUNK_SIZE: usize = 4096;

    let mem = unsafe { slice::from_raw_parts_mut(mem, max_size) };

    // would be better to use 4096 - (addr % 4096) as the first chunk size
    let mut read = 0;
    for chunk in mem.chunks_mut(CHUNK_SIZE) {
        let Ok(count) = vmem::read(pid, base + read, chunk) else {
            return IoResult::Err;
        };

        read += count;

        if chunk.contains(&0) {
            return IoResult::Ok;
        }

        // this does pretty much nothing, but why not
        if count != CHUNK_SIZE {
            return IoResult::Err;
        }
    }
    IoResult::Ok
}

#[unsafe(no_mangle)]
unsafe extern "C" fn write_buf(mem: *mut u8, len: usize, pid: u32, base: usize) -> IoResult {
    let source = unsafe { slice::from_raw_parts(mem, len) };

    vmem::write_exact(pid, base, source).into()
}
