use libc::{CLONE_FILES, CLONE_FS, CLONE_SYSVSEM, unshare};
use memmap::MmapOptions;
use seccompy::{
    FilterAction, FilterWithListenerFlags, SetFilterError,
    seccomp_bpf::filter::{Filter, FilterArgs, VerificationError},
};
use std::{
    collections::HashMap,
    hint,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    process::exit,
    sync::atomic::{AtomicBool, AtomicI32, Ordering},
    thread::{self},
    time::Duration,
};
use thiserror::Error;

use crate::supervisor::{Supervisor, SyscallHandler};

const NO_DESCRIPTOR: i32 = -1;
const SECCOMP_ERROR: i32 = -2;

fn try_read_descriptor(desc: &AtomicI32) -> Option<Result<i32, ()>> {
    match desc.load(Ordering::Relaxed) {
        NO_DESCRIPTOR => None,
        SECCOMP_ERROR => Some(Err(())),
        desc => Some(Ok(desc)),
    }
}

#[derive(Error, Debug)]
pub enum SeccompError {
    #[error("failed to compile the filter")]
    CompilationError(#[from] VerificationError),
    #[error("failed to set filter")]
    SetFilterError(#[from] SetFilterError),
}

#[unsafe(no_mangle)]
pub static SUCCESS: i32 = 0;

#[unsafe(no_mangle)]
pub static COMPILATION_ERROR: i32 = -1;

#[unsafe(no_mangle)]
pub static SET_FILTER_ERROR: i32 = -2;

fn setup_seccomp(
    desc: &AtomicI32,
    can_drop_fd: &AtomicBool,
    sysnr_to_intercept: &[u32],
) -> Result<(), SeccompError> {
    // this may be useful if we're spawning a process with a separate memory space
    // for now we do not though
    // if unsafe { prctl(PR_SET_PTRACER, PR_SET_PTRACER_ANY) } == -1 {
    //     eprintln!("[syshook] failed to set ptracer to PR_SET_PTRACER_ANY, memory accesses may fail");
    // }

    let mut filter = Filter::new(FilterArgs {
        default_action: FilterAction::Allow,
        ..Default::default()
    });

    filter.add_syscall_group(sysnr_to_intercept, FilterAction::UserNotif);

    // todo: ProcessControlError should be public API, so it can be used as a SeccompError variant
    seccompy::set_no_new_privileges().unwrap();

    let descriptor = seccompy::set_filter_with_listener(
        FilterWithListenerFlags {
            ignore_non_fatal_signals: true,
            sync_threads: false,
            ..Default::default()
        },
        &filter.compile()?,
    )?;

    desc.store(descriptor.as_raw_fd(), Ordering::Relaxed);
    // better not to make syscalls if possible
    loop {
        hint::spin_loop();

        if can_drop_fd.load(Ordering::Relaxed) {
            break;
        }
    }
    drop(descriptor);
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn new_callback_map() -> Box<HashMap<u32, SyscallHandler>> {
    Box::new(HashMap::default())
}

#[unsafe(no_mangle)]
pub extern "C" fn insert_callback(
    map: &mut HashMap<u32, SyscallHandler>,
    sys_nr: u32,
    callback: SyscallHandler,
) {
    map.insert(sys_nr, callback);
}

#[unsafe(no_mangle)]
pub extern "C" fn supervise(callbacks: Box<HashMap<u32, SyscallHandler>>) -> i32 {
    let mut mem = MmapOptions::new()
        .len(size_of::<AtomicI32>() + size_of::<AtomicBool>())
        .map_anon()
        .expect("mmap failed");

    let mem = mem.as_mut_ptr();

    #[expect(
        clippy::cast_ptr_alignment,
        reason = "mmap returns a page-aligned ptr, page alignment is surely at least 4 bytes"
    )]
    let fd = unsafe { mem.cast::<AtomicI32>().as_ref().unwrap() };
    fd.store(NO_DESCRIPTOR, Ordering::Relaxed);

    let can_drop_fd = unsafe {
        mem.byte_add(size_of::<AtomicI32>())
            .cast::<AtomicBool>()
            .as_ref()
            .unwrap()
    };

    let to_intercept = callbacks.keys().copied().collect::<Vec<u32>>();

    thread::spawn(|| {
        // yes, that's a bad way to share data between threads (we have static atomics+parking/condvars/channels/oncelocks),
        // but this one works even if we use spawn a process using clone syscall with only the CLONE_FILES flag set,
        // which may be useful in the future
        let descriptor = loop {
            if let Some(res) = try_read_descriptor(fd) {
                if let Ok(desc) = res {
                    break desc;
                }
                eprintln!("[syshook] supervisee failed, bye");
                exit(1);
            }
            thread::sleep(Duration::from_micros(100));
        };

        let fd = unsafe { OwnedFd::from_raw_fd(descriptor) };

        unsafe { unshare(CLONE_FILES) };
        unsafe { unshare(CLONE_SYSVSEM) };
        unsafe { unshare(CLONE_FS) };

        can_drop_fd.store(true, Ordering::Relaxed);

        Supervisor::new(fd, *callbacks).supervise();
    });

    match setup_seccomp(fd, can_drop_fd, &to_intercept) {
        Ok(()) => SUCCESS,
        Err(e) => match e {
            SeccompError::CompilationError(_) => COMPILATION_ERROR,
            SeccompError::SetFilterError(_) => SET_FILTER_ERROR,
        },
    }
}
