package example.coaptodo;

import com.siemens.ct.exi.core.CodingMode;
import com.siemens.ct.exi.core.EXIFactory;
import com.siemens.ct.exi.core.EncodingOptions;
import com.siemens.ct.exi.core.FidelityOptions;
import com.siemens.ct.exi.core.grammars.Grammars;
import com.siemens.ct.exi.core.helpers.DefaultEXIFactory;
import com.siemens.ct.exi.grammars.GrammarFactory;
import com.siemens.ct.exi.main.api.sax.EXIResult;
import com.siemens.ct.exi.main.api.sax.EXISource;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import javax.xml.parsers.SAXParserFactory;
import org.eclipse.californium.core.CoapClient;
import org.eclipse.californium.core.CoapResponse;
import org.eclipse.californium.core.coap.CoAP.ResponseCode;
import org.eclipse.californium.elements.config.Configuration;
import org.xml.sax.Attributes;
import org.xml.sax.InputSource;
import org.xml.sax.XMLReader;
import org.xml.sax.helpers.DefaultHandler;

/**
 * The Java todo client: Eclipse Californium for CoAP, EXIficient for SenML EXI.
 *
 * <pre>
 *   java -jar todo-java.jar [--host H] [--port N] list | add TITLE | done ID | remove ID
 * </pre>
 *
 * <p>SenML EXI is the XML form of SenML (RFC 8428 section 7) coded as strict,
 * schema-informed EXI with the options in the header and schemaId "a"
 * (<a href="https://www.rfc-editor.org/rfc/rfc8428#section-8">RFC 8428 section 8</a>).
 */
public final class TodoClient {
    static final String SENML_NS = "urn:ietf:params:xml:ns:senml";
    static final String BASE_NAME = "urn:example:todo:";
    /** CoAP Content-Format of application/senml-exi (RFC 8428 section 12.3). */
    static final int SENML_EXI = 114;

    private final EXIFactory exi;
    private final String uri;

    TodoClient(String host, int port) throws Exception {
        this.exi = exiFactory();
        this.uri = "coap://" + host + ":" + port + "/todos";
    }

    /** The EXI settings of RFC 8428 section 8: strict, options in the header, schemaId "a", no cookie. */
    static EXIFactory exiFactory() throws Exception {
        Grammars grammars;
        try (InputStream xsd = TodoClient.class.getResourceAsStream("/senml.xsd")) {
            grammars = GrammarFactory.newInstance().createGrammars(xsd);
        }
        grammars.setSchemaId("a");
        EXIFactory f = DefaultEXIFactory.newInstance();
        f.setGrammars(grammars);
        // A stream that names its schema in the header asks for it by id; this
        // client knows only the SenML schema, so every id gets it.
        f.setSchemaIdResolver(id -> grammars);
        f.setFidelityOptions(FidelityOptions.createStrict());
        f.setCodingMode(CodingMode.BIT_PACKED);
        EncodingOptions enc = EncodingOptions.createDefault();
        enc.setOption(EncodingOptions.INCLUDE_OPTIONS);
        enc.setOption(EncodingOptions.INCLUDE_SCHEMA_ID);
        f.setEncodingOptions(enc);
        return f;
    }

    /** Encodes a one-record pack, urn:example:todo:FIELD with one value attribute. */
    byte[] fieldPack(String field, String valueAttr, String value) throws Exception {
        // In strict EXI the attributes must come in schema order (by name), which
        // a SAX encoder takes as given: bn, n, then the value (vb or vs).
        String xml = "<sensml xmlns=\"" + SENML_NS + "\"><senml bn=\"" + BASE_NAME + "\" n=\"" + field + "\" "
                + valueAttr + "=\"" + escape(value) + "\"/></sensml>";
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        EXIResult result = new EXIResult(exi);
        result.setOutputStream(out);
        SAXParserFactory spf = SAXParserFactory.newInstance();
        spf.setNamespaceAware(true);
        XMLReader reader = spf.newSAXParser().getXMLReader();
        reader.setContentHandler(result.getHandler());
        reader.parse(new InputSource(new ByteArrayInputStream(xml.getBytes(StandardCharsets.UTF_8))));
        return out.toByteArray();
    }

    static String escape(String s) {
        return s.replace("&", "&amp;").replace("<", "&lt;").replace("\"", "&quot;");
    }

