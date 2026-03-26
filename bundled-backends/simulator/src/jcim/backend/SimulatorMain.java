package jcim.backend;

import com.licel.jcardsim.base.Simulator;
import com.licel.jcardsim.utils.AIDUtil;
import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.io.OutputStreamWriter;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Properties;
import javacard.framework.AID;

public final class SimulatorMain {
    private static final String DEFAULT_PACKAGE_NAME = "jcim.bundled.applet";
    private static final String DEFAULT_PACKAGE_AID = "A00000006203010C01";
    private static final String DEFAULT_VERSION = "1.0";
    private static final String ATR_SYSTEM_PROPERTY = "com.licel.jcardsim.card.ATR";
    private static final PrintStream QUIET_STDOUT = new PrintStream(OutputStream.nullOutputStream());

    private SimulatorMain() {}

    public static void main(String[] args) throws Exception {
        Map<String, String> options = parseOptions(args);
        String backendKind = options.getOrDefault("--backend-kind", "simulator");
        String profileId = options.getOrDefault("--profile-id", "classic222");
        String version = options.getOrDefault("--version", profileVersion(profileId));
        String readerName = options.getOrDefault("--reader-name", "JCIM Simulation");
        String atrHex = options.getOrDefault("--atr", profileAtr(profileId));
        Path metadataPath = Path.of(requireOption(options, "--simulator-metadata"));
        if (!Files.exists(metadataPath)) {
            throw new IllegalArgumentException("simulator metadata does not exist: " + metadataPath);
        }

        RuntimeDescriptor descriptor = RuntimeDescriptor.load(metadataPath);
        ProfileSpec profile = ProfileSpec.forId(profileId, version, readerName, atrHex);
        ManagedJavaSimulatorRuntime runtime =
                new ManagedJavaSimulatorRuntime(
                        backendKind, profile, descriptor, options.get("--cap-path"));
        runtime.start();

        BufferedReader reader =
                new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        BufferedWriter writer =
                new BufferedWriter(new OutputStreamWriter(System.out, StandardCharsets.UTF_8));
        try {
            String line;
            while ((line = reader.readLine()) != null) {
                String op = tryExtractStringField(line, "op");
                if (line.trim().isEmpty()) {
                    writeLine(writer, errorReply(op, "empty command"));
                    continue;
                }
                try {
                    writeLine(writer, runtime.handleControlLine(line));
                    if ("shutdown".equals(op)) {
                        return;
                    }
                } catch (RuntimeException exception) {
                    writeLine(writer, errorReply(op, sanitizeError(exception)));
                    if ("shutdown".equals(op)) {
                        return;
                    }
                }
            }
        } finally {
            runtime.shutdown();
        }
    }

    private static String requireOption(Map<String, String> options, String key) {
        String value = options.get(key);
        if (value == null || value.isEmpty()) {
            throw new IllegalArgumentException("missing required option " + key);
        }
        return value;
    }

    private static Map<String, String> parseOptions(String[] args) {
        Map<String, String> options = new HashMap<>();
        for (int index = 0; index < args.length; index++) {
            String current = args[index];
            if (!current.startsWith("--")) {
                continue;
            }
            if (index + 1 < args.length && !args[index + 1].startsWith("--")) {
                options.put(current, args[index + 1]);
                index += 1;
            } else {
                options.put(current, "true");
            }
        }
        return options;
    }

    private static void writeLine(BufferedWriter writer, String line) throws IOException {
        writer.write(line);
        writer.write('\n');
        writer.flush();
    }

    private static String sanitizeError(RuntimeException exception) {
        String message = exception.getMessage();
        if (message == null || message.isEmpty()) {
            return exception.getClass().getSimpleName();
        }
        return message.replace('\n', ' ').replace('\r', ' ');
    }

    private static String errorReply(String op, String error) {
        return object(
                field("op", quote(op == null || op.isEmpty() ? "shutdown" : op)),
                field("ok", "false"),
                field("error", quote(error == null ? "backend error" : error)));
    }

    private static String tryExtractStringField(String json, String field) {
        String raw = extractRawFieldValue(json, field);
        if (raw == null || "null".equals(raw)) {
            return null;
        }
        return parseJsonStringLiteral(raw);
    }

    private static String requireStringField(String json, String field) {
        String value = tryExtractStringField(json, field);
        if (value == null || value.isEmpty()) {
            throw new IllegalArgumentException("missing required field `" + field + "`");
        }
        return value;
    }

    private static boolean requireBooleanField(String json, String field) {
        String raw = extractRawFieldValue(json, field);
        if ("true".equals(raw)) {
            return true;
        }
        if ("false".equals(raw)) {
            return false;
        }
        throw new IllegalArgumentException("missing or invalid boolean field `" + field + "`");
    }

    private static Integer optionalIntegerField(String json, String field) {
        String raw = extractRawFieldValue(json, field);
        if (raw == null || "null".equals(raw)) {
            return null;
        }
        return Integer.parseInt(raw);
    }

