// Ghidra headless post-script: verify Ashfall RE address tables against a
// loaded image (Ghidra 12+ — Jython removed, Java scripts required).
//
// Usage:
//   analyzeHeadless <proj_dir> <proj> -process <file> -noanalysis \
//       -postScript VerifyRE.java <spec.json> <out.txt> -scriptPath <dir>
//
// Spec schema (JSON, parsed with bundled Gson): { "checks": [ ... ] }
//   byte     { type, addr, bytes }        exact byte match at addr
//   fn       { type, addr }               function starts at addr
//   vtable   { type, base, slots: { "+0x1EC": "0x76CD00" | "" } }
//   cmdtable { type, base, stride, count, entries: [{name, handler}] }
//   xref     { type, addr, min }
//   ptr      { type, addr }               report u32 at addr (INFO)

import java.io.File;
import java.io.FileReader;
import java.io.PrintWriter;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryAccessException;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;

public class VerifyRE extends GhidraScript {

    private Memory mem;
    private FunctionManager fm;
    private final List<String> results = new ArrayList<>();

    private long toLong(JsonElement e) {
        String s = e.getAsString();
        return s.startsWith("0x") ? Long.parseLong(s.substring(2), 16) : Long.parseLong(s);
    }

    private Address at(long a) {
        return currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(a);
    }

    private byte[] rd(long a, int n) {
        byte[] b = new byte[n];
        try {
            mem.getBytes(at(a), b);
            return b;
        } catch (MemoryAccessException e) {
            return null;
        }
    }

    private String hexs(byte[] b) {
        StringBuilder sb = new StringBuilder();
        for (byte x : b) {
            sb.append(String.format("%02x", x));
        }
        return sb.toString();
    }

    private void byteCheck(JsonObject c) {
        String name = c.get("name").getAsString();
        long addr = toLong(c.get("addr"));
        byte[] exp = hexToBytes(c.get("bytes").getAsString());
        byte[] got = rd(addr, exp.length);
        if (got == null) {
            results.add("FAIL " + name + " @ 0x" + Long.toHexString(addr) + ": read-fail");
        } else if (java.util.Arrays.equals(got, exp)) {
            results.add("PASS " + name + " @ 0x" + Long.toHexString(addr));
        } else {
            results.add("FAIL " + name + " @ 0x" + Long.toHexString(addr)
                    + ": want " + c.get("bytes").getAsString() + " got " + hexs(got));
        }
    }

    private static byte[] hexToBytes(String s) {
        int n = s.length() / 2;
        byte[] out = new byte[n];
        for (int i = 0; i < n; i++) {
            out[i] = (byte) Integer.parseInt(s.substring(2 * i, 2 * i + 2), 16);
        }
        return out;
    }

    private void fnCheck(JsonObject c) {
        String name = c.get("name").getAsString();
        long addr = toLong(c.get("addr"));
        Function f = fm.getFunctionAt(at(addr));
        if (f != null) {
            results.add("PASS fn " + name + " @ 0x" + Long.toHexString(addr) + " (" + f.getName() + ")");
        } else {
            results.add("FAIL fn " + name + " @ 0x" + Long.toHexString(addr) + ": no function");
        }
    }

    private void vtableCheck(JsonObject c) {
        String name = c.get("name").getAsString();
        long base = toLong(c.get("base"));
        for (Map.Entry<String, JsonElement> e : c.getAsJsonObject("slots").entrySet()) {
            long slot = Long.parseLong(e.getKey().replaceAll("[^0-9a-fA-F]", ""), 16);
            String expect = e.getValue().getAsString();
            byte[] p = rd(base + slot, 4);
            if (p == null) {
                results.add("FAIL " + name + "+" + e.getKey() + ": read-fail");
                continue;
            }
            long target = (p[0] & 0xFFL) | ((p[1] & 0xFFL) << 8) | ((p[2] & 0xFFL) << 16) | ((p[3] & 0xFFL) << 24);
            if (!expect.isEmpty()) {
                long exp = Long.parseLong(expect.substring(2), 16);
                if (target == exp) {
                    results.add("PASS " + name + "+" + e.getKey() + " -> 0x" + String.format("%08x", target));
                } else {
                    results.add("FAIL " + name + "+" + e.getKey() + ": want 0x"
                            + String.format("%08x", exp) + " got 0x" + String.format("%08x", target));
                }
            } else {
                results.add("INFO " + name + "+" + e.getKey() + " -> 0x" + String.format("%08x", target));
            }
        }
    }

