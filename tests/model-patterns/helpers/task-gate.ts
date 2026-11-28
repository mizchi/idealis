export function taskGate() {
  let open!: () => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<void>((resolve, fail) => {
    open = resolve;
    reject = fail;
  });
  return { promise, open, reject };
}