    private static long optionalLongField(String json, String field, long fallback) {
        String raw = extractRawFieldValue(json, field);
        if (raw == null || "null".equals(raw)) {
            return fallback;
        }
        return Long.parseLong(raw);
    }

    private static String extractRawFieldValue(String json, String field) {
        int keyIndex = json.indexOf("\"" + field + "\"");
        if (keyIndex < 0) {
            return null;
        }
        int colon = json.indexOf(':', keyIndex + field.length() + 2);
        if (colon < 0) {
            throw new IllegalArgumentException("invalid JSON field `" + field + "`");
        }
        int start = skipWhitespace(json, colon + 1);
        if (start >= json.length()) {
            throw new IllegalArgumentException("missing JSON value for `" + field + "`");
        }
        char marker = json.charAt(start);
        if (marker == '"') {
            int end = findJsonStringEnd(json, start + 1);
            return json.substring(start, end + 1);
        }
        int end = start;
        while (end < json.length()) {
            char current = json.charAt(end);
            if (current == ',' || current == '}' || current == ']' || Character.isWhitespace(current)) {
                break;
            }
            end += 1;
        }
        return json.substring(start, end);
    }

    private static int skipWhitespace(String value, int index) {
        int current = index;
        while (current < value.length() && Character.isWhitespace(value.charAt(current))) {
            current += 1;
        }
        return current;
    }

    private static int findJsonStringEnd(String json, int start) {
        boolean escaped = false;
        for (int index = start; index < json.length(); index++) {
            char current = json.charAt(index);
            if (escaped) {
                escaped = false;
                continue;
            }
            if (current == '\\') {
                escaped = true;
                continue;
            }
            if (current == '"') {
                return index;
            }
        }
        throw new IllegalArgumentException("unterminated JSON string");
    }

    private static String parseJsonStringLiteral(String literal) {
        if (literal == null || "null".equals(literal)) {
            return null;
        }
        if (literal.length() < 2 || literal.charAt(0) != '"' || literal.charAt(literal.length() - 1) != '"') {
            throw new IllegalArgumentException("invalid JSON string literal");
        }
        StringBuilder builder = new StringBuilder(literal.length() - 2);
        for (int index = 1; index < literal.length() - 1; index++) {
            char current = literal.charAt(index);
            if (current != '\\') {
                builder.append(current);
                continue;
            }
            if (index + 1 >= literal.length() - 1) {
                throw new IllegalArgumentException("invalid JSON escape");
            }
            char escaped = literal.charAt(++index);
            switch (escaped) {
                case '"':
                case '\\':
                case '/':
                    builder.append(escaped);
                    break;
                case 'b':
                    builder.append('\b');
                    break;
                case 'f':
                    builder.append('\f');
                    break;
                case 'n':
                    builder.append('\n');
                    break;
                case 'r':
                    builder.append('\r');
                    break;
                case 't':
                    builder.append('\t');
                    break;
                case 'u':
                    if (index + 4 >= literal.length()) {
                        throw new IllegalArgumentException("invalid JSON unicode escape");
                    }
                    builder.append((char) Integer.parseInt(literal.substring(index + 1, index + 5), 16));
                    index += 4;
                    break;
                default:
                    throw new IllegalArgumentException("unsupported JSON escape \\" + escaped);
            }
        }
        return builder.toString();
    }

    private static String object(String... fields) {
        StringBuilder builder = new StringBuilder();
        builder.append('{');
        boolean first = true;
        for (String field : fields) {
            if (field == null) {
                continue;
            }
            if (!first) {
                builder.append(',');
            }
            builder.append(field);
            first = false;
        }
        builder.append('}');
        return builder.toString();
    }

    private static String array(List<String> items) {
        StringBuilder builder = new StringBuilder();
        builder.append('[');
        for (int index = 0; index < items.size(); index++) {
            if (index > 0) {
                builder.append(',');
            }
            builder.append(items.get(index));
        }
        builder.append(']');
        return builder.toString();
    }

    private static String field(String name, String valueJson) {
        return quote(name) + ":" + valueJson;
    }

    private static String quote(String value) {
        if (value == null) {
            return "null";
        }
        StringBuilder builder = new StringBuilder(value.length() + 2);
        builder.append('"');
        for (int index = 0; index < value.length(); index++) {
            char current = value.charAt(index);
            switch (current) {
                case '"':
                    builder.append("\\\"");
                    break;
                case '\\':
                    builder.append("\\\\");
                    break;
                case '\b':
                    builder.append("\\b");
                    break;
                case '\f':
                    builder.append("\\f");
                    break;
                case '\n':
                    builder.append("\\n");
                    break;
                case '\r':
                    builder.append("\\r");
                    break;
                case '\t':
                    builder.append("\\t");
                    break;
                default:
                    if (current < 0x20) {
                        builder.append(String.format(Locale.ROOT, "\\u%04X", (int) current));
                    } else {
                        builder.append(current);
                    }
                    break;
            }
        }
        builder.append('"');
        return builder.toString();
    }

    private static String profileVersion(String profileId) {
        switch (profileId) {
            case "classic221":
                return "2.2.1";
            case "classic222":
                return "2.2.2";
            case "classic301":
                return "3.0.1";
            case "classic304":
                return "3.0.4";
            case "classic305":
                return "3.0.5";
            default:
                return "2.2.2";
        }
    }