    private void cmdTableCheck(JsonObject c) {
        String name = c.get("name").getAsString();
        long base = toLong(c.get("base"));
        int stride = c.get("stride").getAsInt();
        int count = c.get("count").getAsInt();
        int nameOff = c.has("name_off") ? c.get("name_off").getAsInt() : 0;
        int handlerOff = c.has("handler_off") ? c.get("handler_off").getAsInt() : 0x18;
        boolean nameIsPtr = c.has("name_ptr") && c.get("name_ptr").getAsBoolean();
        JsonArray entries = c.getAsJsonArray("entries");
        for (JsonElement ee : entries) {
            JsonObject entry = ee.getAsJsonObject();
            String nm = entry.get("name").getAsString();
            String handlerStr = entry.get("handler").getAsString();
            boolean reportOnly = handlerStr.isEmpty() || handlerStr.equals("0");
            long handler = reportOnly ? 0 : Long.parseLong(handlerStr.substring(2), 16);
            boolean found = false;
            if (nm.equals("*")) {
                // list every entry: index, name, handler@handlerOff
                for (int i = 0; i < count; i++) {
                    String s = entryName(base, stride, i, nameOff, nameIsPtr);
                    if (s == null) {
                        results.add("INFO cmdtable * idx " + i + ": read-fail");
                        continue;
                    }
                    byte[] h = rd(base + (long) i * stride + handlerOff, 4);
                    long ht = h == null ? -1 : (h[0] & 0xFFL) | ((h[1] & 0xFFL) << 8) | ((h[2] & 0xFFL) << 16) | ((h[3] & 0xFFL) << 24);
                    results.add("INFO cmdtable * idx " + i + " " + s + " -> 0x" + String.format("%08x", ht));
                }
                continue;
            }
            for (int i = 0; i < count; i++) {
                String s = entryName(base, stride, i, nameOff, nameIsPtr);
                if (s == null) break;
                if (s.equals(nm)) {
                    byte[] h = rd(base + (long) i * stride + handlerOff, 4);
                    long ht = h == null ? -1 : (h[0] & 0xFFL) | ((h[1] & 0xFFL) << 8) | ((h[2] & 0xFFL) << 16) | ((h[3] & 0xFFL) << 24);
                    if (reportOnly) {
                        results.add("INFO cmdtable " + nm + " idx " + i + " -> 0x" + String.format("%08x", ht));
                    } else if (ht == handler) {
                        results.add("PASS cmdtable " + nm + " idx " + i + " -> 0x" + String.format("%08x", ht));
                    } else {
                        results.add("FAIL cmdtable " + nm + " idx " + i + ": want 0x"
                                + String.format("%08x", handler) + " got 0x" + String.format("%08x", ht));
                    }
                    found = true;
                    break;
                }
            }
            if (!found) {
                results.add("FAIL cmdtable " + nm + ": not found in " + count + " entries");
            }
        }
    }

    private void xrefCheck(JsonObject c) {
        String name = c.get("name").getAsString();
        long addr = toLong(c.get("addr"));
        int min = c.get("min").getAsInt();
        int n = 0;
        ReferenceIterator it = currentProgram.getReferenceManager().getReferencesTo(at(addr));
        while (it.hasNext()) {
            it.next();
            n++;
        }
        if (n >= min) {
            results.add("PASS xref " + name + ": " + n + " refs");
        } else {
            results.add("FAIL xref " + name + ": " + n + " refs < min " + min);
        }
    }

    private String entryName(long base, int stride, int i, int nameOff, boolean nameIsPtr) {
        byte[] raw;
        if (nameIsPtr) {
            byte[] p = rd(base + (long) i * stride + nameOff, 4);
            if (p == null) return null;
            long np = (p[0] & 0xFFL) | ((p[1] & 0xFFL) << 8) | ((p[2] & 0xFFL) << 16) | ((p[3] & 0xFFL) << 24);
            if (np == 0) return "";
            raw = rd(np, 64);
        } else {
            raw = rd(base + (long) i * stride + nameOff, 64);
        }
        if (raw == null) return null;
        String s = new String(raw);
        int z = s.indexOf('\0');
        return z >= 0 ? s.substring(0, z) : s;
    }

    private void ptrCheck(JsonObject c) {
        String name = c.get("name").getAsString();
        long addr = toLong(c.get("addr"));
        byte[] p = rd(addr, 4);
        if (p == null) {
            results.add("FAIL ptr " + name + ": read-fail");
        } else {
            long target = (p[0] & 0xFFL) | ((p[1] & 0xFFL) << 8) | ((p[2] & 0xFFL) << 16) | ((p[3] & 0xFFL) << 24);
            String block = currentProgram.getMemory().getBlock(at(addr)) != null
                    ? currentProgram.getMemory().getBlock(at(addr)).getName() : "?";
            results.add("INFO ptr " + name + " -> 0x" + String.format("%08x", target) + " (block " + block + ")");
        }
    }

    private void dumpCheck(JsonObject c) {
        String name = c.get("name").getAsString();
        long addr = toLong(c.get("addr"));
        int len = c.get("len").getAsInt();
        byte[] d = rd(addr, len);
        results.add("DUMP " + name + " @ 0x" + Long.toHexString(addr) + " (" + (d == null ? "read-fail" : hexs(d)) + ")");
    }

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) {
            println("usage: VerifyRE.java <spec.json> <out.txt>");
            return;
        }
        mem = currentProgram.getMemory();
        fm = currentProgram.getFunctionManager();

        JsonObject spec;
        try (FileReader fr = new FileReader(args[0])) {
            spec = JsonParser.parseReader(fr).getAsJsonObject();
        }
        JsonArray checks = spec.getAsJsonArray("checks");
        for (JsonElement ce : checks) {
            JsonObject c = ce.getAsJsonObject();
            try {
                switch (c.get("type").getAsString()) {
                    case "byte": byteCheck(c); break;
                    case "fn": fnCheck(c); break;
                    case "vtable": vtableCheck(c); break;
                    case "cmdtable": cmdTableCheck(c); break;
                    case "xref": xrefCheck(c); break;
                    case "ptr": ptrCheck(c); break;
                    case "dump": dumpCheck(c); break;
                    default: results.add("ERROR unknown type " + c.get("type").getAsString());
                }
            } catch (Exception e) {
                results.add("ERROR " + c.get("name").getAsString() + ": " + e);
            }
        }
        try (PrintWriter pw = new PrintWriter(new File(args[1]))) {
            for (String r : results) {
                pw.println(r);
            }
        }
        println("VerifyRE: wrote " + args[1] + " (" + results.size() + " checks)");
    }
}
