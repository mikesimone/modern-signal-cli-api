// Dumps the JSON-RPC surface of the signal-cli on the classpath as JSON:
//   { "<method>": { "kind": "typed"|"local", "params": ["name", ...] }, ... }
// Typed commands (finishLink, receive, register, verify, startLink, ...) take a Jackson record: its component
// names are the params and unknown ones are rejected; commands typed with Void/Map ignore any param. Local
// commands (send, getAttachment, ...) are argparse commands: signal-cli reads a param under the option's dest as
// declared (dashed, e.g. "group-id") or its camelCase ("groupId"), and list-valued ones also under a plural "s"
// form (JsonRpcNamespace.get/getList); any other spelling (e.g. "group_id") is silently ignored - verified
// against a live daemon. "account" selects the account for every command except JsonRpcMultiCommand ones.
// Run: java -cp 'lib/*' SpecDump.java
import java.lang.reflect.*;
import java.util.*;
import java.util.regex.*;

public class SpecDump {
    static String camel(String dashed) {
        StringBuilder sb = new StringBuilder();
        boolean up = false;
        for (char c : dashed.toCharArray()) {
            if (c == '-' || c == '_') { up = true; continue; }
            sb.append(up ? Character.toUpperCase(c) : c);
            up = false;
        }
        return sb.toString();
    }

    static String q(String s) { return "\"" + s.replace("\\", "\\\\").replace("\"", "\\\"") + "\""; }

    public static void main(String[] args) throws Exception {
        Class<?> commandsCls = Class.forName("org.asamk.signal.commands.Commands");
        Field f = commandsCls.getDeclaredField("commands");
        f.setAccessible(true);
        @SuppressWarnings("unchecked")
        Map<String, Object> commands = new TreeMap<>((Map<String, Object>) f.get(null));
        @SuppressWarnings("unchecked")
        Map<String, Object> attachers = new HashMap<>((Map<String, Object>) commandsCls.getMethod("getCommandSubparserAttachers").invoke(null));
        Class<?> jsonRpcCmd = Class.forName("org.asamk.signal.commands.JsonRpcCommand");
        Class<?> jsonRpcLocal = Class.forName("org.asamk.signal.commands.JsonRpcLocalCommand");

        List<String> out = new ArrayList<>();
        for (var e : commands.entrySet()) {
            Object cmd = e.getValue();
            if (!jsonRpcCmd.isInstance(cmd)) continue;          // CLI-only, not reachable over JSON-RPC
            String name = e.getKey();
            TreeSet<String> params = new TreeSet<>();
            String kind;
            if (jsonRpcLocal.isInstance(cmd)) {
                kind = "local";
                Object attacher = attachers.get(name);
                var parser = net.sourceforge.argparse4j.ArgumentParsers.newFor("signal-cli").build();
                var sub = parser.addSubparsers().addParser(name);
                for (Method m : attacher.getClass().getMethods())
                    if (m.getName().equals("attachToSubparser") && m.getParameterCount() == 1) { m.setAccessible(true); m.invoke(attacher, sub); break; }
                String help = sub.formatHelp();
                boolean positional = false;
                for (String line : help.split("\n")) {
                    if (line.startsWith("positional arguments")) { positional = true; continue; }
                    if (line.startsWith("named arguments") || line.startsWith("optional arguments")) { positional = false; continue; }
                    if (line.startsWith("subcommands")) break;
                    Matcher m = Pattern.compile("^  (-.*?)(?:\\s{2,}.*)?$").matcher(line);
                    if (!positional && m.find()) {
                        String flags = m.group(1);
                        Matcher lf = Pattern.compile("--([a-zA-Z][a-zA-Z0-9-]*)").matcher(flags);
                        if (lf.find() && !lf.group(1).equals("help")) {
                            for (String p : new String[] { lf.group(1), camel(lf.group(1)) }) {
                                params.add(p);
                                if (flags.contains("...]")) params.add(p + "s");
                            }
                        }
                    } else if (positional) {
                        Matcher pm = Pattern.compile("^  ([a-zA-Z][a-zA-Z0-9_-]*)(?:\\s{2,}.*)?$").matcher(line);
                        if (pm.find()) for (String p : new String[] { pm.group(1), camel(pm.group(1)) }) { params.add(p); params.add(p + "s"); }
                    }
                }
            } else {
                kind = "typed";
                Object rt = jsonRpcCmd.getMethod("getRequestType").invoke(cmd);
                Type t = rt == null ? null : ((com.fasterxml.jackson.core.type.TypeReference<?>) rt).getType();
                Class<?> c = t instanceof Class<?> cc ? cc : t instanceof ParameterizedType pt ? (Class<?>) pt.getRawType() : null;
                if (c != null && c.isRecord()) for (RecordComponent rc : c.getRecordComponents()) params.add(rc.getName());
                else kind = "typed-lenient";   // Void/Map: any params are accepted and ignored
            }
            StringBuilder sb = new StringBuilder();
            boolean takesAccount = !Class.forName("org.asamk.signal.commands.JsonRpcMultiCommand").isInstance(cmd);
            sb.append(q(name)).append(": {\"kind\": ").append(q(kind)).append(", \"account\": ").append(takesAccount).append(", \"params\": [");
            sb.append(String.join(", ", params.stream().map(SpecDump::q).toList())).append("]}");
            out.add(sb.toString());
        }
        System.out.println("{\n  " + String.join(",\n  ", out) + "\n}");
    }
}