    private static String profileAtr(String profileId) {
        switch (profileId) {
            case "classic21":
            case "classic211":
                return "3B80800121";
            case "classic221":
            case "classic222":
                return "3B80800122";
            case "classic301":
            case "classic304":
            case "classic305":
                return "3B8180013005";
            default:
                return "3B80800122";
        }
    }

    private static final class ProfileSpec {
        final String id;
        final String version;
        final String readerName;
        final String atrHex;
        final int persistentLimit;
        final int transientResetLimit;
        final int transientDeselectLimit;
        final int apduBufferLimit;
        final int commitBufferLimit;
        final int installScratchLimit;
        final int stackLimit;
        final int pageBytes;
        final int eraseBlockBytes;
        final int journalLimit;

        private ProfileSpec(
                String id,
                String version,
                String readerName,
                String atrHex,
                int persistentLimit,
                int transientResetLimit,
                int transientDeselectLimit,
                int apduBufferLimit,
                int commitBufferLimit,
                int installScratchLimit,
                int stackLimit,
                int pageBytes,
                int eraseBlockBytes,
                int journalLimit) {
            this.id = id;
            this.version = version;
            this.readerName = readerName;
            this.atrHex = atrHex;
            this.persistentLimit = persistentLimit;
            this.transientResetLimit = transientResetLimit;
            this.transientDeselectLimit = transientDeselectLimit;
            this.apduBufferLimit = apduBufferLimit;
            this.commitBufferLimit = commitBufferLimit;
            this.installScratchLimit = installScratchLimit;
            this.stackLimit = stackLimit;
            this.pageBytes = pageBytes;
            this.eraseBlockBytes = eraseBlockBytes;
            this.journalLimit = journalLimit;
        }

        static ProfileSpec forId(String id, String version, String readerName, String atrHex) {
            switch (id) {
                case "classic221":
                    return new ProfileSpec(
                            id, version, readerName, atrHex, 262144, 16384, 4096, 261, 4096, 24576, 12288, 256, 1024, 4096);
                case "classic222":
                    return new ProfileSpec(
                            id, version, readerName, atrHex, 262144, 16384, 4096, 261, 4096, 24576, 12288, 256, 1024, 4096);
                case "classic301":
                    return new ProfileSpec(
                            id, version, readerName, atrHex, 524288, 32768, 8192, 2048, 8192, 65536, 32768, 512, 2048, 8192);
                case "classic304":
                    return new ProfileSpec(
                            id, version, readerName, atrHex, 524288, 32768, 8192, 2048, 8192, 65536, 32768, 512, 2048, 8192);
                case "classic305":
                    return new ProfileSpec(
                            id, version, readerName, atrHex, 524288, 32768, 8192, 2048, 8192, 65536, 32768, 512, 2048, 8192);
                default:
                    throw new IllegalArgumentException("unsupported profile " + id);
            }
        }
    }

    private static final class AppletDescriptor {
        final String className;
        final String aid;

        private AppletDescriptor(String className, String aid) {
            this.className = className;
            this.aid = aid;
        }
    }

    private static final class RuntimeDescriptor {
        final String packageName;
        final String packageAid;
        final String packageVersion;
        final List<AppletDescriptor> applets;

        private RuntimeDescriptor(String packageName, String packageAid, String packageVersion, List<AppletDescriptor> applets) {
            this.packageName = packageName;
            this.packageAid = packageAid;
            this.packageVersion = packageVersion;
            this.applets = applets;
        }

        static RuntimeDescriptor load(Path metadataPath) throws IOException {
            Properties properties = new Properties();
            try (InputStream input = Files.newInputStream(metadataPath)) {
                properties.load(input);
            }
            List<AppletDescriptor> applets = new ArrayList<>();
            int appletCount = parseInt(properties.getProperty("applet.count"), 0);
            for (int index = 0; index < appletCount; index++) {
                String className = properties.getProperty("applet." + index + ".class");
                String aid = properties.getProperty("applet." + index + ".aid");
                if (className != null && !className.isEmpty() && aid != null && !aid.isEmpty()) {
                    applets.add(new AppletDescriptor(className, aid));
                }
            }
            if (applets.isEmpty()) {
                throw new IllegalArgumentException(
                        "simulator metadata did not declare any applets in " + metadataPath);
            }
            return new RuntimeDescriptor(
                    properties.getProperty("package.name", DEFAULT_PACKAGE_NAME),
                    properties.getProperty("package.aid", DEFAULT_PACKAGE_AID),
                    properties.getProperty("package.version", DEFAULT_VERSION),
                    applets);
        }

        private static int parseInt(String value, int fallback) {
            if (value == null || value.isEmpty()) {
                return fallback;
            }
            return Integer.parseInt(value);
        }
    }

    private static final class ChannelState {
        final int channelNumber;
        String selectedAid;

        private ChannelState(int channelNumber) {
            this.channelNumber = channelNumber;
        }
    }

