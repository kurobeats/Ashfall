use std::arch::asm;

#[inline(never)]
unsafe fn vcall0<R: Copy>(obj: *mut u8, index: usize) -> R {
    let entry: usize = *((*(obj as *const *const usize)).add(index));
    let mut ret: usize = 0;
    asm!(
        "mov ecx, {this}",
        "call eax",
        "mov edi, eax",
        "mov {ret}, edi",
        inout("eax") entry => _,
        this = in(reg) obj as usize,
        ret = out(reg) ret,
        out("ecx") _, out("edx") _, out("edi") _,
    );
    std::mem::transmute_copy(&ret)
}

// thiscall target: this in ecx, returns this+7
#[no_mangle]
unsafe extern "C" fn target_fn() -> u32 {
    let this: usize;
    asm!("mov {0}, ecx", out(reg) this);
    (this + 7) as u32
}

fn main() {
    unsafe {
        let vtable: [usize; 4] = [target_fn as usize, 0, 0, 0];
        let mut object: usize = vtable.as_ptr() as usize;
        let r: u32 = vcall0(&mut object as *mut usize as *mut u8, 0);
        let expect = (&mut object as *mut usize as usize) + 7;
        println!("thiscall vcall0: r={r:#x} expect={expect:#x} {}", if r == expect as u32 { "PASS" } else { "FAIL" });
        std::process::exit(if r == expect as u32 { 0 } else { 1 });
    }
}
