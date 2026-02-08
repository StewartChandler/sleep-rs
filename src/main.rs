#![no_std]
#![no_main]

use core::{
    ffi::{CStr, c_char},
    fmt::{self, Write},
    i32,
    panic::PanicInfo,
    ptr::{self},
};

// needs to be seen by the linker in order to make floats work
#[allow(non_upper_case_globals)]
#[unsafe(no_mangle)]
pub static _fltused: i32 = 0;

mod win32 {
    pub use windows_sys::Win32::{
        Foundation::{HANDLE, INVALID_HANDLE_VALUE},
        System::{
            Console::{
                GetStdHandle, STD_ERROR_HANDLE, STD_HANDLE, STD_OUTPUT_HANDLE, WriteConsoleA,
            },
            Environment::GetCommandLineA,
            Threading::{ExitProcess, Sleep},
        },
    };
}

/// gracefully terminates the current process
///
/// # args
///   - `code`: the exit code of the process
#[inline(always)]
fn exit_process(code: u32) -> ! {
    // Saftey:
    //   - this is not marked as pub, this code can only be executed by this program, so it will
    //     not be executed as a dll
    unsafe { win32::ExitProcess(code) }
}

/// returns the command used to invoke this program, this is what is used for argument passing
///
/// # Returns
///   - `None` if `GetCommandLineA` returns a null pointer, or the string returned is not valid
///     UTF-8
///   - `Some(&'static str)` upon successfully fetching the command
fn get_cmd_line() -> Option<&'static str> {
    // returns a null terminated const c string of the entire command line encoded as ascii
    let cstr = unsafe { win32::GetCommandLineA() };
    if cstr.is_null() {
        None
    } else {
        // Safety:
        //   - cstr returned from `GetCommandLineA` is a `nul` terminated c str
        //   - the result of `GetCommandLineA` is not to be modified or freed by any other code
        //     and the lifetime is to be managed by the system, so the result should be a const str
        //     that will be valid for any lifetime
        // Unsafety:
        //   - the length of the string could possibly be greater than `isize::MAX`, in practice
        //     this will never happen, but the I don't know if windows actually makes any garuntees
        //     of that
        //   - the string may not exist for the `'static` lifetime, the documentation just says that
        //     the lifetime is managed by the system, but give no bounds on how long that is, I am
        //     assuming then, that the string will be available for however long you need it to be,
        //     hence static lifetime, but I really don't know
        let cstr = unsafe { CStr::from_ptr(cstr as *const c_char) };
        cstr.to_str().ok()
    }
}

/// rounds an `f32` to the nearest `i32` rounding towards even numbers in the case of a `.5`
/// fractional part
///
/// # Returns
///   - `0i32` if `x` is not finite
///   - `i32::MAX` if `x` is greater than `i32::MAX`
///   - `i32::MIN` if `x` is less than `i32::MIN`
fn round(x: f32) -> i32 {
    if !x.is_finite() {
        return 0;
    } else if x == f32::INFINITY {
        return i32::MAX;
    } else if x == f32::NEG_INFINITY {
        return i32::MIN;
    } else if x == 0.0 {
        return 0;
    }

    const EXPONENT_BITS: u32 = u32::BITS - f32::MANTISSA_DIGITS;
    const EXPONENT_MSK: u32 = (1u32 << EXPONENT_BITS) - 1;
    const EXPONENT_NEG_EMIN: i32 = (1i32 << (EXPONENT_BITS - 1)) - 1;

    let bits = x.to_bits();
    let exp = (((bits >> (f32::MANTISSA_DIGITS - 1)) & EXPONENT_MSK) as i32) - EXPONENT_NEG_EMIN;
    // extracts the mantissa bits from the float by masking off the portion stored in the float
    //    0b0_10000001_01000000000000000000000                                             == 5.0f32
    //  & 0b0_00000000_11111111111111111111111            == ((1 << (f32::MANTISSA_DIGITS - 1)) - 1)
    //  = 0b0_00000000_01000000000000000000000   == (bits & ((1 << (f32::MANTISSA_DIGITS - 1)) - 1))
    // then we add in the leading 1 that is elided from the iee754 float format
    //    0b0_00000000_01000000000000000000000
    //  | 0b0_00000001_00000000000000000000000                  == (1 << (f32::MANTISSA_DIGITS - 1))
    //  = 0b0_00000001_01000000000000000000000
    let mantissa =
        (1 << (f32::MANTISSA_DIGITS - 1)) | (bits & ((1 << (f32::MANTISSA_DIGITS - 1)) - 1));
    // extracts the sign as an i32 by first using a arithmatic shift right to sign extend the sign
    // bit of the float to the rest of the bits i.e. `0b111 ... 111` (`-1i32`) if the sign bit is 1,
    // otherwise `0b000 ... 000` (0i32) if it is 0, then the bitwise or makes the result `1` if it
    // was `0`, and does nothing if it was `-1`, so the result is that `sign` is `-1` if the sign
    // bit was `1` and `1` if it was `0`
    let sign = ((bits as i32) >> (i32::BITS - 1)) | 1i32;
    let num = if exp >= (u32::BITS - 1) as i32 {
        // exp [31,inf)
        // bc of the wrapping add, will be `i32::MIN` if sign bit is set
        i32::MAX.wrapping_add((bits >> (i32::BITS - 1)) as i32)
    } else if exp >= (f32::MANTISSA_DIGITS - 1) as i32 {
        // exp [23, 31)
        (mantissa << (exp - (f32::MANTISSA_DIGITS - 1) as i32)) as i32 * sign
    } else if exp >= 0 {
        // exp [0, 23)
        // is the value truncated
        let base_val = mantissa >> ((f32::MANTISSA_DIGITS - 1) as i32 - exp);
        // add 1 (for rounding purposes) if fract part > .5 or if fractional part == .5 and the last
        // bit of the integral part is 1
        (base_val
            + (((mantissa << (EXPONENT_BITS + 1 + exp as u32)).saturating_sub(!base_val & 0b1))
                >> (u32::BITS - 1)) as u32) as i32
            * sign
    } else if exp == -1 {
        // exp == -1 means x == 0.5yyyyyy, so if yyyyyy != 1 then round up (1) else round towards an
        // even number (0)
        ((bits & ((1 << (f32::MANTISSA_DIGITS - 1)) - 1)) != 0) as i32 * sign
    } else {
        0
    };

    num
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

        (!handle.is_null() && handle != win32::INVALID_HANDLE_VALUE)
            .then_some(Self { handle: handle })
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

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    exit_process(3)
}

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
