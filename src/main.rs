#![no_std]
#![no_main]

use core::{
    panic::PanicInfo,
    ptr::{self},
    slice,
};

const NUL_CHR: u16 = unsafe { *w!("") };

macro_rules! utf16_str_lit {
    ($s:literal) => {{
        const C_STR: *const u16 = ::windows_sys::w!($s);
        const NUL_CHR: u16 = unsafe { *::windows_sys::w!("") };

        const OUTPUT: &'static [u16] = {
            ::core::assert!(!C_STR.is_null(), "`C_STR` should not be null");
            // should assert alignement of the pointer as well but for whatever reason I cannot

            let mut idx = 0isize;
            let len = loop {
                // Safety:
                //   - `idx` is an isize so the offset in bytes must fit in an isize
                //   - as the pointer is a string derived from a system call, it should all be a
                //     part of the same contiguous allocation
                let end_ptr = unsafe { C_STR.offset(idx) };

                // there must be a nul terminator at the end of the string
                if unsafe { *end_ptr } == NUL_CHR {
                    break idx;
                }

                idx += 1;
            } as usize;
            // Saftey:
            //   - `cstr` is non-null alligned and part of a contiguous allocation valid for reads
            //     of `size * size_of::<u16>()` bytes.
            //   - `cstr` does point to `size` consecutive properly initialized values of u16 bc it
            //     was given to us by the os
            //   - the data should not be mutated bc it is a const c-string given to us by the os
            //   - size should not exceed that which would make
            //     `size * size_of::<u16>() > isize::MAX`
            unsafe { slice::from_raw_parts(C_STR, len) }
        };

        OUTPUT
    }};
}

use windows_sys::{
    Win32::{
        Foundation::{HANDLE, INVALID_HANDLE_VALUE},
        System::{
            Console::{
                GetStdHandle, STD_ERROR_HANDLE, STD_HANDLE, STD_OUTPUT_HANDLE, WriteConsoleW,
            },
            Environment::GetCommandLineW,
            Threading::{ExitProcess, Sleep},
        },
    },
    core::PCWSTR,
    w,
};

// // needs to be seen by the linker in order to make floats work
// #[allow(non_upper_case_globals)]
// #[unsafe(no_mangle)]
// pub static _fltused: i32 = 0;

// required by lack of ucrt link

/// # Safety
///   - `wstr` must be a pointer to a NUL terminated wchar_t string
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wcslen(wstr: *const u16) -> usize {
    // Safety: `wstr` is a pointer to a NUL terminated string so it is safe to index into it up to
    // the point where we reach a NUL character
    (0..)
        .map(|idx| unsafe { wstr.offset(idx) })
        .take_while(|&ptr| unsafe { *ptr } != NUL_CHR)
        .count()
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
    unsafe { ExitProcess(code) }
}

/// returns the command used to invoke this program, this is what is used for argument passing
///
/// # Returns
///   - `None` if `GetCommandLineW` returns a null pointer
///   - `Some(&'static [u16])` upon successfully fetching the command
fn get_cmd_line_utf16() -> Option<&'static [u16]> {
    // returns a null terminated const c string of the entire command line encoded as utf16
    let cstr = unsafe { GetCommandLineW() };
    (!cstr.is_null() && cstr.is_aligned()).then(|| {
        let mut off = 0isize;
        let size = loop {
            // Safety:
            //   - `off` is an isize so the offset in bytes must fit in an isize
            //   - as the pointer is a string derived from a system call, it should all be a part of
            //     the same contiguous allocation
            let end_ptr = unsafe { cstr.offset(off) };

            // there must be a nul terminator at the end of the string
            if unsafe { *end_ptr } == NUL_CHR {
                break off;
            }

            off += 1;
        } as usize;

        // Saftey:
        //   - `cstr` is non-null alligned and part of a contiguous allocation valid for reads of
        //     `size * size_of::<u16>()` bytes.
        //   - `cstr` does point to `size` consecutive properly initialized values of u16 bc it was
        //     given to us by the os
        //   - the data should not be mutated bc it is a const c-string given to us by the os
        //   - size should not exceed that which would make `size * size_of::<u16>() > isize::MAX`
        unsafe { slice::from_raw_parts(cstr, size) }
    })
}

#[inline(always)]
fn sleep_ms(secs: u32) {
    unsafe { Sleep(secs) };
}

