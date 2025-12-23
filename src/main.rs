#![no_std]
#![no_main]

use core::{
    fmt::{self, Write},
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
    pub const STD_ERROR_HANDLE: STD_HANDLE = 4294967284u32;
    pub const INVALID_HANDLE_VALUE: HANDLE = 0xffffffffffffffff as *mut core::ffi::c_void;

    #[allow(non_snake_case)]
    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn ExitProcess(uexitcode: u32) -> !;
        pub fn GetCommandLineA() -> PCSTR;
        pub fn GetStdHandle(handle_type: STD_HANDLE) -> HANDLE;
        pub fn Sleep(nmillisecs: u32);
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

///
///
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

fn round(x: f32) -> i32 {
    if !x.is_finite() {
        return 0;
    } else if x == 0.0 {
        return 0;
    }

    let bits = x.to_bits();
    let exp = ((bits >> (f32::MANTISSA_DIGITS - 1)) & 0xff) - 126;
    let mantissa =
        (1 << (f32::MANTISSA_DIGITS - 1)) | (bits & ((1 << (f32::MANTISSA_DIGITS - 1)) - 1));
    let sign = ((bits as i32) >> 31) | 1;
    let int_pt = mantissa >> (f32::MANTISSA_DIGITS - exp);
    let frac_pt = (mantissa << (32 + exp - f32::MANTISSA_DIGITS)) - ((!int_pt) & 1);
    let num = int_pt + (frac_pt >> 31);

    (num as i32) * sign
}

#[inline(always)]
fn sleep(secs: f32) {
    unsafe { win32::Sleep(round(secs * 1000.0) as u32) };
}

struct WinHandleOut<const HT: win32::STD_HANDLE> {
    handle: win32::HANDLE,
}

impl<const HT: win32::STD_HANDLE> WinHandleOut<HT> {
    pub fn new() -> Option<Self> {
        let handle = unsafe { win32::GetStdHandle(HT) };

        (handle != win32::INVALID_HANDLE_VALUE).then_some(Self { handle: handle })
    }
}

impl<const HT: win32::STD_HANDLE> Write for WinHandleOut<HT> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut num_written: u32 = 0;
        let res = unsafe {
            win32::WriteConsoleA(
                self.handle,
                s.as_ptr(),
                s.len() as u32,
                &mut num_written as *mut _,
                ptr::null(),
            )
        };

        (res != 0).then_some(()).ok_or(fmt::Error)
    }
}

type WinStdOut = WinHandleOut<{ win32::STD_OUTPUT_HANDLE }>;
type WinErrOut = WinHandleOut<{ win32::STD_ERROR_HANDLE }>;

#[unsafe(no_mangle)]
pub extern "system" fn _start() -> ! {
    let result = get_cmd_line();
    let mut stdout = WinStdOut::new().unwrap_or_else(|| exit_process(2));
    let mut stderr = WinErrOut::new().unwrap_or_else(|| exit_process(2));

    if let Some(cmd) = result {
        let duration = cmd.split_whitespace().skip(1).next().unwrap_or_else(|| {
            let _ = writeln!(
                stderr,
                "ERROR: requires 1 argument of how long to sleep for in seconds"
            );

            exit_process(1)
        });

        let f_durr = duration
            .parse::<f32>()
            .ok()
            .and_then(|x| (x >= 0.0).then_some(x))
            .unwrap_or_else(|| {
                let _ = writeln!(
                    stderr,
                    "ERROR: invalid number of seconds to sleep for: {duration}"
                );

                exit_process(1)
            });

        let _ = writeln!(stdout, "sleeping for {:.3}s", f_durr);
        sleep(f_durr);
    }

    exit_process(0)
}
