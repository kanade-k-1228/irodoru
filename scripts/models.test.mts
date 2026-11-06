import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, stat, utimes, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { type TestContext, test } from "node:test";
import { check, prepare, required } from "./models.mts";

async function fixture(t: TestContext) {
  const directory = await mkdtemp(join(tmpdir(), "irodoru-models-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const source = join(directory, "source");
  const output = join(directory, "output");
  for (const [directory, names] of Object.entries(required)) {
    await mkdir(join(source, directory), { recursive: true });
    for (const name of names) await writeFile(join(source, directory, name), "test");
  }
  await writeFile(join(source, "anime/manifest.json"), JSON.stringify({ name: "anime", checkpoint: "example" }));
  return { source, output };
}

test("stages only anime, codec and sidecars; repeats without copying unchanged weights", async (t) => {
  const { source, output } = await fixture(t);
  await writeFile(join(source, "anime/backbone.portable.onnx"), "exclude");
  await writeFile(join(source, "anime/backbone.onnx.data"), "include");
  await mkdir(join(source, "mf"));
  await writeFile(join(source, "mf/manifest.json"), "exclude");
  await utimes(join(source, "anime/backbone.onnx"), 1700000000, 1700000000.123456);
  await prepare(source, output);
  await assert.rejects(stat(join(output, "mf")), { code: "ENOENT" });
  await assert.rejects(stat(join(output, "anime/backbone.portable.onnx")), { code: "ENOENT" });
  assert.equal(await readFile(join(output, "anime/backbone.onnx.data"), "utf8"), "include");
  await check(output);
  const index = JSON.parse(await readFile(join(output, "bundle.json"), "utf8"));
  assert.equal(index.files.length, 15);
  assert.equal(index.checkpoint, "example");
  const before = await stat(join(output, "anime/backbone.onnx"), { bigint: true });
  await writeFile(join(output, "stale.onnx"), "old");
  await assert.rejects(check(output), /Unexpected files/);
  await prepare(source, output);
  const after = await stat(join(output, "anime/backbone.onnx"), { bigint: true });
  assert.equal(after.ctimeNs, before.ctimeNs);
  await check(output);
  await writeFile(join(output, "anime/backbone.onnx"), "corrupt");
  await assert.rejects(check(output), /checksum mismatch/);
  await rm(join(source, "anime/backbone.onnx"));
  await assert.rejects(prepare(source, output), /Missing model resource/);
});

test("rejects invalid manifests, overlapping directories and malformed bundle indexes", async (t) => {
  const { source, output } = await fixture(t);
  await assert.rejects(prepare(source, source), /overlap/);
  await assert.rejects(prepare(source, join(source, "nested")), /overlap/);
  await assert.rejects(prepare(source, join(source, "..")), /overlap/);
  await writeFile(join(source, "anime/manifest.json"), '{"name":"mf"}');
  await assert.rejects(prepare(source, output), /anime/);
  await writeFile(join(source, "anime/manifest.json"), '{"name":"anime"}');
  await prepare(source, output);
  const path = join(output, "bundle.json");
  const index = JSON.parse(await readFile(path, "utf8"));
  for (const files of [index.files.slice(1), [...index.files, index.files[0]]]) {
    await writeFile(path, JSON.stringify({ ...index, files }));
    await assert.rejects(check(output), /Incomplete or duplicate/);
  }
  for (const pathValue of ["../secret", "/anime/test", "anime/../test", "anime/..\\test", "anime/C:test"]) {
    await writeFile(
      path,
      JSON.stringify({ ...index, files: [...index.files, { path: pathValue, size: 0, sha256: "" }] }),
    );
    await assert.rejects(check(output), /Invalid model resource path/);
  }
});