struct WinHandleOut<const HT: STD_HANDLE> {
    handle: HANDLE,
}

impl<const HT: STD_HANDLE> WinHandleOut<HT> {
    pub fn new() -> Option<Self> {
        let handle = unsafe { GetStdHandle(HT) };

        (!handle.is_null() && handle != INVALID_HANDLE_VALUE).then_some(Self { handle })
    }

    pub fn write_utf16(&self, utf16_str: &[u16]) -> Result<(), ()> {
        let mut chars_written = 0u32;
        let mut ptr = utf16_str.as_ptr();
        let mut to_write = utf16_str.len();

        while to_write != 0 {
            let result = unsafe {
                WriteConsoleW(
                    self.handle,
                    ptr as PCWSTR,
                    to_write.clamp(0, u32::MAX as usize) as u32,
                    &mut chars_written as *mut _,
                    ptr::null(),
                )
            };

            // then there was an error in writing
            if result == 0 {
                return Err(());
            }

            to_write = to_write.saturating_sub(chars_written as usize);
            // Safety:
            //   - `chars_written` is bounded by the size of the slice so this pointer must be at
            //     most pointing 1 past the end of the slice
            // Unsafety:
            //   - yes this does mean technically that if the allocation is at the end of the
            //     address-space then it could "wrap" around to be 0
            ptr = unsafe { ptr.add(chars_written as usize) };

            chars_written = 0;
        }

        Ok(())
    }

    pub fn write_fixed_pt<const PTS: usize>(&self, num: u32) -> Result<(), ()> {
        const MAX_DIGITS: usize = (u32::MAX.ilog10() + 1) as usize;
        // one day we'll be able to do this:
        // const _PTS_CHK: () = assert!(
        //     PTS < MAX_DIGITS,
        //     "pts must not be greater than the max number of digits that can be stored by a usize"
        // );

        const ZERO_CHR: u16 = unsafe { *w!("0") };
        const POINT_CHR: u16 = unsafe { *w!(".") };

        let mut buffer = [NUL_CHR; MAX_DIGITS + 2];
        let int_part = num / (10u32.pow(PTS as u32));
        let fract_part = num % (10u32.pow(PTS as u32));

        let int_part_sz = (int_part.checked_ilog10().unwrap_or(0) + 1) as usize;

        for (i, pos) in buffer[..int_part_sz].iter_mut().enumerate() {
            let dig = (int_part / (10u32.pow((int_part_sz - 1 - i) as u32))) % 10;
            *pos = ZERO_CHR + dig as u16;
        }

        buffer[int_part_sz] = POINT_CHR;

        for (i, pos) in buffer[int_part_sz + 1..=int_part_sz + PTS]
            .iter_mut()
            .enumerate()
        {
            let dig = (fract_part / (10u32.pow((PTS - 1 - i) as u32))) % 10;
            *pos = ZERO_CHR + dig as u16;
        }

        self.write_utf16(&buffer[..=int_part_sz + PTS + 1])
    }
}

fn read_fixed_pt<const PTS: usize>(buf: &[u16]) -> Result<u32, ()> {
    const ZERO_CHR: u16 = unsafe { *w!("0") };
    const POINT_CHR: u16 = unsafe { *w!(".") };

    let (int_pt, fract_pt) = {
        let split_idx = buf
            .iter()
            .enumerate()
            .find_map(|(idx, &chr)| (chr == POINT_CHR).then_some(idx))
            .unwrap_or(buf.len());

        (&buf[..split_idx], &buf[buf.len().min(split_idx + 1)..])
    };

    int_pt
        .iter()
        .all(|x| (ZERO_CHR..ZERO_CHR + 10).contains(x))
        .then_some(())
        .ok_or(())?;
    fract_pt
        .iter()
        .all(|x| (ZERO_CHR..ZERO_CHR + 10).contains(x))
        .then_some(())
        .ok_or(())?;

    let int_pt = int_pt
        .iter()
        .rev()
        .enumerate()
        .map(|(idx, chr)| (chr - ZERO_CHR) as u32 * 10u32.pow(idx as u32))
        .fold(0u32, |acc, x| acc.saturating_add(x));

    // TODO: deal with rounding
    let fract_pt = fract_pt
        .iter()
        .enumerate()
        .take(PTS)
        .map(|(idx, chr)| (chr - ZERO_CHR) as u32 * 10u32.pow((PTS - 1 - idx) as u32))
        .fold(0u32, |acc, x| acc.saturating_add(x));

    Ok(int_pt
        .saturating_mul(10u32.pow(PTS as u32))
        .saturating_add(fract_pt))
}

