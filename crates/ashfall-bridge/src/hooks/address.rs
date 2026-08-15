//! AutoPtr-style lazy address resolution + thiscall call shims.
//!
//! Ported from TiltedReverse (SkyrimTogetherReborn lineage): `AutoPtr`
//! resolves a function address once and caches it; `EngineAddr` selects
//! among per-build candidates by prologue signature; `call_thiscall_*`
//! invokes a thiscall function at an explicit address.
//!
//! The x86 (32-bit) thiscall convention is `__fastcall(this, edx, args...)`:
//! `this` in ECX, stack args right-to-left, callee cleans. The `edx`
//! register is a mandatory-but-junk second parameter on Win32 — Tilted's
//! ThisCall.hpp fills it with `nullptr`; we preserve it across the call
//! instead. On x86_64 the Windows ABI passes `this` in RCX and a plain
//! `extern "system"` fn-pointer call is correct (used by tests/host).

/// A candidate address with its expected prologue signature (first bytes of
/// the function, e.g. GOG FO3 `51 8b 0d` vs Steam `55 8b ec`).
pub struct Candidate {
    pub addr: usize,
    /// Expected prologue bytes (checked against the live image, first N).
    pub signature: &'static [u8],
}

/// Pick the first candidate whose prologue matches the running image.
/// Returns `fallback` when none match (non-Windows hosts, unknown builds).
pub fn select_candidate(candidates: &[Candidate], fallback: usize) -> usize {
    #[cfg(target_os = "windows")]
    {
        unsafe fn rd(addr: usize, n: usize) -> Vec<u8> {
            (0..n).map(|i| *((addr + i) as *const u8)).collect()
        }
        for c in candidates {
            // SAFETY: rd dereferences game-memory addresses for the
            // candidate prologue check; the sites are known code in the
            // loaded exe (validated by the address tables).
            if unsafe { rd(c.addr, c.signature.len()) } == c.signature {
                return c.addr;
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = candidates;
    }
    fallback
}

/// Lazily-resolved, cached function address. Resolves once (via `resolve`)
/// on first `get`, then serves the cached value — STR `AutoPtr` semantics.
pub struct AutoPtr {
    addr: std::sync::OnceLock<usize>,
    resolve: fn() -> usize,
}

impl AutoPtr {
    pub const fn new(resolve: fn() -> usize) -> Self {
        Self {
            addr: std::sync::OnceLock::new(),
            resolve,
        }
    }

    /// Resolved address (resolves on first call).
    pub fn get(&self) -> usize {
        *self.addr.get_or_init(self.resolve)
    }

    pub fn get_ptr<T>(&self) -> *const T {
        self.get() as *const T
    }

    pub fn get_mut_ptr<T>(&self) -> *mut T {
        self.get() as *mut T
    }
}

// ═══════════════════════════════════════════════════════════════
// thiscall call shims at explicit addresses
// ═══════════════════════════════════════════════════════════════

/// Call a thiscall function at `addr` with `this` only.
#[cfg(target_arch = "x86")]
pub unsafe fn call_thiscall_0<R: Copy>(addr: usize, this: *mut u8) -> R {
    let mut ret: usize = 0;
    core::arch::asm!(
        "push ecx",
        "push edx",
        "mov ecx, {this}",
        "mov eax, {addr}",
        "call eax",
        "mov edi, eax",
        "mov {ret}, edi",
        "pop edx",
        "pop ecx",
        addr = in(reg) addr,
        this = in(reg) this as usize,
        ret = out(reg) ret,
        out("edi") _,
        out("eax") _,
    );
    std::mem::transmute_copy(&ret)
}

/// Call a thiscall function at `addr` with `this` + one stack argument.
#[cfg(target_arch = "x86")]
pub unsafe fn call_thiscall_1<T: Copy, R: Copy>(addr: usize, this: *mut u8, a1: T) -> R {
    let arg: usize = std::mem::transmute_copy(&a1);
    let mut ret: usize = 0;
    core::arch::asm!(
        "push ecx",
        "push edx",
        "mov ecx, {this}",
        "mov eax, {addr}",
        "push {arg}",
        "call eax",
        "mov edi, eax",
        "mov {ret}, edi",
        "pop edx",
        "pop ecx",
        addr = in(reg) addr,
        this = in(reg) this as usize,
        arg = in(reg) arg,
        ret = out(reg) ret,
        out("edi") _,
        out("eax") _,
    );
    std::mem::transmute_copy(&ret)
}

/// Call a thiscall function at `addr` with `this` + two stack arguments.
#[cfg(target_arch = "x86")]
pub unsafe fn call_thiscall_2<T1: Copy, T2: Copy, R: Copy>(
    addr: usize,
    this: *mut u8,
    a1: T1,
    a2: T2,
) -> R {
    let arg1: usize = std::mem::transmute_copy(&a1);
    let arg2: usize = std::mem::transmute_copy(&a2);
    let mut ret: usize = 0;
    core::arch::asm!(
        "push ecx",
        "push edx",
        "mov ecx, {this}",
        "mov eax, {addr}",
        "push {arg2}",
        "push {arg1}",
        "call eax",
        "mov edi, eax",
        "mov {ret}, edi",
        "pop edx",
        "pop ecx",
        addr = in(reg) addr,
        this = in(reg) this as usize,
        arg1 = in(reg) arg1,
        arg2 = in(reg) arg2,
        ret = out(reg) ret,
        out("edi") _,
        out("eax") _,
    );
    std::mem::transmute_copy(&ret)
}

/// Call a thiscall function at `addr` with `this` + three stack arguments.
#[cfg(target_arch = "x86")]
pub unsafe fn call_thiscall_3<T1: Copy, T2: Copy, T3: Copy, R: Copy>(
    addr: usize,
    this: *mut u8,
    a1: T1,
    a2: T2,
    a3: T3,
) -> R {
    let arg1: usize = std::mem::transmute_copy(&a1);
    let arg2: usize = std::mem::transmute_copy(&a2);
    let arg3: usize = std::mem::transmute_copy(&a3);
    let mut ret: usize = 0;
    core::arch::asm!(
        "push ecx",
        "push edx",
        "mov ecx, {this}",
        "mov eax, {addr}",
        "push {arg3}",
        "push {arg2}",
        "push {arg1}",
        "call eax",
        "mov edi, eax",
        "mov {ret}, edi",
        "pop edx",
        "pop ecx",
        addr = in(reg) addr,
        this = in(reg) this as usize,
        arg1 = in(reg) arg1,
        arg2 = in(reg) arg2,
        arg3 = in(reg) arg3,
        ret = out(reg) ret,
        out("edi") _,
        out("eax") _,
    );
    std::mem::transmute_copy(&ret)
}

/// Non-x86 (x86_64 / tests): Windows x64 ABI passes `this` in RCX — a plain
/// `extern "system"` fn-pointer call is correct.
#[cfg(not(target_arch = "x86"))]
pub unsafe fn call_thiscall_0<R: Copy>(addr: usize, this: *mut u8) -> R {
    let f: unsafe extern "system" fn(*mut u8) -> R = std::mem::transmute(addr);
    f(this)
}

#[cfg(not(target_arch = "x86"))]
pub unsafe fn call_thiscall_1<T: Copy, R: Copy>(addr: usize, this: *mut u8, a1: T) -> R {
    let f: unsafe extern "system" fn(*mut u8, T) -> R = std::mem::transmute(addr);
    f(this, a1)
}

#[cfg(not(target_arch = "x86"))]
pub unsafe fn call_thiscall_2<T1: Copy, T2: Copy, R: Copy>(
    addr: usize,
    this: *mut u8,
    a1: T1,
    a2: T2,
) -> R {
    let f: unsafe extern "system" fn(*mut u8, T1, T2) -> R = std::mem::transmute(addr);
    f(this, a1, a2)
}

#[cfg(not(target_arch = "x86"))]
pub unsafe fn call_thiscall_3<T1: Copy, T2: Copy, T3: Copy, R: Copy>(
    addr: usize,
    this: *mut u8,
    a1: T1,
    a2: T2,
    a3: T3,
) -> R {
    let f: unsafe extern "system" fn(*mut u8, T1, T2, T3) -> R = std::mem::transmute(addr);
    f(this, a1, a2, a3)
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "system" fn fake_get_value(this: *mut u8) -> u32 {
        (this as usize) as u32
    }
    unsafe extern "system" fn fake_add(this: *mut u8, a: u32, b: u32) -> u32 {
        (this as usize) as u32 + a + b
    }

    #[test]
    fn test_call_thiscall_explicit_address() {
        unsafe {
            let this = 0x1000usize as *mut u8;
            let v = call_thiscall_0::<u32>(fake_get_value as *const () as usize, this);
            assert_eq!(v, 0x1000);
            let s = call_thiscall_2::<u32, u32, u32>(fake_add as *const () as usize, this, 1, 2);
            assert_eq!(s, 0x1000 + 3);
        }
    }

    #[test]
    fn test_auto_ptr_resolves_once() {
        static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        fn resolve() -> usize {
            CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            0xDEADBEEF
        }
        let ptr = AutoPtr::new(resolve);
        assert_eq!(ptr.get(), 0xDEADBEEF);
        assert_eq!(ptr.get(), 0xDEADBEEF);
        assert_eq!(
            CALLS.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "resolved once"
        );
    }

    /// Live-image prologue reads only run on Windows; on non-Windows the
    /// fallback is always returned. (On Windows this would read unmapped
    /// test addresses — guarded out.)
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_select_candidate_fallback_off_windows() {
        let picked = select_candidate(
            &[
                Candidate {
                    addr: 0x1111,
                    signature: &[0x51, 0x8B],
                },
                Candidate {
                    addr: 0x2222,
                    signature: &[0x55, 0x8B],
                },
            ],
            0x3333,
        );
        assert_eq!(picked, 0x3333);
    }
}
