use nix::{
    errno::Errno,
    sys::uio::{RemoteIoVec, process_vm_readv, process_vm_writev},
    unistd::Pid as NixPid,
};
use std::io::{IoSlice, IoSliceMut};

#[inline]
pub fn read(pid: u32, base: usize, buf: &mut [u8]) -> nix::Result<usize> {
    let len = buf.len();

    let iov = IoSliceMut::new(buf);
    let remote_iov = RemoteIoVec { base, len };

    process_vm_readv(
        NixPid::from_raw(pid.cast_signed()),
        &mut [iov],
        &[remote_iov],
    )
}

#[inline]
pub fn read_exact(pid: u32, base: usize, buf: &mut [u8]) -> nix::Result<()> {
    if read(pid, base, buf)? == buf.len() {
        Ok(())
    } else {
        Err(Errno::EFAULT)
    }
}

#[inline]
pub fn read_const<const N: usize>(pid: u32, base: usize) -> nix::Result<[u8; N]> {
    let mut buf = [0; N];

    if read(pid, base, &mut buf)? == N {
        Ok(buf)
    } else {
        Err(Errno::EFAULT)
    }
}

pub trait ReadFromMem: Sized {
    fn from_mem(pid: u32, base: usize) -> nix::Result<Self>;
}

macro_rules! impl_from_mem {
    ($($type:ty),*) => {
        $(impl ReadFromMem for $type {
            #[inline]
            fn from_mem(pid: u32, base: usize) -> nix::Result<Self> {
                Ok(Self::from_ne_bytes(
                    read_const::<{ size_of::<Self>() }>(pid, base)?,
                ))
            }
        })*
    }
}

impl_from_mem!(u8, u16, u32, u64, i8, i16, i32, i64, usize);

#[inline]
pub fn write(pid: u32, base: usize, buf: &[u8]) -> nix::Result<usize> {
    let len = buf.len();

    let iov = IoSlice::new(buf);
    let remote_iov = RemoteIoVec { base, len };

    process_vm_writev(NixPid::from_raw(pid.cast_signed()), &[iov], &[remote_iov])
}

#[inline]
pub fn write_exact(pid: u32, base: usize, buf: &[u8]) -> nix::Result<()> {
    if write(pid, base, buf)? == buf.len() {
        Ok(())
    } else {
        Err(Errno::EFAULT)
    }
}
