// Ghidra headless post-script: BFS call-graph reachability.
// Usage: analyzeHeadless <proj> -process <file> -noanalysis \
//     -postScript CallGraph.java <start> <target> <depth> -scriptPath <dir>
// Prints whether <target> is reachable from <start> via call references,
// and the shortest path. Uses the analyzed function call graph.

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;
import ghidra.program.model.symbol.RefType;

public class CallGraph extends GhidraScript {

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 3) {
            println("usage: CallGraph.java <start> <target> <depth>");
            return;
        }
        long start = Long.parseLong(args[0].startsWith("0x") ? args[0].substring(2) : args[0], 16);
        long target = Long.parseLong(args[1].startsWith("0x") ? args[1].substring(2) : args[1], 16);
        int maxDepth = Integer.parseInt(args[2]);

        FunctionManager fm = currentProgram.getFunctionManager();
        Address targetAddr = currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(target);
        Function targetFn = fm.getFunctionContaining(targetAddr);
        if (targetFn == null) {
            println("target 0x" + Long.toHexString(target) + " has no function");
            return;
        }
        // BFS
        java.util.Map<Long, Long> parent = new java.util.HashMap<>();
        java.util.ArrayDeque<Long> queue = new java.util.ArrayDeque<>();
        java.util.Set<Long> visited = new java.util.HashSet<>();
        queue.add(start);
        visited.add(start);
        int depth = 0;
        boolean found = false;
        long foundNode = 0;
        while (!queue.isEmpty() && depth <= maxDepth) {
            int levelSize = queue.size();
            for (int i = 0; i < levelSize; i++) {
                long cur = queue.poll();
                Function fn = fm.getFunctionContaining(
                        currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(cur));
                if (fn == null) continue;
                if (fn.getEntryPoint().equals(targetFn.getEntryPoint())) {
                    found = true;
                    foundNode = cur;
                    break;
                }
                ghidra.program.model.address.AddressIterator body = fn.getBody().getAddresses(true);
                while (body.hasNext()) {
                    ghidra.program.model.address.Address a = body.next();
                    for (Reference r : currentProgram.getReferenceManager().getReferencesFrom(a)) {
                        if (!r.getReferenceType().isCall()) continue;
                        long callee = r.getToAddress().getOffset();
                        if (visited.contains(callee)) continue;
                        visited.add(callee);
                        parent.put(callee, cur);
                        queue.add(callee);
                    }
                }
            }
            if (found) break;
            depth++;
        }
        if (found) {
            // reconstruct path
            StringBuilder path = new StringBuilder();
            long n = targetFn.getEntryPoint().getOffset();
            while (n != start) {
                path.insert(0, String.format("0x%08X <- ", n));
                n = parent.getOrDefault(n, start);
            }
            path.insert(0, String.format("0x%08X <- ", start));
            println("REACHABLE (depth " + depth + "): " + path);
        } else {
            println("NOT reachable from 0x" + Long.toHexString(start) + " within depth " + maxDepth
                    + " (visited " + visited.size() + " fns)");
        }
    }
}
