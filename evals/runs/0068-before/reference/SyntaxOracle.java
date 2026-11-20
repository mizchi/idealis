
import java.nio.file.Files;
import java.nio.file.Path;
import java.io.InputStream;
import tla2sany.parser.TLAplusParser;
import tla2sany.st.TreeNode;

public class SyntaxOracle {
  private static String quote(String value) {
    StringBuilder result = new StringBuilder("\"");
    for (char character : value.toCharArray()) {
      switch (character) {
        case '"': result.append("\\\""); break;
        case '\\': result.append("\\\\"); break;
        case '\n': result.append("\\n"); break;
        case '\r': result.append("\\r"); break;
        case '\t': result.append("\\t"); break;
        default:
          if (character < 32) result.append(String.format("\\u%04x", (int) character));
          else result.append(character);
      }
    }
    return result.append('"').toString();
  }
  private static void tree(TreeNode node, StringBuilder result) {
    result.append("{\"kind\":").append(node.getKind())
      .append(",\"image\":").append(quote(node.getImage()))
      .append(",\"children\":[");
    TreeNode[] children = node.heirs();
    for (int index = 0; index < children.length; index++) {
      if (index > 0) result.append(',');
      tree(children[index], result);
    }
    result.append("]}");
  }
  public static void main(String[] args) throws Exception {
    try (InputStream source = Files.newInputStream(Path.of(args[0]))) {
      TLAplusParser parser = new TLAplusParser(source, "UTF-8");
      boolean accepted = parser.parse();
      StringBuilder result = new StringBuilder("{\"kind\":\"")
        .append(accepted ? "accepted" : "syntax").append("\"");
      if (accepted) { result.append(",\"tree\":"); tree(parser.rootNode(), result); }
      Files.writeString(Path.of(args[1]), result.append('}').toString());
    }
  }
}