    private static final class SecureMessagingTracker {
        boolean active;
        String protocol;
        Integer securityLevel;
        String sessionId;
        long commandCounter;

        void clear() {
            active = false;
            protocol = null;
            securityLevel = null;
            sessionId = null;
            commandCounter = 0;
        }
    }

    private static final class ManagedJavaSimulatorRuntime {
        private final String backendKind;
        private final ProfileSpec profile;
        private final RuntimeDescriptor descriptor;
        private final String capPathLabel;
        private final Map<Integer, ChannelState> openChannels;
        private final SecureMessagingTracker secureMessaging;

        private Simulator simulator;
        private boolean powerOn;
        private boolean installed;
        private String currentAtrHex;
        private String selectedAid;
        private String runtimeSelectedAid;
        private Integer lastStatusWord;

        @FunctionalInterface
        private interface QuietSupplier<T> {
            T run();
        }

        ManagedJavaSimulatorRuntime(
                String backendKind, ProfileSpec profile, RuntimeDescriptor descriptor, String capPathLabel) {
            this.backendKind = backendKind;
            this.profile = profile;
            this.descriptor = descriptor;
            this.capPathLabel = capPathLabel;
            this.openChannels = new LinkedHashMap<>();
            this.secureMessaging = new SecureMessagingTracker();
            this.currentAtrHex = profile.atrHex;
            this.selectedAid = null;
            this.runtimeSelectedAid = null;
        }

        void start() {
            try {
                withQuietStdout(
                        () -> {
                            System.setProperty(ATR_SYSTEM_PROPERTY, profile.atrHex);
                            this.simulator = new Simulator();
                            this.simulator.changeProtocol("T=1");
                            installDeclaredApplets();
                            this.installed = true;
                            this.powerOn = true;
                            this.currentAtrHex = profile.atrHex;
                            this.simulator.reset();
                            return null;
                        });
                resetTrackedSession();
            } catch (RuntimeException exception) {
                shutdown();
                throw new IllegalStateException(
                        "failed to start managed Java simulator: " + exception.getMessage(),
                        exception);
            }
        }

        private <T> T withQuietStdout(QuietSupplier<T> supplier) {
            PrintStream originalOut = System.out;
            try {
                // jcardsim and loaded applets can emit diagnostics on stdout, which would corrupt
                // the backend JSON control stream. Keep simulator chatter off stdout entirely.
                System.setOut(QUIET_STDOUT);
                return supplier.run();
            } finally {
                System.setOut(originalOut);
            }
        }

        String handleControlLine(String line) {
            String op = requireStringField(line, "op");
            switch (op) {
                case "handshake":
                    return handshakeReply();
                case "health":
                    return healthReply();
                case "get_session_state":
                    return sessionStateReply();
                case "transmit_typed":
                    return transmitReply(op, requireStringField(line, "raw_hex"));
                case "transmit_raw":
                    return transmitReply(op, requireStringField(line, "apdu_hex"));
                case "reset":
                    return resetReply();
                case "power":
                    return powerReply(requireStringField(line, "action"));
                case "manage_channel":
                    return manageChannelReply(requireBooleanField(line, "open"), optionalIntegerField(line, "channel_number"));
                case "open_secure_messaging":
                    return openSecureMessagingReply(
                            tryExtractStringField(line, "protocol"),
                            optionalIntegerField(line, "security_level"),
                            tryExtractStringField(line, "session_id"));
                case "advance_secure_messaging":
                    return advanceSecureMessagingReply(optionalLongField(line, "increment_by", 1L));
                case "close_secure_messaging":
                    return closeSecureMessagingReply();
                case "install":
                    throw new IllegalArgumentException(
                            "managed Java simulator is class-backed and does not accept CAP install commands");
                case "delete_package":
                    return deletePackageReply(requireStringField(line, "aid"));
                case "list_applets":
                    return listAppletsReply();
                case "list_packages":
                    return listPackagesReply();
                case "snapshot":
                    return snapshotReply();
                case "shutdown":
                    shutdown();
                    return object(field("op", quote("shutdown")), field("ok", "true"));
                default:
                    throw new IllegalArgumentException("unsupported operation " + op);
            }
        }

        String handshakeReply() {
            return object(
                    field("op", quote("handshake")),
                    field("ok", "true"),
                    field(
                            "handshake",
                            object(
                                    field("protocol_version", quote("1.0")),
                                    field("backend_kind", quote(backendKind)),
                                    field("reader_name", quote(profile.readerName)),
                                    field("backend_capabilities", backendCapabilitiesJson()))));
        }

        String healthReply() {
            return object(
                    field("op", quote("health")),
                    field("ok", "true"),
                    field(
                            "health",
                            object(
                                    field("backend_kind", quote(backendKind)),
                                    field("status", quote("ready")),
                                    field("message", quote(healthMessage())),
                                    field("protocol_version", quote("1.0")))));
        }

        String sessionStateReply() {
            return object(
                    field("op", quote("get_session_state")),
                    field("ok", "true"),
                    field("session_state", sessionStateJson()));
        }

