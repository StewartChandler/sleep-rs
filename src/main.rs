#![no_std]
#![no_main]

use core::{
    panic::PanicInfo,
    ptr::{self, slice_from_raw_parts},
};

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    loop {}
}

#[allow(non_upper_case_globals)]
#[unsafe(no_mangle)]
pub static _fltused: i32 = 0;

mod win32 {
    use core::ffi::c_void;

    pub type PCSTR = *const u8;
    pub type HANDLE = *mut c_void;
    pub type BOOL = i32;
    #[allow(non_camel_case_types)]
    pub type STD_HANDLE = u32;
    pub const STD_OUTPUT_HANDLE: STD_HANDLE = 4294967285u32;

    #[allow(non_snake_case)]
    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn ExitProcess(uexitcode: u32) -> !;
        pub fn GetCommandLineA() -> PCSTR;
        pub fn GetStdHandle(handle_type: STD_HANDLE) -> HANDLE;
        pub fn WriteConsoleA(
            hconsoleoutput: HANDLE,
            lpbuffer: PCSTR,
            nnumberofcharstowrite: u32,
            lpnumberofcharswritten: *mut u32,
            lpreserved: *const c_void,
        ) -> BOOL;
    }
}

/// gracefully terminates the current process
///
/// # args
///   - `code`: the exit code of the process
#[inline(always)]
fn exit_process(code: u32) -> ! {
    unsafe { win32::ExitProcess(code) }
}

#[inline(always)]
fn write_to_stdout(contents: &str) -> bool {
    let handle = unsafe { win32::GetStdHandle(win32::STD_OUTPUT_HANDLE) };

    let mut num_written: u32 = 0;
    let res = unsafe {
        win32::WriteConsoleA(
            handle,
            contents.as_ptr(),
            contents.len() as u32,
            &mut num_written as *mut _,
            ptr::null(),
        )
    };

    res != 0
}

///
///
#[inline(always)]
fn get_cmd_line() -> Option<&'static str> {
    // returns a null terminated const c string of the entire comand line encoded as ascii
    let cstr = unsafe { win32::GetCommandLineA() };
    if cstr.is_null() {
        None
    } else {
        let end = (0usize..(isize::MAX as usize / size_of::<u8>()))
            .map(|idx| unsafe { cstr.add(idx) })
            .find(|&ptr| unsafe { *ptr } == b'\0')?;

        let len = unsafe { end.offset_from_unsigned(cstr) };

        let bslice = unsafe { slice_from_raw_parts(cstr, len).as_ref::<'static>()? };

        str::from_utf8(bslice).ok()
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn _start() -> ! {
    let result = get_cmd_line();

    if let Some(cmd) = result {
        cmd.split_whitespace().skip(1).for_each(|s| {
            write_to_stdout(s);
            write_to_stdout("\r\n");
        });
    }

    exit_process(0)
}
