import {
  decodeOracleReply,
  type OracleClient,
} from "../workspace-oracle/protocol.js";
import { encodeBatch, encodeBuffer } from "./encode.js";
import type { Root } from "./generated/oracle.js";

export const witClient = (
  root: Root,
  mode: "wit-record" | "wit-buffer",
): OracleClient => ({
  request: (input) => {
    const batch = encodeBatch(input);
    return decodeOracleReply(
      mode === "wit-record"
        ? root.protocol.check(batch)
        : root.protocol.checkBuffer(encodeBuffer(batch)),
    );
  },
});
