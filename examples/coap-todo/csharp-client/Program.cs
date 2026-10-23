// todo-csharp: the C# todo client. Com.AugustCellars.CoAP speaks CoAP; Nagasena for
// .NET (OpenEXI's C# port) reads and writes SenML EXI.
//
//   todo-csharp [--host H] [--port N] list | add <title> | done <id> | remove <id>
//
// SenML EXI is the XML form of SenML (RFC 8428 section 7) coded as strict,
// schema-informed EXI, with the EXI options in the header and schemaId "a"
// (https://www.rfc-editor.org/rfc/rfc8428#section-8). The C# port of Nagasena has no
// schema compiler, so it reads the SenML schema compiled by Java Nagasena
// (senml.xsc, made by ../scripts/gen-schemas.sh).

using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security;
using System.Text;
using Com.AugustCellars.CoAP;
using Nagasena.Proc;
using Nagasena.Proc.Common;
using Nagasena.Proc.Grammars;
using Nagasena.Proc.IO;
using Nagasena.Sax;
using Nagasena.Schema;
using Org.System.Xml.Sax;

internal static class TodoClient {
    private const string SenmlNs = "urn:ietf:params:xml:ns:senml";
    private const string BaseName = "urn:example:todo:";
    private const int SenmlExi = 114; // CoAP Content-Format of application/senml-exi (RFC 8428 section 12.3)
    private const int TimeoutMs = 20000;

    private static GrammarCache LoadGrammars() {
        string xsc = Path.Combine(AppContext.BaseDirectory, "senml.xsc");
        using var s = File.OpenRead(xsc);
        return new GrammarCache(EXISchema.readIn(s), GrammarOptions.STRICT_OPTIONS);
    }

    // A one-record pack, urn:example:todo:<field> with one value attribute. In strict
    // EXI the attributes must come in the grammar's order, by local name
    // (https://www.w3.org/TR/exi/#complexTypeGrammars): bn, n, then vb or vs.
    private static byte[] FieldPack(GrammarCache grammars, string field, string valueAttr, string value) {
        string xml = $"<sensml xmlns=\"{SenmlNs}\"><senml bn=\"{BaseName}\" n=\"{field}\" "
            + $"{valueAttr}=\"{SecurityElement.Escape(value)}\"/></sensml>";
        var t = new Transmogrifier {
            AlignmentType = AlignmentType.bitPacked,
            OutputOptions = HeaderOptionsOutputType.all, // options and schemaId in the header
            OutputCookie = false,
        };
        t.setGrammarCache(grammars, new SchemaId("a"));
        var output = new MemoryStream();
        t.OutputStream = output;
        t.encode(new InputSource<Stream>(new MemoryStream(Encoding.UTF8.GetBytes(xml))));
        return output.ToArray();
    }

    // Reads the list pack. A record's full name is the base name in force plus its
    // own name (https://www.rfc-editor.org/rfc/rfc8428#section-4.5.1); items are
    // named <id>/title and <id>/done.
    private static SortedDictionary<int, (string Title, bool Done)> ReadList(GrammarCache grammars, byte[] exi) {
        var items = new SortedDictionary<int, (string Title, bool Done)>();
        var decoder = new EXIDecoder { GrammarCache = grammars, InputStream = new MemoryStream(exi) };
        // The header names the schema ("a"); with no resolver set, Nagasena uses the
        // grammars given here.
        Scanner scanner = decoder.processHeader();
        string baseName = "";
        var record = new Dictionary<string, string>();
        EventDescription ev;
        while ((ev = scanner.nextEvent()) != null) {
            switch (ev.EventKind) {
                case EventDescription_Fields.EVENT_SE when ev.URI == SenmlNs && ev.Name == "senml":
                    record.Clear();
                    break;
                case EventDescription_Fields.EVENT_AT:
                    record[ev.Name] = ev.Characters.makeString();
                    break;
                case EventDescription_Fields.EVENT_EE when record.Count > 0:
                    if (record.TryGetValue("bn", out var bn)) {
                        baseName = bn;
                    }
                    string name = baseName + record.GetValueOrDefault("n", "");
                    string? vs = record.GetValueOrDefault("vs");
                    string? vb = record.GetValueOrDefault("vb");
                    record.Clear();
                    if (!name.StartsWith(BaseName, StringComparison.Ordinal)) {
                        break;
                    }
                    string[] parts = name.Substring(BaseName.Length).Split('/', 2);
                    if (parts.Length != 2 || !int.TryParse(parts[0], out int id)) {
                        break; // the count record
                    }
                    var item = items.GetValueOrDefault(id, (Title: "", Done: false));
                    if (parts[1] == "title" && vs != null) {
                        item.Title = vs;
                    } else if (parts[1] == "done" && vb != null) {
                        item.Done = vb == "true" || vb == "1";
                    }
                    items[id] = item;
                    break;
            }
        }
        return items;
    }

    private static Response Send(Request request) {
        request.Send();
        return request.WaitForResponse(TimeoutMs) ?? throw new IOException("no answer from the server");
    }

    private static void Expect(Response r, StatusCode code) {
        if (r.StatusCode != code) {
            throw new IOException($"{r.StatusCode}: {r.ResponseText}");
        }
    }

    private static int Run(string host, string port, List<string> cmd) {
        var grammars = LoadGrammars();
        string uri = $"coap://{host}:{port}/todos";
        string op = cmd.Count > 0 ? cmd[0] : "";
        if (op == "list" && cmd.Count == 1) {
            var req = new Request(Method.GET) { URI = new Uri(uri), Accept = SenmlExi };
            var r = Send(req);
            Expect(r, StatusCode.Content);
            foreach (var (id, item) in ReadList(grammars, r.Payload)) {
                Console.WriteLine($"{id} [{(item.Done ? 'x' : ' ')}] {item.Title}");
            }
        } else if (op == "add" && cmd.Count == 2) {
            var req = new Request(Method.POST) { URI = new Uri(uri), Accept = SenmlExi };
            req.SetPayload(FieldPack(grammars, "title", "vs", cmd[1]), SenmlExi);
            var r = Send(req);
            Expect(r, StatusCode.Created);
            Console.WriteLine(r.LocationPaths.LastOrDefault() ?? "");
        } else if (op == "done" && cmd.Count == 2) {
            var req = new Request(Method.PUT) { URI = new Uri($"{uri}/{cmd[1]}"), Accept = SenmlExi };
            req.SetPayload(FieldPack(grammars, "done", "vb", "true"), SenmlExi);
            Expect(Send(req), StatusCode.Changed);
        } else if (op == "remove" && cmd.Count == 2) {
            var req = new Request(Method.DELETE) { URI = new Uri($"{uri}/{cmd[1]}") };
            Expect(Send(req), StatusCode.Deleted);
        } else {
            Console.Error.WriteLine("usage: todo-csharp [--host H] [--port N] list | add <title> | done <id> | remove <id>");
            return 2;
        }
        return 0;
    }

    private static int Main(string[] args) {
        string host = "127.0.0.1", port = "5683";
        var cmd = new List<string>();
        for (int i = 0; i < args.Length; i++) {
            if (args[i] == "--host" && i + 1 < args.Length) {
                host = args[++i];
            } else if (args[i] == "--port" && i + 1 < args.Length) {
                port = args[++i];
            } else {
                cmd.Add(args[i]);
            }
        }
        try {
            return Run(host, port, cmd);
        } catch (Exception e) {
            Console.Error.WriteLine($"todo-csharp: {e.Message}");
            return 1;
        }
    }
}
