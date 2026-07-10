//! Safe memory patching primitives for the Ashfall bridge DLL.
//!
//! Ported from vaultmp-extended vaultmpdll/vaultmp.cpp (SafeWrite8/16/32/Buf,
//! WriteRelJump, WriteRelCall) and vaultgui/Hook.cpp (HookJmp, HookCall).
//!
//! All functions operate on raw `usize` addresses — the bridge lives inside
//! the game process, so addresses are direct virtual memory pointers.
//!
//! # Platform
//!
//! On Windows: uses VirtualProtect/VirtualAlloc/VirtualFree via windows-sys.
//! On non-Windows: no-op stubs (tests use direct buffer writes, no protect needed).

use std::ptr;

// ═══════════════════════════════════════════════════════════════
// Windows implementation
// ═══════════════════════════════════════════════════════════════

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use windows_sys::Win32::System::Memory::{
        VirtualProtect, VirtualAlloc, VirtualFree,
        MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
    };

    // ── RAII memory protection guard ──

    pub struct MemoryProtect {
        addr: *const u8,
        size: usize,
        old_protect: u32,
    }

    impl MemoryProtect {
        pub unsafe fn new(addr: *const u8, size: usize) -> Option<Self> {
            let mut old_protect: u32 = 0;
            let ok = VirtualProtect(addr as _, size, PAGE_EXECUTE_READWRITE, &mut old_protect);
            if ok == 0 {
                return None;
            }
            Some(Self { addr, size, old_protect })
        }
    }

    impl Drop for MemoryProtect {
        fn drop(&mut self) {
            unsafe {
                let mut old: u32 = 0;
                VirtualProtect(self.addr as _, self.size, self.old_protect, &mut old);
            }
        }
    }

    /// Write a single u8 at `addr`.
    pub unsafe fn safe_write8(addr: usize, value: u8) {
        let mut old: u32 = 0;
        VirtualProtect(addr as _, 1, PAGE_EXECUTE_READWRITE, &mut old);
        ptr::write_volatile(addr as *mut u8, value);
        VirtualProtect(addr as _, 1, old, &mut old);
    }

    /// Write a single u16 at `addr`.
    pub unsafe fn safe_write16(addr: usize, value: u16) {
        let mut old: u32 = 0;
        VirtualProtect(addr as _, 2, PAGE_EXECUTE_READWRITE, &mut old);
        ptr::write_volatile(addr as *mut u16, value);
        VirtualProtect(addr as _, 2, old, &mut old);
    }

    /// Write a single u32 at `addr`.
    pub unsafe fn safe_write32(addr: usize, value: u32) {
        let mut old: u32 = 0;
        VirtualProtect(addr as _, 4, PAGE_EXECUTE_READWRITE, &mut old);
        ptr::write_volatile(addr as *mut u32, value);
        VirtualProtect(addr as _, 4, old, &mut old);
    }

    /// Write a byte buffer at `addr`.
    pub unsafe fn safe_write_buf(addr: usize, data: &[u8]) {
        let mut old: u32 = 0;
        VirtualProtect(addr as _, data.len(), PAGE_EXECUTE_READWRITE, &mut old);
        ptr::copy_nonoverlapping(data.as_ptr(), addr as *mut u8, data.len());
        VirtualProtect(addr as _, data.len(), old, &mut old);
    }

    /// Allocate executable memory.
    pub unsafe fn alloc_exec(size: usize) -> Option<*mut u8> {
        let ptr = VirtualAlloc(ptr::null(), size, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE);
        if ptr.is_null() { None } else { Some(ptr as *mut u8) }
    }

    /// Free executable memory.
    pub unsafe fn free_exec(ptr: *mut u8, size: usize) {
        VirtualFree(ptr as _, size, MEM_RELEASE);
    }
}

// ═══════════════════════════════════════════════════════════════
// Non-Windows stubs (for local testing without VirtualProtect)
// ═══════════════════════════════════════════════════════════════

#[cfg(not(target_os = "windows"))]
mod non_windows_impl {
    use super::*;

    pub struct MemoryProtect {
        _addr: *const u8,
    }

    impl MemoryProtect {
        pub unsafe fn new(_addr: *const u8, _size: usize) -> Option<Self> {
            // ponytail: non-Windows: no memory protection needed for tests
            Some(Self { _addr })
        }
    }

    impl Drop for MemoryProtect {
        fn drop(&mut self) {}
    }

    pub unsafe fn safe_write8(addr: usize, value: u8) {
        ptr::write_volatile(addr as *mut u8, value);
    }

    pub unsafe fn safe_write16(addr: usize, value: u16) {
        ptr::write_volatile(addr as *mut u16, value);
    }

    pub unsafe fn safe_write32(addr: usize, value: u32) {
        ptr::write_volatile(addr as *mut u32, value);
    }

    pub unsafe fn safe_write_buf(addr: usize, data: &[u8]) {
        ptr::copy_nonoverlapping(data.as_ptr(), addr as *mut u8, data.len());
    }

    pub unsafe fn alloc_exec(_size: usize) -> Option<*mut u8> {
        None
    }

    pub unsafe fn free_exec(ptr: *mut u8, _size: usize) {
        // ponytail: if non-Windows, ptr is from some other allocator, no-op here
        let _ = ptr;
    }
}

// ── Re-export the platform-specific items ──

#[cfg(target_os = "windows")]
pub use windows_impl::*;