        String snapshotReply() {
            return object(
                    field("op", quote("snapshot")),
                    field("ok", "true"),
                    field(
                            "snapshot",
                            object(
                                    field("backend_kind", quote(backendKind)),
                                    field("profile_id", quote(profile.id)),
                                    field("version", quote(profileVersionLabel())),
                                    field("backend_capabilities", backendCapabilitiesJson()),
                                    field("atr_hex", quote(currentAtrHex)),
                                    field("reader_name", quote(profile.readerName)),
                                    field("iso_capabilities", isoCapabilitiesJson()),
                                    field("power_on", powerOn ? "true" : "false"),
                                    field("selected_aid", selectedAid == null ? "null" : quote(selectedAid)),
                                    field("session_state", sessionStateJson()),
                                    field("memory_limits", memoryLimitsJson()),
                                    field("memory_status", memoryStatusJson()))));
        }

        String transmitReply(String op, String apduHex) {
            String responseHex = transmit(apduHex);
            return object(
                    field("op", quote(op)),
                    field("ok", "true"),
                    field(
                            "exchange",
                            object(
                                    field("response_hex", quote(responseHex)),
                                    field("session_state", sessionStateJson()))));
        }

        String resetReply() {
            String atrHex = reset();
            return object(
                    field("op", quote("reset")),
                    field("ok", "true"),
                    field(
                            "reset",
                            object(
                                    field("atr_hex", quote(atrHex)),
                                    field("session_state", sessionStateJson()))));
        }

        String powerReply(String action) {
            String atrHex = power(action);
            return object(
                    field("op", quote("power")),
                    field("ok", "true"),
                    field(
                            "power",
                            object(
                                    field("atr_hex", atrHex == null ? "null" : quote(atrHex)),
                                    field("session_state", sessionStateJson()))));
        }

        String manageChannelReply(boolean open, Integer channelNumber) {
            return transmitReply("manage_channel", encodeHex(manageChannelCommand(open, channelNumber)));
        }

        String openSecureMessagingReply(String protocol, Integer securityLevel, String sessionId) {
            if (!powerOn) {
                throw new IllegalStateException("simulator card is powered off");
            }
            if (secureMessaging.active) {
                throw new IllegalStateException("secure messaging is already active");
            }
            secureMessaging.active = true;
            secureMessaging.protocol = protocol;
            secureMessaging.securityLevel = securityLevel;
            secureMessaging.sessionId = sessionId;
            secureMessaging.commandCounter = 0;
            return secureMessagingReply("open_secure_messaging");
        }

        String advanceSecureMessagingReply(long incrementBy) {
            if (!secureMessaging.active) {
                throw new IllegalStateException("secure messaging is not active");
            }
            bumpSecureMessagingCounter(Math.max(1L, incrementBy));
            return secureMessagingReply("advance_secure_messaging");
        }

        String closeSecureMessagingReply() {
            if (!secureMessaging.active) {
                throw new IllegalStateException("secure messaging is not active");
            }
            secureMessaging.clear();
            return secureMessagingReply("close_secure_messaging");
        }

        String secureMessagingReply(String op) {
            return object(
                    field("op", quote(op)),
                    field("ok", "true"),
                    field(
                            "secure_messaging",
                            object(field("session_state", sessionStateJson()))));
        }

        String deletePackageReply(String aid) {
            return object(
                    field("op", quote("delete_package")),
                    field("ok", "true"),
                    field("deleted", deletePackage(aid) ? "true" : "false"));
        }

        String listAppletsReply() {
            List<String> applets = new ArrayList<>();
            if (installed) {
                for (AppletDescriptor applet : descriptor.applets) {
                    applets.add(
                            object(
                                    field("package_aid", quote(descriptor.packageAid)),
                                    field("applet_aid", quote(applet.aid)),
                                    field("instance_aid", quote(applet.aid)),
                                    field("selectable", "true"),
                                    field("package_name", quote(descriptor.packageName)),
                                    field("applet_name", quote(applet.className))));
                }
            }
            return object(
                    field("op", quote("list_applets")),
                    field("ok", "true"),
                    field("applets", array(applets)));
        }

        String listPackagesReply() {
            List<String> packages = new ArrayList<>();
            if (installed) {
                packages.add(
                        object(
                                field("package_aid", quote(descriptor.packageAid)),
                                field("package_name", quote(descriptor.packageName)),
                                field("version", quote(descriptor.packageVersion)),
                                field("applet_count", Integer.toString(descriptor.applets.size()))));
            }
            return object(
                    field("op", quote("list_packages")),
                    field("ok", "true"),
                    field("packages", array(packages)));
        }

        String transmit(String apduHex) {
            byte[] apdu = decodeHex(apduHex);
            byte[] response = exchange(apdu);
            updateTrackedState(apdu, response);
            return encodeHex(response);
        }

        String reset() {
            ensureSimulator();
            withQuietStdout(
                    () -> {
                        simulator.reset();
                        return null;
                    });
            powerOn = true;
            currentAtrHex = profile.atrHex;
            resetTrackedSession();
            return currentAtrHex;
        }

