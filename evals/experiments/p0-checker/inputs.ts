export const sharing = (depth: number, eager = false) => ({
  source: `---- MODULE Sharing ----
VARIABLE x
D0 == TRUE
${Array.from({ length: depth }, (_, index) => `D${index + 1} == ${eager ? "" : "IF TRUE THEN TRUE ELSE "}(D${index} /\\ D${index})`).join("\n")}
Init == x = 0
Next == UNCHANGED x
Safe == D${depth}
====`,
  config: "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
});
