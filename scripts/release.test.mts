import assert from "node:assert/strict";
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
      [`irodoru-${target}.dmg`],
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
