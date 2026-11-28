import assert from "node:assert/strict";
import test from "node:test";
import {
  objectStorePattern,
  objectStoreProjection,
} from "../../examples/model-patterns/library.js";
import { s3ObjectStore } from "../../examples/model-patterns/object-store/s3.js";
import {
  recordReplay,
  replayRecording,
} from "../../examples/model-patterns/replay.js";

const rcheck = "crates/rcheck/target/debug/rcheck";
const bucket = "pattern-sandbox";
const key = "isolated/test";

test("S3 observations traverse all pages and stop before touching an existing key", async () => {
  const tokens: (string | undefined)[] = [];
  const store = s3ObjectStore(
    {
      async putObject() {
        assert.fail("must not overwrite an existing key");
      },
      async deleteObject() {
        assert.fail("must not delete an existing key");
      },
      async getObject() {
        throw new DOMException("missing", "NoSuchKey");
      },
      async listObjectsV2(input) {
        assert.equal(input.Bucket, bucket);
        assert.equal(input.Prefix, key);
        tokens.push(input.ContinuationToken);
        return input.ContinuationToken === undefined
          ? {
              Contents: [{ Key: key + "-other" }],
              IsTruncated: true,
              NextContinuationToken: "page-2",
            }
          : { Contents: [{ Key: key }] };
      },
    },
    bucket,
  );
  const { scenario, adapter } = objectStorePattern(store, key);
  const result = await recordReplay(rcheck, scenario, adapter);
  assert.equal(result.verdict, "mismatch");
  assert.deepEqual(result.difference, {
    step: 0,
    command: null,
    expected: { body: null, listed: false },
    observed: { body: null, listed: true },
  });
  assert.deepEqual(tokens, [undefined, "page-2"]);
  const saved = JSON.parse(
    JSON.stringify(result.recording),
  ) as typeof result.recording;
  assert.deepEqual(
    await replayRecording(rcheck, saved, objectStoreProjection),
    {
      verdict: "mismatch",
      difference: result.difference,
    },
  );
  assert.deepEqual(tokens, [undefined, "page-2"]);
});

test("S3 commands use the configured bucket and compare independent GET and LIST results", async () => {
  let body: string | null = null;
  const writes: string[] = [];
  const store = s3ObjectStore(
    {
      async putObject(input) {
        assert.deepEqual(input, { Bucket: bucket, Key: key, Body: input.Body });
        writes.push(input.Body);
        body = input.Body;
      },
      async deleteObject(input) {
        assert.deepEqual(input, { Bucket: bucket, Key: key });
        writes.push("delete");
        body = null;
      },
      async getObject(input) {
        assert.deepEqual(input, { Bucket: bucket, Key: key });
        if (body === null) throw new DOMException("missing", "NoSuchKey");
        const value = body;
        return { Body: { transformToString: async () => value } };
      },
      async listObjectsV2(input) {
        assert.deepEqual(input, { Bucket: bucket, Prefix: key });
        return body === null ? {} : { Contents: [{ Key: key }] };
      },
    },
    bucket,
  );
  const { scenario, adapter } = objectStorePattern(store, key);
  const result = await recordReplay(rcheck, scenario, adapter);
  assert.equal(result.verdict, "matched");
  assert.deepEqual(writes, ["V1", "V2", "delete"]);
  assert.deepEqual(result.recording.observations, [
    { body: null, listed: false },
    { body: "V1", listed: true },
    { body: "V2", listed: true },
    { body: null, listed: false },
  ]);
});

test("S3 access failures and incomplete LIST responses stay errors", async () => {
  const denied = new DOMException("denied", "AccessDenied");
  const store = s3ObjectStore(
    {
      async putObject() {},
      async deleteObject() {},
      async getObject() {
        throw denied;
      },
      async listObjectsV2() {
        return { IsTruncated: true };
      },
    },
    bucket,
  );
  await assert.rejects(store.get(key), (error) => error === denied);
  await assert.rejects(
    store.list(key),
    /missing or repeated continuation token/,
  );
});