        String power(String requested) {
            ensureSimulator();
            String normalized = requested == null ? "" : requested.trim().toLowerCase(Locale.ROOT);
            if ("on".equals(normalized)) {
                withQuietStdout(
                        () -> {
                            simulator.reset();
                            return null;
                        });
                powerOn = true;
                currentAtrHex = profile.atrHex;
                resetTrackedSession();
                return currentAtrHex;
            }
            if (!"off".equals(normalized)) {
                throw new IllegalArgumentException("unsupported power action " + requested);
            }
            powerOn = false;
            clearTrackedSession();
            return null;
        }

        boolean deletePackage(String aid) {
            return installed && descriptor.packageAid.equalsIgnoreCase(aid) && false;
        }

        void shutdown() {
            simulator = null;
            powerOn = false;
            clearTrackedSession();
        }

        private void installDeclaredApplets() {
            for (AppletDescriptor applet : descriptor.applets) {
                AID aid = parseAid(applet.aid);
                try {
                    withQuietStdout(
                            () -> {
                                simulator.installApplet(
                                        aid, applet.className, new byte[0], (short) 0, (byte) 0);
                                return null;
                            });
                } catch (RuntimeException exception) {
                    throw new IllegalStateException(
                            "unable to install applet "
                                    + applet.className
                                    + " ("
                                    + applet.aid
                                    + "): "
                                    + exception.getMessage(),
                            exception);
                }
            }
        }

        private byte[] exchange(byte[] command) {
            ensureSimulator();
            if (!powerOn) {
                throw new IllegalStateException("simulator card is powered off");
            }
            if (command.length < 4) {
                throw new IllegalArgumentException("APDU is too short");
            }

            int channel = logicalChannelFromCla(command[0]);
            if (channel > maxLogicalChannelNumber()) {
                return statusWordResponse(0x6881);
            }
            if (!openChannels.containsKey(channel)) {
                return statusWordResponse(0x6881);
            }

            int ins = unsigned(command[1]);
            if (ins == 0x70) {
                return handleManageChannelApdu(command);
            }

            if (ins != 0xA4) {
                syncRuntimeSelection(channel);
            }
            byte[] normalized = normalizeChannelCla(command);
            return withQuietStdout(() -> simulator.transmitCommand(normalized));
        }

        private byte[] handleManageChannelApdu(byte[] command) {
            int p1 = unsigned(command[2]);
            int p2 = unsigned(command[3]);
            if (p1 == 0x00) {
                int requestedChannel = p2 == 0 ? allocateLogicalChannel() : p2;
                if (requestedChannel <= 0 || requestedChannel > maxLogicalChannelNumber()) {
                    return statusWordResponse(0x6881);
                }
                if (openChannels.containsKey(requestedChannel)) {
                    return statusWordResponse(0x6881);
                }
                openChannels.put(requestedChannel, new ChannelState(requestedChannel));
                return new byte[] {(byte) requestedChannel, (byte) 0x90, 0x00};
            }
            if (p1 == 0x80) {
                if (p2 == 0 || p2 > maxLogicalChannelNumber()) {
                    return statusWordResponse(0x6A86);
                }
                if (!openChannels.containsKey(p2)) {
                    return statusWordResponse(0x6881);
                }
                openChannels.remove(p2);
                syncSelectedAid();
                return statusWordResponse(0x9000);
            }
            return statusWordResponse(0x6A86);
        }

        private byte[] manageChannelCommand(boolean open, Integer channelNumber) {
            if (open) {
                return new byte[] {0x00, 0x70, 0x00, (byte) (channelNumber == null ? 0 : channelNumber), 0x01};
            }
            if (channelNumber == null) {
                throw new IllegalArgumentException("manage_channel close requires channel_number");
            }
            return new byte[] {0x00, 0x70, (byte) 0x80, (byte) (int) channelNumber};
        }

        private int allocateLogicalChannel() {
            for (int channel = 1; channel <= maxLogicalChannelNumber(); channel++) {
                if (!openChannels.containsKey(channel)) {
                    return channel;
                }
            }
            return -1;
        }

        private void syncRuntimeSelection(int channel) {
            String channelAid = selectedAidForChannel(channel);
            if (channelAid == null || Objects.equals(runtimeSelectedAid, channelAid)) {
                return;
            }
            byte[] response = withQuietStdout(() -> simulator.transmitCommand(AIDUtil.select(channelAid)));
            int status = statusWord(response);
            if (status != 0x9000) {
                throw new IllegalStateException(
                        "unable to restore channel "
                                + channel
                                + " selection "
                                + channelAid
                                + " ("
                                + String.format(Locale.ROOT, "%04X", status)
                                + ")");
            }
            runtimeSelectedAid = channelAid;
        }

        private byte[] normalizeChannelCla(byte[] command) {
            byte[] normalized = command.clone();
            normalized[0] = stripLogicalChannel(command[0]);
            return normalized;
        }

        private byte stripLogicalChannel(byte cla) {
            int value = unsigned(cla);
            if ((value & 0x40) != 0) {
                return (byte) (value & 0xB0);
            }
            return (byte) (value & 0xFC);
        }

