use seccompy::{SeccompNotif, continue_syscall, fail_syscall, return_syscall, send_response};

#[repr(u8)]
#[derive(Debug)]
pub enum ResponseType {
    Continue = 0,
    Return = 1,
    Fail = 2,
    NeverRespond = 3,
}

#[unsafe(no_mangle)]
pub extern "C" fn resolve(
    noti_fd: Box<(SeccompNotif, i32)>,
    response_type: ResponseType,
    value: i64,
) {
    let (noti, raw_fd) = *noti_fd;

    let response = match response_type {
        ResponseType::Continue => continue_syscall(noti),
        ResponseType::Return => return_syscall(noti, value),
        // is the error case even worth implementing
        // it's literally return but with value negation
        ResponseType::Fail => {
            assert_ne!(value, 0, "error code must not be zero");
            fail_syscall(noti, value.try_into().expect("invalid error code"))
        }
        ResponseType::NeverRespond => return,
    };
    let _ = send_response(raw_fd, response);
}