    /** One item of the list. */
    static final class Item {
        String title = "";
        boolean done;
    }

    /**
     * Reads the list pack. A record's full name is the base name in force plus its
     * own name (RFC 8428 section 4.5.1); items are named ID/title and ID/done.
     */
    Map<Integer, Item> readList(byte[] payload) throws Exception {
        Map<Integer, Item> items = new TreeMap<>();
        EXISource source = new EXISource(exi);
        XMLReader reader = source.getXMLReader();
        reader.setContentHandler(new DefaultHandler() {
            String baseName = "";

            @Override
            public void startElement(String ns, String local, String qname, Attributes atts) {
                if (!SENML_NS.equals(ns) || !"senml".equals(local)) {
                    return;
                }
                String bn = atts.getValue("bn");
                if (bn != null) {
                    baseName = bn;
                }
                String n = atts.getValue("n");
                String name = baseName + (n == null ? "" : n);
                if (!name.startsWith(BASE_NAME)) {
                    return;
                }
                String[] parts = name.substring(BASE_NAME.length()).split("/", 2);
                if (parts.length != 2 || !parts[0].matches("[0-9]+")) {
                    return; // the count record
                }
                Item item = items.computeIfAbsent(Integer.parseInt(parts[0]), k -> new Item());
                if (parts[1].equals("title") && atts.getValue("vs") != null) {
                    item.title = atts.getValue("vs");
                } else if (parts[1].equals("done") && atts.getValue("vb") != null) {
                    String vb = atts.getValue("vb");
                    item.done = vb.equals("true") || vb.equals("1");
                }
            }
        });
        reader.parse(new InputSource(new ByteArrayInputStream(payload)));
        return items;
    }

    private static void expect(CoapResponse r, ResponseCode code) {
        if (r == null) {
            throw new IllegalStateException("no answer from the server");
        }
        if (r.getCode() != code) {
            throw new IllegalStateException(r.getCode().text + ": " + r.getResponseText());
        }
    }

    void run(List<String> cmd) throws Exception {
        String op = cmd.isEmpty() ? "" : cmd.get(0);
        if (op.equals("list") && cmd.size() == 1) {
            CoapResponse r = new CoapClient(uri).get(SENML_EXI);
            expect(r, ResponseCode.CONTENT);
            for (Map.Entry<Integer, Item> e : readList(r.getPayload()).entrySet()) {
                System.out.println(e.getKey() + " [" + (e.getValue().done ? "x" : " ") + "] " + e.getValue().title);
            }
        } else if (op.equals("add") && cmd.size() == 2) {
            CoapResponse r = new CoapClient(uri).post(fieldPack("title", "vs", cmd.get(1)), SENML_EXI);
            expect(r, ResponseCode.CREATED);
            List<String> location = r.getOptions().getLocationPath();
            System.out.println(location.isEmpty() ? "" : location.get(location.size() - 1));
        } else if (op.equals("done") && cmd.size() == 2) {
            CoapResponse r = new CoapClient(uri + "/" + cmd.get(1)).put(fieldPack("done", "vb", "true"), SENML_EXI);
            expect(r, ResponseCode.CHANGED);
        } else if (op.equals("remove") && cmd.size() == 2) {
            CoapResponse r = new CoapClient(uri + "/" + cmd.get(1)).delete();
            expect(r, ResponseCode.DELETED);
        } else {
            throw new IllegalArgumentException(
                    "usage: todo-java [--host H] [--port N] list | add <title> | done <id> | remove <id>");
        }
    }

    public static void main(String[] args) {
        String host = "127.0.0.1";
        int port = 5683;
        List<String> cmd = new ArrayList<>();
        for (int i = 0; i < args.length; i++) {
            switch (args[i]) {
                case "--host" -> host = args[++i];
                case "--port" -> port = Integer.parseInt(args[++i]);
                default -> cmd.add(args[i]);
            }
        }
        // Californium would otherwise write Californium3.properties into the
        // current directory; the built-in defaults are fine here.
        Configuration.setStandard(Configuration.createStandardWithoutFile());
        int status = 0;
        try {
            new TodoClient(host, port).run(cmd);
        } catch (Exception e) {
            System.err.println("todo-java: " + e.getMessage());
            status = 1;
        }
        // Californium's endpoint threads would keep the JVM alive.
        System.exit(status);
    }
}
