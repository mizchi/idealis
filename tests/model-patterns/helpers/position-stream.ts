import assert from "node:assert/strict";

export function readPositionStream(body: ReadableStream<Uint8Array>) {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  return {
    async next(): Promise<number> {
      while (!buffer.includes("\n")) {
        const chunk = await reader.read();
        assert.equal(
          chunk.done,
          false,
          "stream ended before the next position",
        );
        buffer += decoder.decode(chunk.value, { stream: true });
      }
      const end = buffer.indexOf("\n");
      const { position } = JSON.parse(buffer.slice(0, end)) as {
        position: unknown;
      };
      buffer = buffer.slice(end + 1);
      assert.ok(typeof position === "number" && Number.isSafeInteger(position));
      return position;
    },
    async cancelAfterDisconnect(): Promise<void> {
      try {
        await reader.cancel();
      } catch (error) {
        if (!(error instanceof TypeError)) throw error;
      }
    },
  };
}
