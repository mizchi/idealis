import assert from "node:assert/strict";

export const createPhaseRecorder = () => {
  let buffer = "";
  const events: { code: number; milliseconds: number }[] = [];
  return {
    add: (chunk: string, milliseconds: number) => {
      buffer += chunk;
      const lines = buffer.split("\n");
      buffer = lines.pop() ?? "";
      for (const line of lines) {
        const code = /@!@!@STARTMSG (\d+):\d+ @!@!@/.exec(line)?.[1];
        if (code && [2189, 2190, 2191, 2193].includes(Number(code)))
          events.push({ code: Number(code), milliseconds });
      }
    },
    finish: () => {
      const start = events.filter((event) => event.code === 2189);
      const init = events.filter(
        (event) => event.code === 2190 || event.code === 2191,
      );
      const end = events.filter((event) => event.code === 2193);
      assert.equal(start.length, 1, "Missing or repeated init-start marker");
      assert.equal(init.length, 1, "Missing or repeated init-done marker");
      assert.equal(end.length, 1, "Missing or repeated success marker");
      const initStartMs = start[0]?.milliseconds;
      const initDoneMs = init[0]?.milliseconds;
      const successMs = end[0]?.milliseconds;
      assert.ok(
        initStartMs !== undefined &&
          initDoneMs !== undefined &&
          successMs !== undefined &&
          initStartMs <= initDoneMs &&
          initDoneMs <= successMs,
      );
      return {
        initStartMs,
        initDoneMs,
        successMs,
        initAndSearchMs: successMs - initStartMs,
        searchAndFinalMs: successMs - initDoneMs,
      };
    },
  };
};
