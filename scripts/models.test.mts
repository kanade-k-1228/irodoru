import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { mkdir, mkdtemp, readFile, rm, stat, utimes, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { type TestContext, test } from "node:test";
import { fileURLToPath } from "node:url";
import { check, distribute, maxArchiveSize, prepare, publish, required, validRepository } from "./models.mts";

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

test("models use one reproducible ZIP and preserve their content revision across app releases", async (t) => {
  const { source, output } = await fixture(t);
  await writeFile(join(source, "anime/backbone.onnx.data"), Buffer.from([0, 1, 2, 255, 4, 128]));
  await prepare(source, output);
  const assets = join(source, "../assets");
  const first = await distribute(output, assets, "example/app-models");
  const bytes = await readFile(join(assets, first.archive.asset));
  assert.equal(first.schema, 2);
  assert.equal(first.archive.format, "zip");
  assert.equal(first.archive.asset, `${first.revision}-models.zip`);
  assert.equal(first.archive.size, bytes.length);
  assert.equal(first.archive.sha256, createHash("sha256").update(bytes).digest("hex"));
  assert.ok(first.files.every((file) => !("parts" in file)));
  assert.equal(
    first.revision,
    createHash("sha256")
      .update(await readFile(join(output, "bundle.json")))
      .digest("hex"),
  );
  await utimes(join(output, "anime/backbone.onnx"), 1800000000, 1800000000);
  await writeFile(join(assets, "stale.part001"), "old split model");
  const second = await distribute(output, assets, "example/app-models");
  assert.deepEqual(first, second);
  assert.deepEqual(await readFile(join(assets, second.archive.asset)), bytes);
  await assert.rejects(stat(join(assets, "stale.part001")), { code: "ENOENT" });
  const unpublished = await distribute(output, assets, null);
  assert.equal(unpublished.repository, null);
  assert.equal(unpublished.revision, first.revision);
  assert.deepEqual(unpublished.archive, first.archive);
  assert.deepEqual(JSON.parse(await readFile(join(assets, "model-distribution.json"), "utf8")), unpublished);
  await writeFile(join(source, "anime/backbone.onnx.data"), "changed");
  await prepare(source, output);
  const changed = await distribute(output, assets, null);
  assert.notEqual(changed.revision, first.revision);
  assert.notEqual(changed.archive.sha256, first.archive.sha256);
  await assert.rejects(stat(join(assets, first.archive.asset)), { code: "ENOENT" });
});

test("model archives reject overlapping paths and unsafe registry repositories", async (t) => {
  const { source, output } = await fixture(t);
  await prepare(source, output);
  const assets = join(source, "../assets");
  for (const destination of [output, join(output, "nested"), join(output, "..")])
    await assert.rejects(distribute(output, destination, null), /overlap/);
  for (const repository of [
    "",
    "Example/app",
    "ghcr.io/example/app",
    "example/app/extra",
    "example/../app",
    "example/app:tag",
    "example/app?query",
    "example/-app",
    "example/app.",
    `x/${"a".repeat(254)}`,
  ]) {
    assert.equal(validRepository(repository), false);
    await assert.rejects(distribute(output, assets, repository), /Invalid GHCR/);
  }
  await writeFile(join(source, "anime/backbone.onnx.data"), "");
  await prepare(source, output);
  await assert.rejects(distribute(output, assets, null), /Empty or invalid/);
  await rm(join(source, "anime/backbone.onnx.data"));
  await writeFile(join(source, "anime/backbone.onnx."), "invalid name");
  await prepare(source, output);
  await assert.rejects(distribute(output, assets, null), /Invalid distribution model path/);
});

test("Python archive helper verifies content, deterministic metadata and ZIP64", () => {
  execFileSync(
    process.platform === "win32" ? "python" : "python3",
    [fileURLToPath(new URL("./test_model_archive.py", import.meta.url))],
    { stdio: "inherit" },
  );
});

test("GHCR publishes a single ZIP layer, pins its digest, and checks anonymous access", async (t) => {
  const { source, output } = await fixture(t);
  await prepare(source, output);
  const assets = join(source, "../assets");
  const distribution = await distribute(output, assets, "example/app-models");
  const { archive } = distribution;
  const layer = {
    digest: `sha256:${archive.sha256}`,
    size: archive.size,
    mediaType: "application/zip",
    annotations: { "org.opencontainers.image.title": archive.asset },
  };
  const bytes = JSON.stringify({ schemaVersion: 2, layers: [layer] });
  const digest = `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
  const calls: string[][] = [];
  const run = (args: string[], cwd: string) => {
    assert.equal(cwd, assets);
    calls.push(args);
    if (args[0] === "push") {
      assert.equal(args[1], `ghcr.io/example/app-models:${archive.sha256}`);
      assert.ok(args.includes("application/vnd.irodoru.models.v2"));
      assert.ok(args.includes("org.opencontainers.image.created=1970-01-01T00:00:00Z"));
      assert.ok(args.includes("org.opencontainers.image.source=https://github.com/example/app"));
      assert.ok(args.includes(`org.opencontainers.image.revision=${distribution.revision}`));
      assert.deepEqual(
        args.filter((arg) => arg.endsWith(":application/zip")),
        [`${archive.asset}:application/zip`],
      );
      writeFileSync(args[args.indexOf("--export-manifest") + 1], bytes);
    } else {
      assert.equal(readFileSync(args[args.indexOf("--registry-config") + 1], "utf8"), '{"auths":{}}');
      if (args[0] === "manifest") {
        assert.equal(args[2], `ghcr.io/example/app-models@${digest}`);
        writeFileSync(args[args.indexOf("--output") + 1], bytes);
      } else {
        assert.equal(args[0], "blob");
        assert.equal(args[2], `ghcr.io/example/app-models@sha256:${archive.sha256}`);
        assert.ok(args.includes("--descriptor"));
        assert.ok(!args.includes("--output"));
      }
    }
  };
  assert.equal(await publish(assets, run), digest);
  assert.equal(calls.filter((args) => args[0] === "blob").length, 1);
  assert.equal(await publish(assets, run), digest);
  for (const denied of ["manifest", "blob"]) {
    await assert.rejects(
      publish(assets, (args, cwd) => {
        if (args[0] === denied) throw new Error("unauthorized");
        run(args, cwd);
      }),
      /private by default.*Public.*rerun/,
    );
  }
  await assert.rejects(
    publish(assets, (args, cwd) => {
      run(args, cwd);
      if (args[0] === "manifest") writeFileSync(args[args.indexOf("--output") + 1], "wrong");
    }),
    /Public OCI manifest digest mismatch/,
  );
  for (const layers of [
    [],
    [layer, layer],
    [{ ...layer, digest: `sha256:${"a".repeat(64)}` }],
    [{ ...layer, size: layer.size + 1 }],
    [{ ...layer, mediaType: "application/octet-stream" }],
  ]) {
    await assert.rejects(
      publish(assets, (args, cwd) => {
        run(args, cwd);
        if (args[0] === "push") writeFileSync(args[args.indexOf("--export-manifest") + 1], JSON.stringify({ layers }));
      }),
      /Published OCI layer/,
    );
  }
  const manifestPath = join(assets, "model-distribution.json");
  for (const size of [0, maxArchiveSize + 1, Number.MAX_SAFE_INTEGER + 1]) {
    await writeFile(manifestPath, JSON.stringify({ ...distribution, archive: { ...archive, size } }));
    await assert.rejects(
      publish(assets, () => assert.fail("must not publish invalid archive")),
      /Invalid model archive size/,
    );
  }
  for (const patch of [{ asset: "../escape.zip" }, { format: "tar" }]) {
    await writeFile(manifestPath, JSON.stringify({ ...distribution, archive: { ...archive, ...patch } }));
    await assert.rejects(
      publish(assets, () => assert.fail("must not publish invalid archive")),
      /Invalid model archive/,
    );
  }
  await writeFile(manifestPath, JSON.stringify(distribution));
  await writeFile(join(assets, archive.asset), "corrupted");
  await assert.rejects(
    publish(assets, () => assert.fail("must not publish corrupt content")),
    /checksum mismatch/,
  );
  await distribute(output, assets, null);
  await assert.rejects(
    publish(assets, () => assert.fail("must not publish an unpublished manifest")),
    /not ready/,
  );
});
