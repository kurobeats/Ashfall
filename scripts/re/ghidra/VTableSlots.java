// Ghidra headless post-script: dump a vtable's slots with decompile-based
// structural fingerprints + dispatch-site xrefs. The fingerprints survive
// recompiles (byte-identity doesn't): the normalized decompiled C is hashed,
// so GOG↔Steam slot twins can be matched across builds.
//
// Usage:
//   analyzeHeadless <proj> -process <file> -noanalysis \
//       -postScript VTableSlots.java <base> <count> <out.txt> -scriptPath <dir>
//
// Output per slot: idx|offset|target|fn_size|code_hash|dispatch_sites
//   code_hash = SHA1 of the sorted set of normalized code lines
//   dispatch_sites = comma-joined "enclosingFn@site" addresses of code that
//                    references the slot (call [reg+off] / mov [reg+off])

import java.io.File;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;

public class VTableSlots extends GhidraScript {

    private Memory mem;
    private FunctionManager fm;

    private long rd32(long a) {
        byte[] b = new byte[4];
        try {
            mem.getBytes(at(a), b);
            return (b[0] & 0xFFL) | ((b[1] & 0xFFL) << 8) | ((b[2] & 0xFFL) << 16) | ((b[3] & 0xFFL) << 24);
        } catch (Exception e) {
            return 0;
        }
    }

    private Address at(long a) {
        return currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(a);
    }

    // Strip everything that differs across recompiles: addresses, fn names,
    // data labels, numeric literals. Keep structure + mnemonics.
    private String normalize(String c) {
        StringBuilder sb = new StringBuilder();
        for (String line : c.split("\n")) {
            String t = line.trim();
            if (t.isEmpty() || t.startsWith("/*") || t.startsWith("*/") || t.startsWith("//")) {
                continue;
            }
            t = t.replaceAll("0x[0-9a-fA-F]+", "N");
            t = t.replaceAll("FUN_[0-9a-fA-F]+", "F");
            t = t.replaceAll("DAT_[0-9a-fA-F]+", "D");
            t = t.replaceAll("PTR_[0-9a-fA-F]+", "P");
            t = t.replaceAll("\\b[0-9]+(\\.[0-9]+)?[fFlLuU]*\\b", "n");
            t = t.replaceAll("\\b(local|uStack|fStack|iStack|auStack|afStack|puStack|piStack|pfStack|ppuStack|uVar|iVar|cVar|bVar|fVar|piVar|pfVar|puVar|ppuVar|pvVar|pStack|aVar|abStack|acStack|unaff_|extraout_|sStack)[A-Za-z0-9_]*", "V");
            if (t.contains("(") || t.contains("=") || t.startsWith("return") || t.startsWith("if") || t.startsWith("switch") || t.startsWith("do") || t.startsWith("while") || t.startsWith("goto")) {
                sb.append(t).append("\n");
            }
        }
        return sb.toString();
    }

    // Sorted set of normalized code lines (for Jaccard similarity).
    private String lineSet(String c) {
        Set<String> lines = new TreeSet<>();
        for (String line : normalize(c).split("\n")) {
            if (!line.trim().isEmpty()) {
                lines.add(line.trim());
            }
        }
        return String.join("|", lines);
    }

    private String sha1(String s) {
        try {
            MessageDigest md = MessageDigest.getInstance("SHA-1");
            byte[] d = md.digest(s.getBytes(StandardCharsets.UTF_8));
            StringBuilder h = new StringBuilder();
            for (byte b : d) {
                h.append(String.format("%02x", b));
            }
            return h.substring(0, 16);
        } catch (Exception e) {
            return "ERR";
        }
    }

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 3) {
            println("usage: VTableSlots.java <base> <count> <out.txt>");
            return;
        }
        long base = Long.parseLong(args[0].startsWith("0x") ? args[0].substring(2) : args[0], 16);
        int count = Integer.parseInt(args[1]);
        mem = currentProgram.getMemory();
        fm = currentProgram.getFunctionManager();
        DecompInterface decomp = new DecompInterface();
        decomp.openProgram(currentProgram);

        List<String> out = new ArrayList<>();
        List<String> lines = new ArrayList<>();
        for (int i = 0; i < count; i++) {
            long slotAddr = base + i * 4L;
            long target = rd32(slotAddr);
            long size = 0;
            String hash = "-";
            String sites = "-";
            String lineSet = "-";
            if (target != 0) {
                Function fn = fm.getFunctionContaining(at(target));
                if (fn != null) {
                    size = fn.getBody().getNumAddresses();
                    DecompileResults res = decomp.decompileFunction(fn, 120, monitor);
                    if (res != null && res.decompileCompleted()) {
                        String c = res.getDecompiledFunction().getC();
                        hash = sha1(normalize(c));
                        lineSet = lineSet(c);
                    }
                }
                // dispatch sites referencing this slot
                StringBuilder sitesSb = new StringBuilder();
                ReferenceIterator it = currentProgram.getReferenceManager().getReferencesTo(at(slotAddr));
                while (it.hasNext()) {
                    Reference r = it.next();
                    Address from = r.getFromAddress();
                    Function owner = fm.getFunctionContaining(from);
                    String oname = owner != null ? owner.getEntryPoint().toString() : "?";
                    if (sitesSb.length() > 0) sitesSb.append(",");
                    sitesSb.append(oname).append("@").append(from);
                }
                sites = sitesSb.length() > 0 ? sitesSb.toString() : "-";
            }
            out.add(i + "|+0x" + String.format("%03X", i * 4) + "|0x" + Long.toHexString(target)
                    + "|" + size + "|" + hash + "|" + (sites));
            lines.add(i + "|+0x" + String.format("%03X", i * 4) + "|" + lineSet);
        }
        try (PrintWriter pw = new PrintWriter(new File(args[2]))) {
            for (String s : out) {
                pw.println(s);
            }
        }
        try (PrintWriter pw = new PrintWriter(new File(args[2] + ".lines"))) {
            for (String s : lines) {
                pw.println(s);
            }
        }
        println("VTableSlots: wrote " + args[2] + " (+.lines) (" + count + " slots)");
        decomp.dispose();
    }
}
