import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { copyFile, mkdir, readdir, readFile, realpath, rename, rm, stat, utimes, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { parseArgs } from "node:util";

const root = fileURLToPath(new URL("../", import.meta.url));
export const required = {
  anime: [
    "manifest.json",
    "tokenizer.json",
    "caption_tokenizer.json",
    "forced_splits.json",
    "backbone.onnx",
    "text_proj.onnx",
    "caption_proj.onnx",
    "speaker_encoder.onnx",
    "duration.onnx",
    "dit_context.onnx",
    "dit_step.onnx",
  ],
  codec: ["codec.json", "dacvae_encoder.onnx", "dacvae_decoder.onnx"],
};

async function fingerprint(path: string) {
  const hash = createHash("sha256");
  let size = 0;
  for await (const chunk of createReadStream(path)) {
    size += chunk.length;
    hash.update(chunk);
  }
  return { size, sha256: hash.digest("hex") };
}

async function filesIn(directory: string) {
  const entries = await readdir(directory, { recursive: true, withFileTypes: true });
  return entries
    .filter((entry) => entry.isFile())
    .map((entry) => relative(directory, join(entry.parentPath, entry.name)).split(sep).join("/"));
}

function contains(parent: string, child: string) {
  const path = relative(parent, child);
  return path === "" || (!isAbsolute(path) && path !== ".." && !path.startsWith(`..${sep}`));
}

export async function prepare(source: string, destination: string) {
  source = await realpath(source);
  destination = resolve(destination);
  for (const [directory, names] of Object.entries(required)) {
    for (const name of names) {
      const path = join(source, directory, name);
      const info = await stat(path).catch(() => null);
      if (!info?.isFile() || info.size === 0)
        throw new Error(`Missing model resource: ${path}. Run pnpm models:export first.`);
    }
  }
  const manifest = JSON.parse(await readFile(join(source, "anime/manifest.json"), "utf8"));
  if (manifest.name !== "anime") throw new Error("The app must bundle the anime model");
  if (contains(source, destination) || contains(destination, source))
    throw new Error("Model staging directories must not overlap");
  await mkdir(destination, { recursive: true });
  destination = await realpath(destination);
  if (contains(source, destination) || contains(destination, source))
    throw new Error("Model staging directories must not overlap");
  const paths = [];
  for (const [directory, names] of Object.entries(required)) {
    for (const name of (await readdir(join(source, directory))).sort()) {
      if (names.includes(name) || names.some((graph) => graph.endsWith(".onnx") && name.startsWith(`${graph}.`))) {
        paths.push(`${directory}/${name}`);
      }
    }
  }
  const expected = new Set([...paths, "bundle.json"]);
  for (const path of await filesIn(destination)) {
    if (!expected.has(path)) await rm(join(destination, path));
  }
  const files = [];
  for (const path of paths) {
    const input = join(source, path);
    const target = join(destination, path);
    const info = await stat(input);
    if (!info.isFile()) throw new Error(`Not a regular model file: ${input}`);
    const existing = await stat(target).catch((error) => {
      if (error.code === "ENOENT") return null;
      throw error;
    });
    if (!existing || existing.size !== info.size || Math.abs(existing.mtimeMs - info.mtimeMs) >= 0.001) {
      await mkdir(dirname(target), { recursive: true });
      const temporary = `${target}.tmp`;
      await copyFile(input, temporary);
      await utimes(temporary, info.atimeMs / 1000, info.mtimeMs / 1000);
      await rename(temporary, target);
    }
    files.push({ path, ...(await fingerprint(target)) });
  }
  const index = { model: "anime", checkpoint: manifest.checkpoint ?? null, files };
  const temporary = join(destination, "bundle.json.tmp");
  await writeFile(temporary, `${JSON.stringify(index, null, 2)}\n`);
  await rename(temporary, join(destination, "bundle.json"));
  console.log(
    `Bundled anime + codec: ${files.reduce((size, entry) => size + entry.size, 0)} bytes (${files.length} files)`,
  );
}

export async function check(destination: string) {
  const index: { model: string; files: { path: string; size: number; sha256: string }[] } = JSON.parse(
    await readFile(join(destination, "bundle.json"), "utf8"),
  );
  if (index.model !== "anime") throw new Error("The app must bundle anime");
  const paths = new Set(index.files.map((entry) => entry.path));
  const expected = Object.entries(required).flatMap(([directory, names]) =>
    names.map((name) => `${directory}/${name}`),
  );
  if (!expected.every((path) => paths.has(path)) || paths.size !== index.files.length)
    throw new Error("Incomplete or duplicate model resources");
  for (const entry of index.files) {
    const parts = entry.path.split("/");
    if (
      parts.length !== 2 ||
      !Object.hasOwn(required, parts[0]) ||
      !parts[1] ||
      parts[1] === "." ||
      parts[1] === ".." ||
      /[\\:]/.test(entry.path)
    ) {
      throw new Error("Invalid model resource path");
    }
    const actual = await fingerprint(join(destination, entry.path));
    if (actual.size !== entry.size || actual.sha256 !== entry.sha256)
      throw new Error(`Model checksum mismatch: ${entry.path}`);
  }
  const actual = await filesIn(destination);
  if (actual.length !== paths.size + 1 || actual.some((path) => path !== "bundle.json" && !paths.has(path))) {
    throw new Error("Unexpected files in app model resources; run pnpm models:prepare");
  }
  console.log("Anime model resources verified");
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const { values, positionals } = parseArgs({
    options: {
      source: { type: "string", default: join(root, "models") },
      destination: { type: "string", default: join(root, "pkgs/app/src-tauri/resources/models") },
    },
    allowPositionals: true,
  });
  if (positionals.length !== 1 || !["prepare", "check"].includes(positionals[0]))
    throw new Error("Usage: models.mts prepare|check [--source DIR] [--destination DIR]");
  if (positionals[0] === "prepare") await prepare(values.source, values.destination);
  else await check(values.destination);
}