        private void resetTrackedSession() {
            clearTrackedSession();
            if (powerOn) {
                openChannels.put(0, new ChannelState(0));
            }
        }

        private void clearTrackedSession() {
            openChannels.clear();
            secureMessaging.clear();
            selectedAid = null;
            runtimeSelectedAid = null;
            lastStatusWord = null;
        }

        private void updateTrackedState(byte[] apdu, byte[] response) {
            lastStatusWord = statusWord(response);
            int channel = logicalChannelFromCla(apdu[0]);
            if (lastStatusWord == 0x9000) {
                int ins = unsigned(apdu[1]);
                int p1 = unsigned(apdu[2]);
                int p2 = unsigned(apdu[3]);
                if (ins == 0xA4 && apdu.length >= 5) {
                    int lc = unsigned(apdu[4]);
                    if (p1 == 0x04 && apdu.length >= 5 + lc) {
                        String aid = encodeHex(slice(apdu, 5, lc));
                        ensureChannel(channel).selectedAid = aid;
                        runtimeSelectedAid = aid;
                        syncSelectedAid();
                    }
                } else if (ins == 0x70) {
                    if (p1 == 0x00) {
                        int openedChannel =
                                response.length >= 3 ? unsigned(response[0]) : (p2 == 0 ? 1 : p2);
                        ensureChannel(openedChannel);
                    } else if (p1 == 0x80) {
                        openChannels.remove(p2);
                        syncSelectedAid();
                    }
                } else if (ins == 0x82 && (unsigned(apdu[0]) & 0x80) == 0) {
                    secureMessaging.active = p1 != 0 || p2 != 0;
                    secureMessaging.protocol = "iso7816";
                    secureMessaging.securityLevel = p1;
                }
            }
            if (secureMessaging.active) {
                bumpSecureMessagingCounter(1L);
            }
        }

        private ChannelState ensureChannel(int channelNumber) {
            ChannelState state = openChannels.get(channelNumber);
            if (state == null) {
                state = new ChannelState(channelNumber);
                openChannels.put(channelNumber, state);
            }
            return state;
        }

        private String selectedAidForChannel(int channelNumber) {
            ChannelState state = openChannels.get(channelNumber);
            return state == null ? null : state.selectedAid;
        }

        private void syncSelectedAid() {
            ChannelState basic = openChannels.get(0);
            selectedAid = basic == null ? null : basic.selectedAid;
        }

        private void bumpSecureMessagingCounter(long incrementBy) {
            secureMessaging.commandCounter = Math.min(
                    0xFFFF_FFFFL,
                    secureMessaging.commandCounter + Math.max(1L, incrementBy));
        }

        private int logicalChannelFromCla(byte cla) {
            int value = unsigned(cla);
            if ((value & 0x40) != 0) {
                return 4 + (value & 0x0F);
            }
            return value & 0x03;
        }

        private int maxLogicalChannelNumber() {
            return 3;
        }

        private String backendCapabilitiesJson() {
            return object(
                    field("protocol_version", quote("1.0")),
                    field("iso_capabilities", isoCapabilitiesJson()),
                    field("supports_typed_apdu", "true"),
                    field("supports_raw_apdu", "true"),
                    field("supports_apdu", "true"),
                    field("supports_reset", "true"),
                    field("supports_power_control", "true"),
                    field("supports_get_session_state", "true"),
                    field("supports_manage_channel", "true"),
                    field("supports_secure_messaging", "true"),
                    field("supports_snapshot", "true"),
                    field("supports_install", "false"),
                    field("supports_delete", "false"),
                    field("supports_backend_health", "true"),
                    field("executes_real_methods", "true"),
                    field("wire_compatible_scp02", "false"),
                    field("wire_compatible_scp03", "false"),
                    field(
                            "supported_profiles",
                            array(
                                    List.of(
                                            quote("classic221"),
                                            quote("classic222"),
                                            quote("classic301"),
                                            quote("classic304"),
                                            quote("classic305")))));
        }

        private String isoCapabilitiesJson() {
            return object(
                    field("protocols", array(List.of(quote("T1")))),
                    field("extended_length", extendedLengthSupported() ? "true" : "false"),
                    field("logical_channels", "true"),
                    field("max_logical_channels", "4"),
                    field("secure_messaging", "true"),
                    field("file_model_visibility", "false"),
                    field("raw_apdu", "true"));
        }

