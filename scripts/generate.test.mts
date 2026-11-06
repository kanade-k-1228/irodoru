import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { generate, type Run } from "./generate.mts";

test("generation and read-only checks handle both targets, missing files and stale content", async (t) => {
  const directory = await mkdtemp(join(tmpdir(), "irodoru-generate-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  await mkdir(join(directory, "pkgs/server"), { recursive: true });
  await mkdir(join(directory, "pkgs/app/src/logic/lib"), { recursive: true });
  const api = join(directory, "pkgs/server/openapi.json");
  const types = join(directory, "pkgs/app/src/logic/lib/desktop.d.ts");
  const calls: string[][] = [];
  const run: Run = (command, args, options) => {
    assert.equal(command, "cargo");
    assert.ok(args.includes("--locked"));
    assert.equal(options.cwd, directory);
    calls.push(args);
    return args.includes("openapi") ? '{"openapi":"3.1.0"}' : "export type Example = string;\n";
  };
  await assert.rejects(generate(["app-types"], { directory, run, check: true }), /stale/);
  await assert.rejects(readFile(types), { code: "ENOENT" });
  await generate([], { directory, run });
  assert.equal(await readFile(api, "utf8"), '{\n  "openapi": "3.1.0"\n}\n');
  await generate([], { directory, run, check: true });
  await writeFile(types, "stale types\n");
  await assert.rejects(generate([], { directory, run, check: true }), /stale/);
  assert.equal(await readFile(types, "utf8"), "stale types\n");
  const count = calls.length;
  await assert.rejects(generate(["api", "unknown"], { directory, run }), /Unknown generation target/);
  assert.equal(calls.length, count);
  await assert.rejects(
    generate(["app-types"], {
      directory,
      run: () => {
        throw new Error("cargo failed");
      },
    }),
    /cargo failed/,
  );
  assert.equal(await readFile(types, "utf8"), "stale types\n");
});
