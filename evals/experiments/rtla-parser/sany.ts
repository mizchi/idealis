export const classifySany = (
  output: string,
  exitCode: number | null,
): "accepted" | "syntax" | "infrastructure" => {
  if (
    !output.includes("Parsing file") ||
    output.includes("Semantic processing")
  )
    return "infrastructure";
  if (
    /\*\*\*Parse Error\*\*\*|Lexical (?:\{error:|error)|lexical error|Token Manager Error/.test(
      output,
    )
  )
    return "syntax";
  return exitCode === 0 && !/Fatal errors|Exception/.test(output)
    ? "accepted"
    : "infrastructure";
};