fn get_next_arg(cmd_str: &[u16]) -> Option<(&[u16], &[u16])> {
    const BSLASH_CHR: u16 = unsafe { *w!("\\") };
    const QUOTE_CHR: u16 = unsafe { *w!("\"") };
    const SPACE_CHR: u16 = unsafe { *w!(" ") };

    (!cmd_str.is_empty()).then(|| {
        if cmd_str[0] == QUOTE_CHR {
            let end_idx = cmd_str
                .iter()
                .enumerate()
                .skip(1)
                .scan(false, |st, (idx, &chr)| {
                    let prev_st = *st;
                    *st = !prev_st && (chr == BSLASH_CHR);

                    Some((!prev_st && chr == QUOTE_CHR, idx))
                })
                .find_map(|(pred, idx)| (pred).then_some(idx))
                .unwrap_or(cmd_str.len());

            (
                &cmd_str[1..end_idx.min(cmd_str.len())],
                &cmd_str[(end_idx + 2).min(cmd_str.len())..],
            )
        } else {
            let end_idx = cmd_str
                .iter()
                .enumerate()
                .find_map(|(idx, &chr)| (chr == SPACE_CHR).then_some(idx))
                .unwrap_or(cmd_str.len());

            (
                &cmd_str[..end_idx.min(cmd_str.len())],
                &cmd_str[(end_idx + 1).min(cmd_str.len())..],
            )
        }
    })
}

type WinStdOut = WinHandleOut<{ STD_OUTPUT_HANDLE }>;
#[allow(unused)]
type WinErrOut = WinHandleOut<{ STD_ERROR_HANDLE }>;

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    exit_process(3)
}

#[unsafe(no_mangle)]
pub extern "system" fn _start() -> ! {
    let cmd_str = get_cmd_line_utf16().unwrap_or_else(|| exit_process((-1i32) as u32));

    let stdout = WinStdOut::new().unwrap_or_else(|| exit_process((-1i32) as u32));

    let (_exec_name, rem) = get_next_arg(cmd_str).unwrap_or_else(|| exit_process((-1i32) as u32));
    let (arg1, _rem) = get_next_arg(rem).unwrap_or_else(|| {
        // TODO: add usage printing in this case
        exit_process((-1i32) as u32)
    });

    let num_ms = read_fixed_pt::<3>(arg1).unwrap_or_else(|_| exit_process((-1i32) as u32));

    stdout
        .write_utf16(utf16_str_lit!("will now sleep for: "))
        .unwrap_or_else(|_| exit_process((-1i32) as u32));
    stdout
        .write_fixed_pt::<3>(num_ms)
        .unwrap_or_else(|_| exit_process((-1i32) as u32));
    stdout
        .write_utf16(utf16_str_lit!("s\r\n"))
        .unwrap_or_else(|_| exit_process((-1i32) as u32));

    sleep_ms(num_ms);

    exit_process(0)

    // let result = get_cmd_line();
    // let mut stdout = WinStdOut::new().unwrap_or_else(|| exit_process(2));
    // let mut stderr = WinErrOut::new().unwrap_or_else(|| exit_process(2));

    // if let Some(cmd) = result {
    //     let duration = cmd.split_whitespace().skip(1).next().unwrap_or_else(|| {
    //         let _ = writeln!(
    //             stderr,
    //             "ERROR: requires 1 argument of how long to sleep for in seconds"
    //         );

    //         exit_process(1)
    //     });

    //     let f_durr = duration
    //         .parse::<f32>()
    //         .ok()
    //         .and_then(|x| (x >= 0.0).then_some(x))
    //         .unwrap_or_else(|| {
    //             let _ = writeln!(
    //                 stderr,
    //                 "ERROR: invalid number of seconds to sleep for: {duration}"
    //             );

    //             exit_process(1)
    //         });

    //     let _ = writeln!(stdout, "sleeping for {:.3}s", f_durr);
    //     sleep(f_durr);
    // }

    // exit_process(0)
}
