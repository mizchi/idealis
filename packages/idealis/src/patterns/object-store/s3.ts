import type { ObjectStore } from "./adapter.js";

interface S3Key {
  readonly Bucket: string;
  readonly Key: string;
}

/** The subset of the AWS SDK v3 aggregated S3 client used by this adapter. */
export interface S3ObjectClient {
  putObject(input: S3Key & { readonly Body: string }): Promise<unknown>;
  deleteObject(input: S3Key): Promise<unknown>;
  getObject(input: S3Key): Promise<{
    readonly Body?: { transformToString(): Promise<string> } | undefined;
  }>;
  listObjectsV2(input: {
    readonly Bucket: string;
    readonly Prefix: string;
    readonly ContinuationToken?: string;
  }): Promise<{
    readonly Contents?:
      | readonly { readonly Key?: string | undefined }[]
      | undefined;
    readonly IsTruncated?: boolean | undefined;
    readonly NextContinuationToken?: string | undefined;
  }>;
}

export function s3ObjectStore(
  client: S3ObjectClient,
  bucket: string,
): ObjectStore {
  return {
    async put(key, body) {
      await client.putObject({ Bucket: bucket, Key: key, Body: body });
    },
    async remove(key) {
      await client.deleteObject({ Bucket: bucket, Key: key });
    },
    async get(key) {
      try {
        const result = await client.getObject({ Bucket: bucket, Key: key });
        if (result.Body === undefined)
          throw new Error("S3 GET returned no body");
        return await result.Body.transformToString();
      } catch (error) {
        if (error instanceof Error && error.name === "NoSuchKey") return null;
        throw error;
      }
    },
    async list(prefix) {
      const keys: string[] = [];
      const usedTokens = new Set<string>();
      let token: string | undefined;
      for (;;) {
        const page = await client.listObjectsV2({
          Bucket: bucket,
          Prefix: prefix,
          ...(token === undefined ? {} : { ContinuationToken: token }),
        });
        for (const object of page.Contents ?? []) {
          if (object.Key === undefined)
            throw new Error("S3 LIST returned an object without a key");
          keys.push(object.Key);
        }
        if (page.IsTruncated !== true) return keys;
        token = page.NextContinuationToken;
        if (!token || usedTokens.has(token)) {
          throw new Error(
            "S3 LIST returned a missing or repeated continuation token",
          );
        }
        usedTokens.add(token);
      }
    },
  };
}
