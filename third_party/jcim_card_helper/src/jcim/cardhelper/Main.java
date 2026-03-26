package jcim.cardhelper;

import apdu4j.core.APDUBIBO;
import apdu4j.pcsc.CardBIBO;
import java.util.ArrayList;
import java.util.EnumSet;
import java.util.List;
import javax.smartcardio.ATR;
import javax.smartcardio.Card;
import javax.smartcardio.CardChannel;
import javax.smartcardio.CardTerminal;
import javax.smartcardio.CardTerminals;
import javax.smartcardio.CommandAPDU;
import javax.smartcardio.ResponseAPDU;
import javax.smartcardio.TerminalFactory;
import pro.javacard.capfile.AID;
import pro.javacard.gp.GPSecureChannelVersion;
import pro.javacard.gp.GPSession;
import pro.javacard.gp.keys.PlaintextKeys;

public final class Main {
    private static final String ISD_AID = "A000000003000000";

    private Main() {}

    public static void main(String[] args) throws Exception {
        if (args.length == 0) {
            throw new IllegalArgumentException("missing helper action");
        }

        String action = args[0];
        String readerName = null;
        String apduHex = null;
        String gpSecurityLevel = null;
        for (int index = 1; index < args.length; index++) {
            if ("--reader".equals(args[index]) && index + 1 < args.length) {
                readerName = args[++index];
            } else if ("--hex".equals(args[index]) && index + 1 < args.length) {
                apduHex = args[++index];
            } else if ("--security-level".equals(args[index]) && index + 1 < args.length) {
                gpSecurityLevel = args[++index];
            }
        }

        TerminalFactory factory = TerminalFactory.getDefault();
        CardTerminals terminals = factory.terminals();
        List<CardTerminal> readers = terminals.list();

        switch (action) {
            case "readers":
                for (CardTerminal reader : readers) {
                    System.out.println(
                            reader.getName() + "\tpresent=" + (reader.isCardPresent() ? "1" : "0"));
                }
                return;
            case "status":
                printStatus(selectReader(readers, readerName));
                return;
            case "apdu":
                if (apduHex == null || apduHex.isEmpty()) {
                    throw new IllegalArgumentException("missing --hex APDU payload");
                }
                sendApdu(selectReader(readers, readerName), apduHex);
                return;
            case "reset":
                resetCard(selectReader(readers, readerName));
                return;
            case "gp-auth-open":
                openGpSecureChannel(
                        selectReader(readers, readerName), parseSecurityLevel(gpSecurityLevel));
                return;
            case "gp-secure-apdu":
                if (apduHex == null || apduHex.isEmpty()) {
                    throw new IllegalArgumentException("missing --hex APDU payload");
                }
                sendSecureGpApdu(
                        selectReader(readers, readerName),
                        parseSecurityLevel(gpSecurityLevel),
                        apduHex);
                return;
            default:
                throw new IllegalArgumentException("unsupported helper action " + action);
        }
    }

    private static CardTerminal selectReader(List<CardTerminal> readers, String requestedName) throws Exception {
        if (readers.isEmpty()) {
            throw new IllegalStateException("no PC/SC readers are available");
        }
        if (requestedName == null || requestedName.isEmpty()) {
            return readers.get(0);
        }
        for (CardTerminal reader : readers) {
            if (reader.getName().equals(requestedName)) {
                return reader;
            }
        }
        throw new IllegalStateException("reader not found: " + requestedName);
    }

    private static void printStatus(CardTerminal reader) throws Exception {
        System.out.println("Reader: " + reader.getName());
        System.out.println("Card present: " + (reader.isCardPresent() ? "yes" : "no"));
        if (!reader.isCardPresent()) {
            return;
        }
        Card card = reader.connect("*");
        try {
            ATR atr = card.getATR();
            System.out.println("Protocol: " + card.getProtocol());
            System.out.println("ATR: " + encodeHex(atr.getBytes()));
        } finally {
            card.disconnect(false);
        }
    }

    private static void sendApdu(CardTerminal reader, String apduHex) throws Exception {
        Card card = reader.connect("*");
        try {
            CardChannel channel = card.getBasicChannel();
            ResponseAPDU response = channel.transmit(new CommandAPDU(decodeHex(apduHex)));
            System.out.println(encodeHex(response.getBytes()));
        } finally {
            card.disconnect(false);
        }
    }

    private static void resetCard(CardTerminal reader) throws Exception {
        Card card = reader.connect("*");
        try {
            card.disconnect(true);
        } finally {
            // reconnect below to report the post-reset ATR
        }
        Card resetCard = reader.connect("*");
        try {
            System.out.println(encodeHex(resetCard.getATR().getBytes()));
        } finally {
            resetCard.disconnect(false);
        }
    }

