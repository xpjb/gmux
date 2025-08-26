use std::ffi::CString;
use std::ptr::null_mut;
use std::os::raw::c_char;
use crate::command::Command;

pub fn spawn(cmd: &Command) {
    if unsafe { libc::fork() } == 0 {
        unsafe {
            libc::setsid();
            let shell = CString::new("/bin/sh").unwrap();
            let c_flag = CString::new("-c").unwrap();
            let cmd_str = CString::new(cmd.str()).unwrap();
            libc::execlp(
                shell.as_ptr(),
                shell.as_ptr(),
                c_flag.as_ptr(),
                cmd_str.as_ptr(),
                null_mut::<c_char>(),
            );
        }
    }
}
