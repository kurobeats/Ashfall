//! Trampoline-based function detour.
//!
//! Pattern: write a 5-byte JMP at the target function entry point, redirecting
//! to our hook. The trampoline holds the original first 5 bytes and a JMP back
//! to target+5, allowing call-through to the original implementation.
//!
//! Ported from vaultmp-extended's `BethesdaDelegator` / `PatchGame` pattern.
//!
//! # Platform
//!
//! All operations use raw memory writes via VirtualProtect (windows-sys).
//! On non-Windows targets the primitives fail gracefully — the bridge is only
//! ever deployed inside a Wine/Proton process.

use std::ptr;
use crate::hooks::memory;

// JMP rel32 = opcode (1 byte) + displacement (4 bytes)
const JMP_SIZE: usize = 5;
const JMP_OPCODE: u8 = 0xE9;

// Trampoline: [saved_prologue (JMP_SIZE)] + [JMP target+JMP_SIZE (JMP_SIZE)]
const TRAMPOLINE_SIZE: usize = JMP_SIZE + JMP_SIZE;

/// A function detour — hooks a function, preserves the original via trampoline.
pub struct Detour {
    target: *mut u8,
    hook: *const u8,
    original: Vec<u8>,
    trampoline: *mut u8,
    trampoline_size: usize,
    installed: bool,
}

impl Detour {
    /// Create a detour (not yet installed).
    ///
    /// `target` — address of the function to hook.
    /// `hook` — address of our replacement function.
    ///
    /// Copies the first `JMP_SIZE` bytes to a trampoline.
    /// Returns None if trampoline allocation fails (non-Windows target).
    pub unsafe fn new(target: *mut u8, hook: *const u8) -> Option<Self> {
        // Save original bytes
        let original = {
            let slice = ptr::slice_from_raw_parts(target as *const u8, JMP_SIZE);
            (*slice).to_vec()
        };

        // Allocate executable memory for trampoline
        let trampoline = memory::alloc_exec(TRAMPOLINE_SIZE)?;

        // Build trampoline: [original_bytes][JMP target+JMP_SIZE]
        ptr::copy_nonoverlapping(original.as_ptr(), trampoline, JMP_SIZE);

        let jmp_back_offset =
            (target as isize + JMP_SIZE as isize
             - (trampoline as isize + JMP_SIZE as isize)
             - 5) as i32;
        ptr::write_volatile(trampoline.add(JMP_SIZE), JMP_OPCODE);
        ptr::write_volatile(
            trampoline.add(JMP_SIZE + 1) as *mut i32,
            jmp_back_offset,
        );

        Some(Detour {
            target,
            hook,
            original,
            trampoline,
            trampoline_size: TRAMPOLINE_SIZE,
            installed: false,
        })
    }

    /// Install the detour: write JMP target→hook.
    /// No-op if already installed.
    pub unsafe fn install(&mut self) {
        if self.installed {
            return;
        }
        let jmp_offset =
            (self.hook as isize - self.target as isize - JMP_SIZE as isize) as i32;
        memory::safe_write8(self.target as usize, JMP_OPCODE);
        memory::safe_write32(self.target as usize + 1, jmp_offset as u32);
        self.installed = true;
    }

    /// Uninstall: restore original bytes at target.
    /// No-op if not installed.
    pub unsafe fn uninstall(&mut self) {
        if !self.installed {
            return;
        }
        memory::safe_write_buf(self.target as usize, &self.original);
        self.installed = false;
    }

    /// Return a function pointer to the trampoline (calls original function).
    ///
    /// # Safety
    /// `T` must match the calling convention of the original function.
    pub unsafe fn trampoline_ptr<T>(&self) -> T {
        std::mem::transmute_copy(&self.trampoline)
    }
}

impl Drop for Detour {
    fn drop(&mut self) {
        unsafe {
            if self.installed {
                self.uninstall();
            }
            if !self.trampoline.is_null() {
                memory::free_exec(self.trampoline, self.trampoline_size);
            }
        }
    }
}

// ── Self-check ──

#[cfg(test)]
mod tests {
    use super::*;

    extern "C" fn original_fn() -> i32 {
        42
    }

    extern "C" fn hook_fn() -> i32 {
        99
    }

    #[test]
    fn test_detour_new_saves_original_bytes() {
        unsafe {
            let orig = original_fn as *mut u8;
            let hook = hook_fn as *const u8;
            let d = Detour::new(orig, hook);
            // On non-Windows: trampoline alloc fails → None
            if let Some(detour) = d {
                // Original first byte should be something (MSVC prologue)
                assert!(!detour.original.is_empty());
                assert_eq!(detour.original.len(), JMP_SIZE);
            }
        }
    }

    #[test]
    fn test_detour_install_uninstall_cycle() {
        unsafe {
            let orig = original_fn as *mut u8;
            let hook = hook_fn as *const u8;
            let d = Detour::new(orig, hook);
            if d.is_none() {
                // Non-Windows — skip
                return;
            }
            let mut detour = d.unwrap();

            assert!(!detour.installed);
            detour.install();
            assert!(detour.installed);

            // Double install = no-op
            detour.install();
            assert!(detour.installed);

            detour.uninstall();
            assert!(!detour.installed);

            // Double uninstall = no-op
            detour.uninstall();
            assert!(!detour.installed);

            // Original function still works
            assert_eq!(original_fn(), 42);
        }
    }

    #[test]
    fn test_detour_trampoline_calls_original() {
        unsafe {
            let orig = original_fn as *mut u8;
            let hook = hook_fn as *const u8;
            let d = Detour::new(orig, hook);
            if d.is_none() {
                return;
            }
            let detour = d.unwrap();

            // Even uninstalled, trampoline should call original
            let tramp: extern "C" fn() -> i32 = detour.trampoline_ptr();
            assert_eq!(tramp(), 42);
        }
    }
}