        private String sessionStateJson() {
            List<ChannelState> channelStates = new ArrayList<>(openChannels.values());
            channelStates.sort(Comparator.comparingInt(state -> state.channelNumber));
            List<String> channels = new ArrayList<>();
            for (ChannelState state : channelStates) {
                channels.add(
                        object(
                                field("channel_number", Integer.toString(state.channelNumber)),
                                field("selected_aid", state.selectedAid == null ? "null" : quote(state.selectedAid)),
                                field("current_file", "null")));
            }
            return object(
                    field("power_state", quote(powerOn ? "on" : "off")),
                    field("atr_hex", powerOn ? quote(currentAtrHex) : "null"),
                    field("active_protocol", powerOn ? object(field("protocol", quote("T1"))) : "null"),
                    field("selected_aid", selectedAid == null ? "null" : quote(selectedAid)),
                    field("current_file", "null"),
                    field("open_channels", array(channels)),
                    field(
                            "secure_messaging",
                            object(
                                    field("active", secureMessaging.active ? "true" : "false"),
                                    field("protocol", secureMessaging.protocol == null ? "null" : quote(secureMessaging.protocol)),
                                    field(
                                            "security_level",
                                            secureMessaging.securityLevel == null
                                                    ? "null"
                                                    : Integer.toString(secureMessaging.securityLevel)),
                                    field("session_id", secureMessaging.sessionId == null ? "null" : quote(secureMessaging.sessionId)),
                                    field("command_counter", Long.toString(secureMessaging.commandCounter)))),
                    field("verified_references", array(List.of())),
                    field("retry_counters", array(List.of())),
                    field("last_status", lastStatusWord == null ? "null" : Integer.toString(lastStatusWord)));
        }

        private String memoryLimitsJson() {
            return object(
                    field("persistent_bytes", Integer.toString(profile.persistentLimit)),
                    field("transient_reset_bytes", Integer.toString(profile.transientResetLimit)),
                    field("transient_deselect_bytes", Integer.toString(profile.transientDeselectLimit)),
                    field("apdu_buffer_bytes", Integer.toString(profile.apduBufferLimit)),
                    field("commit_buffer_bytes", Integer.toString(profile.commitBufferLimit)),
                    field("install_scratch_bytes", Integer.toString(profile.installScratchLimit)),
                    field("stack_bytes", Integer.toString(profile.stackLimit)),
                    field("page_bytes", Integer.toString(profile.pageBytes)),
                    field("erase_block_bytes", Integer.toString(profile.eraseBlockBytes)),
                    field("journal_bytes", Integer.toString(profile.journalLimit)),
                    field("wear_limit", "null"));
        }

        private String memoryStatusJson() {
            return object(
                    field("persistent_used", "0"),
                    field("transient_reset_used", "0"),
                    field("transient_deselect_used", "0"),
                    field("commit_buffer_used", "0"),
                    field("install_scratch_peak_bytes", "0"),
                    field("pages_touched", "0"),
                    field("erase_blocks_touched", "0"),
                    field("wear_count", "0"));
        }

        private String profileVersionLabel() {
            switch (profile.version) {
                case "2.2.1":
                    return "v2_2_1";
                case "2.2.2":
                    return "v2_2_2";
                case "3.0.1":
                    return "v3_0_1";
                case "3.0.4":
                    return "v3_0_4";
                case "3.0.5":
                    return "v3_0_5";
                default:
                    throw new IllegalStateException("unsupported profile version " + profile.version);
            }
        }

        private boolean extendedLengthSupported() {
            return profile.apduBufferLimit > 261;
        }

        private void ensureSimulator() {
            if (simulator == null) {
                throw new IllegalStateException("managed Java simulator is unavailable");
            }
        }

        private String healthMessage() {
            StringBuilder builder = new StringBuilder();
            builder.append("managed Java simulator ready with package ").append(descriptor.packageName);
            if (capPathLabel != null && !capPathLabel.isEmpty()) {
                builder.append(" (CAP artifact ").append(capPathLabel).append(')');
            }
            return builder.toString();
        }
    }

    private static AID parseAid(String aidHex) {
        try {
            return AIDUtil.create(aidHex);
        } catch (RuntimeException exception) {
            throw new IllegalArgumentException("invalid AID `" + aidHex + "`", exception);
        }
    }

    private static byte[] statusWordResponse(int statusWord) {
        return new byte[] {(byte) ((statusWord >> 8) & 0xFF), (byte) (statusWord & 0xFF)};
    }

    private static byte[] slice(byte[] source, int offset, int length) {
        byte[] target = new byte[length];
        System.arraycopy(source, offset, target, 0, length);
        return target;
    }

    private static int statusWord(byte[] response) {
        if (response.length < 2) {
            throw new IllegalStateException("APDU response is too short");
        }
        return (unsigned(response[response.length - 2]) << 8) | unsigned(response[response.length - 1]);
    }

    private static int unsigned(byte value) {
        return value & 0xFF;
    }

    private static String encodeHex(byte[] value) {
        if (value == null || value.length == 0) {
            return "";
        }
        StringBuilder builder = new StringBuilder(value.length * 2);
        for (byte item : value) {
            builder.append(String.format(Locale.ROOT, "%02X", unsigned(item)));
        }
        return builder.toString();
    }

    private static byte[] decodeHex(String value) {
        String normalized = value.replace(" ", "").trim();
        if ((normalized.length() & 1) != 0) {
            throw new IllegalArgumentException("hex string has an odd length");
        }
        byte[] bytes = new byte[normalized.length() / 2];
        for (int index = 0; index < normalized.length(); index += 2) {
            bytes[index / 2] = (byte) Integer.parseInt(normalized.substring(index, index + 2), 16);
        }
        return bytes;
    }
}
