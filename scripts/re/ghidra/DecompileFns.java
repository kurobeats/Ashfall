// Ghidra headless post-script: decompile a list of functions to C.
// Usage:
//   analyzeHeadless <proj> -process <file> -noanalysis \
//       -postScript DecompileFns.java <spec.json> <out.txt> -scriptPath <dir>
// Spec: { "fns": [ { "addr": "0x...", "name": "..." } ] }

import java.io.File;
import java.io.FileReader;
import java.io.PrintWriter;
import java.util.ArrayList;
import java.util.List;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;

public class DecompileFns extends GhidraScript {

    private long toLong(JsonElement e) {
        String s = e.getAsString();
        return s.startsWith("0x") ? Long.parseLong(s.substring(2), 16) : Long.parseLong(s);
    }

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) {
            println("usage: DecompileFns.java <spec.json> <out.txt>");
            return;
        }
        JsonObject spec;
        try (FileReader fr = new FileReader(args[0])) {
            spec = JsonParser.parseReader(fr).getAsJsonObject();
        }
        FunctionManager fm = currentProgram.getFunctionManager();
        DecompInterface decomp = new DecompInterface();
        decomp.openProgram(currentProgram);
        List<String> out = new ArrayList<>();
        for (JsonElement fe : spec.getAsJsonArray("fns")) {
            JsonObject f = fe.getAsJsonObject();
            long addr = toLong(f.get("addr"));
            String name = f.get("name").getAsString();
            Address a = currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(addr);
            Function fn = fm.getFunctionAt(a);
            if (fn == null) {
                fn = fm.getFunctionContaining(a);
            }
            out.add("/* ===== " + name + " @ 0x" + Long.toHexString(addr) + " ===== */");
            if (fn == null) {
                out.add("// no function at or containing address");
                continue;
            }
            out.add("// containing fn: " + fn.getName() + " @ " + fn.getEntryPoint());
            DecompileResults res = decomp.decompileFunction(fn, 120, monitor);
            if (res != null && res.decompileCompleted()) {
                out.add(res.getDecompiledFunction().getC());
            } else {
                out.add("// decompile failed");
            }
        }
        try (PrintWriter pw = new PrintWriter(new File(args[1]))) {
            for (String s : out) {
                pw.println(s);
            }
        }
        println("DecompileFns: wrote " + args[1] + " (" + out.size() + " fns)");
        decomp.dispose();
    }
}