#[cfg(not(target_os = "windows"))]
pub use non_windows_impl::*;

// ── Patch: save/restore byte pattern (cross-platform) ──

pub struct Patch {
    addr: *const u8,
    original: Vec<u8>,
    patch_data: Vec<u8>,
}

impl Patch {
    pub unsafe fn new(addr: *const u8, data: &[u8]) -> Self {
        let mut original = Vec::with_capacity(data.len());
        original.extend_from_slice(std::slice::from_raw_parts(addr, data.len()));
        Patch { addr, original, patch_data: data.to_vec() }
    }

    pub unsafe fn apply(&self) -> Option<MemoryProtect> {
        let guard = MemoryProtect::new(self.addr, self.patch_data.len())?;
        ptr::copy_nonoverlapping(
            self.patch_data.as_ptr(),
            self.addr as *mut u8,
            self.patch_data.len(),
        );
        Some(guard)
    }

    pub unsafe fn restore(&self) -> Option<MemoryProtect> {
        let guard = MemoryProtect::new(self.addr, self.original.len())?;
        ptr::copy_nonoverlapping(
            self.original.as_ptr(),
            self.addr as *mut u8,
            self.original.len(),
        );
        Some(guard)
    }
}

// ── Relative jumps/calls (cross-platform — pure byte writes) ──

/// Write a relative JMP (E9 [rel32]) at `from` targeting `to`.
pub unsafe fn write_rel_jump(from: usize, to: usize) {
    let offset = (to as isize - from as isize - 5) as i32;
    safe_write8(from, 0xE9);
    safe_write32(from + 1, offset as u32);
}

/// Write a relative CALL (E8 [rel32]) at `from` targeting `to`.
pub unsafe fn write_rel_call(from: usize, to: usize) {
    let offset = (to as isize - from as isize - 5) as i32;
    safe_write8(from, 0xE8);
    safe_write32(from + 1, offset as u32);
}

// ═══════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_safe_write8_readback() {
        let mut buf: [u8; 8] = [0xA5; 8];
        let addr = buf.as_mut_ptr() as usize;
        unsafe {
            safe_write8(addr, 0x42);
            safe_write8(addr + 1, 0x7B);
        }
        assert_eq!(buf[0], 0x42);
        assert_eq!(buf[1], 0x7B);
        assert_eq!(buf[2], 0xA5);
    }

    #[test]
    fn test_safe_write32() {
        let mut buf: [u32; 2] = [0; 2];
        let addr = buf.as_mut_ptr() as usize;
        unsafe {
            safe_write32(addr, 0xDEADBEEF);
        }
        assert_eq!(buf[0], 0xDEADBEEF);
        assert_eq!(buf[1], 0);
    }

    #[test]
    fn test_safe_write_buf() {
        let mut buf = [0u8; 6];
        let addr = buf.as_mut_ptr() as usize;
        unsafe {
            safe_write_buf(addr, &[0x11, 0x22, 0x33, 0x44]);
        }
        assert_eq!(&buf[..4], &[0x11, 0x22, 0x33, 0x44]);
        assert_eq!(buf[4], 0);
        assert_eq!(buf[5], 0);
    }

    #[test]
    fn test_write_rel_jump_offset() {
        let mut buf = [0u8; 5];
        let addr = buf.as_mut_ptr() as usize;
        let fake_to = addr + 0x1000;
        unsafe {
            write_rel_jump(addr, fake_to);
        }
        assert_eq!(buf[0], 0xE9);
        let written_off = i32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]);
        let expected = (fake_to as isize - addr as isize - 5) as i32;
        assert_eq!(written_off, expected);
    }

    #[test]
    fn test_write_rel_call_offset() {
        let mut buf = [0u8; 5];
        let addr = buf.as_mut_ptr() as usize;
        let fake_to = addr + 0x500;
        unsafe {
            write_rel_call(addr, fake_to);
        }
        assert_eq!(buf[0], 0xE8);
        let written_off = i32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]);
        let expected = (fake_to as isize - addr as isize - 5) as i32;
        assert_eq!(written_off, expected);
    }

    #[test]
    fn test_patch_apply_restore() {
        let mut buf = [0xAAu8, 0xBB, 0xCC, 0xDD];
        let addr = buf.as_mut_ptr();
        unsafe {
            let patch = Patch::new(addr, &[0x11, 0x22, 0x33, 0x44]);
            assert_eq!(buf, [0xAA, 0xBB, 0xCC, 0xDD]);

            let _guard = patch.apply().expect("apply should succeed");
            assert_eq!(buf, [0x11, 0x22, 0x33, 0x44]);

            let _guard = patch.restore().expect("restore should succeed");
            assert_eq!(buf, [0xAA, 0xBB, 0xCC, 0xDD]);
        }
    }

    #[test]
    fn test_memory_protect_noop() {
        let mut buf = [0xFFu8; 16];
        let addr = buf.as_mut_ptr() as *const u8;
        unsafe {
            let guard = MemoryProtect::new(addr, 16);
            assert!(guard.is_some());
            buf[0] = 0x42;
        }
        assert_eq!(buf[0], 0x42);
    }

    #[test]
    fn test_alloc_exec_non_windows_returns_none() {
        #[cfg(not(target_os = "windows"))]
        unsafe {
            assert!(alloc_exec(64).is_none());
        }
    }
}
