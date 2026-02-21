use nix::poll::{PollFd, PollFlags, PollTimeout};
use seccompy::SeccompNotif;
use std::{
    collections::HashMap,
    os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd},
    process::exit,
};

pub type SyscallHandler = extern "C" fn(
    cookie: Box<(SeccompNotif, i32)>,
    pid: u32,
    nr: i32,
    pc: u64,
    arg_0: u64,
    arg_1: u64,
    arg_2: u64,
    arg_3: u64,
    arg_4: u64,
    arg_5: u64,
);

pub struct Supervisor {
    listener: OwnedFd,
    callbacks: HashMap<u32, SyscallHandler>,
}

impl Supervisor {
    pub fn new(listener: OwnedFd, callbacks: HashMap<u32, SyscallHandler>) -> Self {
        Self {
            listener,
            callbacks,
        }
    }

    pub fn supervise(self) -> ! {
        let descriptor_no = self.listener.as_raw_fd();
        let descriptor = unsafe { OwnedFd::from_raw_fd(descriptor_no) };

        eprintln!("[syshook] supervising...");
        loop {
            let Ok(noti) = seccompy::receive_notification(descriptor_no) else {
                let poll_fd = PollFd::new(descriptor.as_fd(), PollFlags::POLLHUP);
                let poll_result = nix::poll::poll(&mut [poll_fd], PollTimeout::NONE).unwrap();
                if poll_result != 0 {
                    break;
                }
                continue;
            };

            // using raw fd is okay: after the fd is closed, the supervised program would no longer exist, and i/o safety violations would be meaningless
            let noti_fd = Box::new((noti, descriptor_no));

            self.callbacks[&noti.data.nr.cast_unsigned()](
                noti_fd,
                noti.pid,
                noti.data.nr,
                noti.data.instruction_pointer,
                noti.data.args[0],
                noti.data.args[1],
                noti.data.args[2],
                noti.data.args[3],
                noti.data.args[4],
                noti.data.args[5],
            );
        }

        eprintln!("[syshook] supervisor shutdown");
        exit(0);
    }
}