    private static void openGpSecureChannel(CardTerminal reader, int securityLevel) throws Exception {
        Card card = reader.connect("*");
        APDUBIBO bibo = new APDUBIBO(CardBIBO.wrap(card));
        try {
            GPSession session = GPSession.connect(bibo, new AID(ISD_AID));
            session.openSecureChannel(
                    resolveGpKeys(),
                    resolveScpVersion(),
                    buildHostChallenge(),
                    apduModes(securityLevel));
            System.out.println(session.getSecureChannel().toString());
        } finally {
            bibo.close();
            card.disconnect(false);
        }
    }

    private static void sendSecureGpApdu(CardTerminal reader, int securityLevel, String apduHex)
            throws Exception {
        Card card = reader.connect("*");
        APDUBIBO bibo = new APDUBIBO(CardBIBO.wrap(card));
        try {
            GPSession session = GPSession.connect(bibo, new AID(ISD_AID));
            session.openSecureChannel(
                    resolveGpKeys(),
                    resolveScpVersion(),
                    buildHostChallenge(),
                    apduModes(securityLevel));
            apdu4j.core.ResponseAPDU response =
                    session.transmit(new apdu4j.core.CommandAPDU(decodeHex(apduHex)));
            System.out.println(encodeHex(response.getBytes()));
        } finally {
            bibo.close();
            card.disconnect(false);
        }
    }

    private static PlaintextKeys resolveGpKeys() {
        String enc = requireEnv("JCIM_GP_ENC");
        String mac = requireEnv("JCIM_GP_MAC");
        String dek = requireEnv("JCIM_GP_DEK");
        return PlaintextKeys.fromKeys(decodeHex(enc), decodeHex(mac), decodeHex(dek));
    }

    private static GPSecureChannelVersion resolveScpVersion() {
        String mode = requireEnv("JCIM_GP_MODE").trim().toLowerCase();
        switch (mode) {
            case "scp02":
                return GPSecureChannelVersion.valueOf(2);
            case "scp03":
                return GPSecureChannelVersion.valueOf(3);
            default:
                throw new IllegalArgumentException("unsupported GP mode: " + mode);
        }
    }

    private static EnumSet<GPSession.APDUMode> apduModes(int securityLevel) {
        EnumSet<GPSession.APDUMode> modes = EnumSet.noneOf(GPSession.APDUMode.class);
        if ((securityLevel & 0x01) != 0) {
            modes.add(GPSession.APDUMode.MAC);
        }
        if ((securityLevel & 0x02) != 0) {
            modes.add(GPSession.APDUMode.ENC);
        }
        if ((securityLevel & 0x10) != 0) {
            modes.add(GPSession.APDUMode.RMAC);
        }
        if ((securityLevel & 0x20) != 0) {
            modes.add(GPSession.APDUMode.RENC);
        }
        if (modes.isEmpty()) {
            modes.add(GPSession.APDUMode.CLR);
        }
        return modes;
    }

    private static byte[] buildHostChallenge() {
        long nanos = System.nanoTime();
        long millis = System.currentTimeMillis();
        byte[] hostChallenge = new byte[8];
        for (int index = 0; index < hostChallenge.length; index++) {
            long source = ((index & 1) == 0) ? nanos : millis;
            hostChallenge[index] = (byte) (source >>> (index * 8));
        }
        return hostChallenge;
    }

    private static int parseSecurityLevel(String value) {
        if (value == null || value.isEmpty()) {
            return 0x01;
        }
        String normalized = value.trim().toLowerCase();
        if (normalized.startsWith("0x")) {
            return Integer.parseInt(normalized.substring(2), 16);
        }
        return Integer.parseInt(normalized);
    }

    private static String requireEnv(String name) {
        String value = System.getenv(name);
        if (value == null || value.isEmpty()) {
            throw new IllegalArgumentException("missing required environment variable " + name);
        }
        return value;
    }

    private static byte[] decodeHex(String value) {
        StringBuilder normalized = new StringBuilder(value.length());
        for (int index = 0; index < value.length(); index++) {
            char current = value.charAt(index);
            if (!Character.isWhitespace(current) && current != ':' && current != '-') {
                normalized.append(Character.toUpperCase(current));
            }
        }
        if ((normalized.length() & 1) != 0) {
            throw new IllegalArgumentException("hex value must contain an even number of characters");
        }
        byte[] bytes = new byte[normalized.length() / 2];
        for (int index = 0; index < bytes.length; index++) {
            int high = Character.digit(normalized.charAt(index * 2), 16);
            int low = Character.digit(normalized.charAt(index * 2 + 1), 16);
            if (high < 0 || low < 0) {
                throw new IllegalArgumentException("invalid hex value");
            }
            bytes[index] = (byte) ((high << 4) | low);
        }
        return bytes;
    }

    private static String encodeHex(byte[] bytes) {
        StringBuilder builder = new StringBuilder(bytes.length * 2);
        for (byte value : bytes) {
            builder.append(Character.forDigit((value >>> 4) & 0x0F, 16));
            builder.append(Character.forDigit(value & 0x0F, 16));
        }
        return builder.toString().toUpperCase();
    }
}
