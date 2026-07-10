//! Ashfall bridge DLL — injected into Fallout3.exe under Proton/Wine.
//!
//! Responsibilities:
//! 1. Hook Gamebryo engine functions (VTable patching).
//! 2. Expose TCP server on 127.0.0.1:1771 for the native Linux client.
//! 3. Encode/decode the pipe protocol (same opcodes as original vaultmpdll).
//!
//! Cross-compiled with: cargo build --target x86_64-pc-windows-gnu
//!
//! ## Loading
//!
//! Preferred path: loaded by NVSE/FOSE as a plugin — `NVSEPlugin_Query` /
//! `NVSEPlugin_Load` handle init.
//!
//! Fallback: loaded via Wine DLL override (`WINEDLLOVERRIDES="bridge=n,b"`).
//! In this case `DllMain` handles init via a static flag to prevent double-init
//! if NVSE also loads the plugin.

pub mod commands;
pub mod console;
pub mod events;
pub mod hooks;
pub mod network;
pub mod plugin;

use std::sync::atomic::{AtomicBool, Ordering};

static RUNNING: AtomicBool = AtomicBool::new(true);
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// DLL entry point — fallback for non-NVSE injection.
///
/// If NVSEPlugin_Load has already run, DllMain is a no-op.
#[no_mangle]
pub extern "system" fn DllMain(
    _hinst: *mut std::ffi::c_void,
    reason: u32,
    _reserved: *mut std::ffi::c_void,
) -> i32 {
    const DLL_PROCESS_ATTACH: u32 = 1;
    const DLL_PROCESS_DETACH: u32 = 0;

    match reason {
        DLL_PROCESS_ATTACH => {
            // Only init if NVSE hasn't already done it
            if !INITIALIZED.swap(true, Ordering::SeqCst) {
                hooks::install();
                std::thread::spawn(|| {
                    network::run_server("127.0.0.1:1771");
                });
                console::register_defaults();
            }
            1 // success
        }
        DLL_PROCESS_DETACH => {
            RUNNING.store(false, Ordering::SeqCst);
            hooks::uninstall();
            1
        }
        _ => 1,
    }
}
