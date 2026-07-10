//! Ashfall bridge DLL — injected into Fallout3.exe under Proton/Wine.
//!
//! Responsibilities:
//! 1. Hook Gamebryo engine functions (VTable patching).
//! 2. Expose TCP server on 127.0.0.1:1771 for the native Linux client.
//! 3. Encode/decode the pipe protocol (same opcodes as original vaultmpdll).
//!
//! Cross-compiled with: cargo build --target x86_64-pc-windows-gnu

pub mod commands;
pub mod hooks;
pub mod network;

use std::thread;
use std::sync::atomic::{AtomicBool, Ordering};

static RUNNING: AtomicBool = AtomicBool::new(true);

/// DLL entry point — called when loaded by Wine/Proton.
#[no_mangle]
pub extern "system" fn DllMain(_hinst: *mut std::ffi::c_void, reason: u32, _reserved: *mut std::ffi::c_void) -> i32 {
    const DLL_PROCESS_ATTACH: u32 = 1;
    const DLL_PROCESS_DETACH: u32 = 0;

    match reason {
        DLL_PROCESS_ATTACH => {
            // Spawn TCP server in background thread
            thread::spawn(|| {
                network::run_server("127.0.0.1:1771");
            });
            // Install engine hooks
            hooks::install();
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
