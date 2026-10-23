// Compiles an XSD into Nagasena's binary schema form (.xsc) for the C# client.
// Nagasena for .NET can read compiled schemas but has no schema compiler, so the
// Java Nagasena compiles it (EXISchemaFactory) and writes it (EXISchema.writeOut);
// the C# side reads it back with EXISchema.readIn.
//
//   java -cp <nagasena jars> scripts/CompileXsc.java in.xsd out.xsc

import java.io.DataOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import org.openexi.schema.EXISchema;
import org.openexi.scomp.EXISchemaFactory;
import org.xml.sax.InputSource;

public class CompileXsc {
    public static void main(String[] args) throws Exception {
        EXISchema schema = new EXISchemaFactory().compile(new InputSource(new File(args[0]).toURI().toString()));
        try (DataOutputStream out = new DataOutputStream(new FileOutputStream(args[1]))) {
            schema.writeOut(out);
        }
    }
}
