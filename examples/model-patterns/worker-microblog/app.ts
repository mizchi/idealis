// Domain operations. The caller supplies storage and publication.
export interface MicroblogStore {
  setFollowing(
    reader: string,
    author: string,
    following: boolean,
  ): Promise<boolean>;
  publish(p: string, author: string): Promise<boolean>;
  author(p: string): Promise<string | null>;
  applyToTimeline(p: string, author: string): Promise<void>;
  visibleAuthor(reader: string, p: string): Promise<string | null>;
}

export function connectMicroblog(
  store: MicroblogStore,
  send: (post: string) => Promise<void>,
) {
  return {
    async follow(reader: string, author: string, following: boolean) {
      if (reader === author) return false;
      return store.setFollowing(reader, author, following);
    },
    publish: (p: string, author: string) => store.publish(p, author),
    async relay(p: string) {
      if ((await store.author(p)) === null) return false;
      await send(p);
      return true;
    },
    async applyPost(p: string) {
      const author = await store.author(p);
      if (author === null) throw new Error(`unknown post ${p}`);
      await store.applyToTimeline(p, author);
      return author;
    },
    async read(reader: string, p: string) {
      const author = await store.visibleAuthor(reader, p);
      if (author === null) return null;
      return { p, author };
    },
  };
}
