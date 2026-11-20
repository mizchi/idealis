import { mkdir } from "node:fs/promises";
import { dirname } from "node:path";

export const createRunDirectory = async (directory: string): Promise<void> => {
  await mkdir(dirname(directory), { recursive: true });
  await mkdir(directory);
};
