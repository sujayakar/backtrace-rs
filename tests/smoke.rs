#![allow(unconditional_recursion)] // FIXME rust-lang/rust#26165

extern crate backtrace;

use std::os::raw::c_void;
use std::str;

static LIBUNWIND: bool = cfg!(all(unix, feature = "libunwind"));
static UNIX_BACKTRACE: bool = cfg!(all(unix, feature = "unix-backtrace"));
static LIBBACKTRACE: bool = cfg!(all(unix, feature = "libbacktrace"));
static DLADDR: bool = cfg!(all(unix, feature = "dladdr"));
static DBGHELP: bool = cfg!(all(windows, feature = "dbghelp"));

#[test]
fn smoke() {
    a(line!());
    #[inline(never)] fn a(start_line: u32) { b(start_line) }
    #[inline(never)] fn b(start_line: u32) { c(start_line) }
    #[inline(never)] fn c(start_line: u32) { test(start_line) }
    #[inline(never)] fn test(start_line: u32) {
        let mut v = Vec::new();
        backtrace::trace(&mut |cx| {
            v.push((cx.ip(), cx.symbol_address()));
            true
        });

        if v.len() < 5 {
            assert!(!LIBUNWIND);
            assert!(!UNIX_BACKTRACE);
            assert!(!DBGHELP);
            return
        }

        assert_frame(v[0], backtrace::trace as usize, "::trace", "", 0);
        assert_frame(v[1], test as usize, "::test",
                     "tests/smoke.rs", start_line + 6);
        assert_frame(v[2], c as usize, "::c", "tests/smoke.rs", start_line + 3);
        assert_frame(v[3], b as usize, "::b", "tests/smoke.rs", start_line + 2);
        assert_frame(v[4], a as usize, "::a", "tests/smoke.rs", start_line + 1);
        assert_frame(v[5], smoke as usize, "smoke::", "", 0);
    }

    fn assert_frame((ip, sym): (*mut c_void, *mut c_void),
                    actual_fn_pointer: usize,
                    expected_name: &str,
                    expected_file: &str,
                    expected_line: u32) {
        let ip = ip as usize;
        let sym = sym as usize;
        assert!(ip >= sym);
        assert!(sym >= actual_fn_pointer);

        // windows dbghelp is *quite* liberal (and wrong) in many of its reports
        // right now...
        if !DBGHELP {
            assert!(sym - actual_fn_pointer < 1024);
        }

        let mut resolved = 0;
        let can_resolve = DLADDR || LIBBACKTRACE || DBGHELP;

        let mut name = None;
        let mut addr = None;
        let mut line = None;
        let mut file = None;
        backtrace::resolve(ip as *mut c_void, &mut |sym| {
            resolved += 1;
            name = sym.name().map(|v| v.to_vec());
            addr = sym.addr();
            line = sym.lineno();
            file = sym.filename().map(|v| v.to_vec());
        });

        // dbghelp doesn't always resolve symbols right now
        match resolved {
            0 => return assert!(!can_resolve || DBGHELP),
            _ => {}
        }

        // * linux dladdr doesn't work (only consults local symbol table)
        // * windows dbghelp doesn't work very well (unsure why)
        if can_resolve &&
           (cfg!(not(target_os = "linux")) || !DLADDR) &&
           !DBGHELP
        {
            let bytes = name.expect("didn't find a name");
            let bytes = str::from_utf8(&bytes).unwrap();
            let mut demangled = String::new();
            backtrace::demangle(&mut demangled, bytes).unwrap();;
            assert!(demangled.contains(expected_name),
                    "didn't find `{}` in `{}`", expected_name, demangled);
        }

        if can_resolve {
            addr.expect("didn't find a symbol");
        }

        if LIBBACKTRACE && cfg!(debug_assertions) {
            let line = line.expect("didn't find a line number");
            let file = file.expect("didn't find a line number");
            if !expected_file.is_empty() {
                assert_eq!(str::from_utf8(&file).unwrap(), expected_file);
            }
            if expected_line != 0 {
                assert!(line == expected_line,
                        "bad line number on frame for `{}`: {} != {}",
                        expected_name, line, expected_line);
            }
        }
    }
}
