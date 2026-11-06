import { type ExecFileSyncOptionsWithStringEncoding, execFileSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { parseArgs } from "node:util";

const root = fileURLToPath(new URL("../", import.meta.url));
export type Run = (command: string, args: string[], options: ExecFileSyncOptionsWithStringEncoding) => string;
type Target = { args: string[]; path: string; format: (output: string) => string };
const targets: Record<string, Target> = {
  api: {
    args: ["run", "--locked", "--quiet", "-p", "irodori-server", "--", "openapi"],
    path: "pkgs/server/openapi.json",
    format: (output) => `${JSON.stringify(JSON.parse(output), null, 2)}\n`,
  },
  "app-types": {
    args: ["run", "--locked", "--quiet", "-p", "irodoru-desktop", "--features", "typegen", "--bin", "desktop-types"],
    path: "pkgs/app/src/logic/lib/desktop.d.ts",
    format: (output) => output,
  },
};

export async function generate(
  names: string[],
  { check = false, directory = root, run = execFileSync }: { check?: boolean; directory?: string; run?: Run } = {},
) {
  if (!names.length) names = Object.keys(targets);
  for (const name of names) {
    if (!Object.hasOwn(targets, name)) throw new Error(`Unknown generation target: ${name}; choose api or app-types`);
  }
  for (const name of names) {
    const target = targets[name];
    const content = target.format(
      run("cargo", target.args, { cwd: directory, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 }),
    );
    const path = resolve(directory, target.path);
    if (check) {
      const existing = await readFile(path, "utf8").catch((error) => {
        if (error.code === "ENOENT") return null;
        throw error;
      });
      if (existing !== content) throw new Error(`${target.path} is stale; run pnpm generate ${name}`);
    } else {
      await writeFile(path, content);
    }
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const { values, positionals } = parseArgs({ options: { check: { type: "boolean" } }, allowPositionals: true });
  await generate(positionals, { check: values.check });
}
