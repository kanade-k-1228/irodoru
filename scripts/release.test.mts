import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { type TestContext, test } from "node:test";
import { containerInstructions, prepareAppAssets, setVersion } from "./release.mts";

async function fixture(t: TestContext) {
  const directory = await mkdtemp(join(tmpdir(), "irodoru-release-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  return directory;
}

test("release versions update app metadata without changing dependency versions", async (t) => {
  const directory = await fixture(t);
  const path = join(directory, "pkgs/app/src-tauri");
  await mkdir(path, { recursive: true });
  await writeFile(join(path, "tauri.conf.json"), '{"version":"0.1.0","productName":"irodoru"}');
  await writeFile(
    join(path, "Cargo.toml"),
    '[package]\nname = "irodoru-desktop"\nversion = "0.1.0"\n\n[dependencies.example]\nversion = "9"\n',
  );
  await setVersion("v1.2.3-rc.1", directory);
  assert.equal(JSON.parse(await readFile(join(path, "tauri.conf.json"), "utf8")).version, "1.2.3-rc.1");
  const manifest = await readFile(join(path, "Cargo.toml"), "utf8");
  assert.match(manifest, /version = "1.2.3-rc.1"/);
  assert.match(manifest, /version = "9"/);
  for (const invalid of ["main", "v1.2", "v1.2.3\n", 'v1.2.3"'])
    await assert.rejects(setVersion(invalid, directory), /Release tags/);
  assert.equal(await readFile(join(path, "Cargo.toml"), "utf8"), manifest);
});

test("installer assets retain contents and distinguish architectures without uploading", async (t) => {
  const directory = await fixture(t);
  const output = join(directory, "assets");
  await mkdir(output);
  for (const target of ["aarch64-apple-darwin", "x86_64-apple-darwin"]) {
    const bundle = join(directory, "target", target, "release/bundle/dmg");
    await mkdir(bundle, { recursive: true });
    await writeFile(join(bundle, "irodoru.dmg"), target);
    await writeFile(join(bundle, "debug.txt"), "ignore");
    const assets = await prepareAppAssets(directory, target, output);
    assert.deepEqual(
      assets.map((path) => basename(path)),
      [`irodoru-${target}.dmg`, `irodoru-${target}.dmg.sha256`],
    );
    assert.equal(await readFile(assets[0], "utf8"), target);
  }
  await assert.rejects(prepareAppAssets(directory, "../invalid", output), /Invalid build target/);
  await mkdir(join(directory, "target/empty/release/bundle"), { recursive: true });
  await assert.rejects(prepareAppAssets(directory, "empty", output), /No installers/);
});

test("container instructions include the immutable digest and independent server usage", () => {
  const digest = `sha256:${"a".repeat(64)}`;
  const text = containerInstructions("ghcr.io/Example/Irodoru", "1.2.3", digest);
  assert.ok(text.includes(`docker pull ghcr.io/example/irodoru@${digest}`));
  assert.ok(text.includes("docker run --rm -p 127.0.0.1:8080:8080 ghcr.io/example/irodoru:1.2.3"));
  assert.ok(!text.includes("Tauri server URL"));
  assert.throws(() => containerInstructions("ghcr.io/example/irodoru", "latest", "bad"), /container reference/);
});

test("large release assets round-trip through bounded parts with a checksum and join commands", async (t) => {
  const directory = await fixture(t);
  const output = join(directory, "assets");
  const target = "x86_64-pc-windows-msvc";
  const bundle = join(directory, "target", target, "release/bundle/portable");
  await mkdir(output);
  await mkdir(bundle, { recursive: true });
  const input = Buffer.from([0, 1, 2, 255, 4, 5, 6, 128, 8, 9]);
  await writeFile(join(bundle, "irodoru.zip"), input);
  const assets = await prepareAppAssets(directory, target, output, 4);
  const parts = assets.filter((path) => /\.part\d+$/.test(path));
  assert.equal(parts.length, 3);
  const chunks = await Promise.all(parts.map((path) => readFile(path)));
  assert.deepEqual(
    chunks.map((chunk) => chunk.length),
    [4, 4, 2],
  );
  assert.deepEqual(Buffer.concat(chunks), input);
  const name = `irodoru-${target}.zip`;
  assert.equal(
    await readFile(join(output, `${name}.sha256`), "utf8"),
    `${createHash("sha256").update(input).digest("hex")}  ${name}\n`,
  );
  const instructions = await readFile(join(output, `irodoru-${target}-README.txt`), "utf8");
  assert.ok(instructions.includes(`copy /b "${name}.part001"+"${name}.part002"+"${name}.part003" "${name}"`));
  assert.ok(instructions.includes(`cat '${name}.part001' '${name}.part002' '${name}.part003' > '${name}'`));
  assert.ok(!assets.some((path) => basename(path) === name));
});

test("release splitting handles empty files, exact boundaries, and a single byte over the limit", async (t) => {
  for (const size of [0, 4, 8, 9]) {
    const directory = await fixture(t);
    const output = join(directory, "assets");
    const bundle = join(directory, "target/linux/release/bundle");
    await mkdir(output);
    await mkdir(bundle, { recursive: true });
    const input = Buffer.alloc(size, 123);
    await writeFile(join(bundle, "app.deb"), input);
    const assets = await prepareAppAssets(directory, "linux", output, 4);
    const data = assets.filter((path) => !path.endsWith(".sha256") && !path.endsWith(".txt"));
    assert.equal(data.length, Math.max(1, Math.ceil(size / 4)));
    assert.deepEqual(Buffer.concat(await Promise.all(data.map((path) => readFile(path)))), input);
    for (const invalid of [0, -1, 0.5, 2 ** 31])
      await assert.rejects(prepareAppAssets(directory, "linux", output, invalid), /Invalid release part size/);
  }
});
