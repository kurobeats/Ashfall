//! dinput8.dll proxy → runs the Ashfall bridge inside Fallout3.exe.
//!
//! The game imports dinput8.dll; a native copy in the game dir wins over
//! wine's builtin, so our DllMain runs bridge init while DirectInput8Create
//! forwards to the real (wine builtin) dinput8 loaded from system32.
//!
//! Build: cargo build --release --target i686-pc-windows-gnu -p ashfall-bridge-proxy
//! Ship: copy target/i686-pc-windows-gnu/release/ashfall_bridge_proxy.dll \
//!         "$FALLOUT3_DIR/dinput8.dll"

#[cfg(target_os = "windows")]
use std::ffi::c_void;
#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "windows")]
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

#[cfg(target_os = "windows")]
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Bridge init — same as bridge.dll's DllMain attach (hooks + TCP server).
#[cfg(target_os = "windows")]
fn init() {
    if !INITIALIZED.swap(true, Ordering::SeqCst) {
        ashfall_bridge::bridge_init();
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub extern "system" fn DllMain(_hinst: *mut c_void, reason: u32, _reserved: *mut c_void) -> i32 {
    const DLL_PROCESS_ATTACH: u32 = 1;
    if reason == DLL_PROCESS_ATTACH {
        init();
    }
    1
}

// ── dinput8 export forwarding ──
// Real (wine builtin) dinput8 lives in system32; load by exact path so we
// never recurse into ourselves (bare-name lookups hit our app-dir copy).

#[cfg(target_os = "windows")]
type D8CreateFn = unsafe extern "system" fn(
    hinst: *mut c_void,
    dw_version: u32,
    riid: *const c_void,
    ppv_out: *mut *mut c_void,
    punk_outer: *mut c_void,
) -> i32;

#[cfg(target_os = "windows")]
static REAL_D8: std::sync::OnceLock<Option<D8CreateFn>> = std::sync::OnceLock::new();

#[cfg(target_os = "windows")]
fn real_d8_create() -> Option<D8CreateFn> {
    *REAL_D8.get_or_init(|| unsafe {
        const SYS_D8: *const u8 = b"C:\\windows\\system32\\dinput8.dll\0".as_ptr();
        let m = LoadLibraryA(SYS_D8);
        if m == 0 {
            return None;
        }
        const NAME: *const u8 = b"DirectInput8Create\0".as_ptr();
        let p: unsafe extern "system" fn() -> isize = GetProcAddress(m, NAME)?;
        Some(std::mem::transmute::<_, D8CreateFn>(p))
    })
}

/// DirectInput8Create — the one entry point FO3 actually calls.
#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn DirectInput8Create(
    hinst: *mut c_void,
    dw_version: u32,
    riid: *const c_void,
    ppv_out: *mut *mut c_void,
    punk_outer: *mut c_void,
) -> i32 {
    match real_d8_create() {
        Some(f) => unsafe { f(hinst, dw_version, riid, ppv_out, punk_outer) },
        None => -1, // DIERR_OUTOFMEMORY-ish; game aborts input init but runs
    }
}
