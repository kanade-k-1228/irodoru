import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import {
  copyFile,
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  realpath,
  rename,
  rm,
  stat,
  utimes,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
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
  if (manifest.name !== "anime") throw new Error("App model resources must use the anime model");
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
    `Staged anime + codec: ${files.reduce((size, entry) => size + entry.size, 0)} bytes (${files.length} files)`,
  );
}

export async function check(destination: string) {
  const index: { model: string; files: { path: string; size: number; sha256: string }[] } = JSON.parse(
    await readFile(join(destination, "bundle.json"), "utf8"),
  );
  if (index.model !== "anime") throw new Error("App model resources must use anime");
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

export function validRepository(repository: string) {
  return (
    repository.length <= 255 &&
    /^[a-z0-9](?:[a-z0-9._-]*[a-z0-9])?\/[a-z0-9](?:[a-z0-9._-]*[a-z0-9])?$/.test(repository)
  );
}

export const maxArchiveSize = 10_000_000_000;

export async function distribute(source: string, destination: string, repository: string | null) {
  if (repository !== null && !validRepository(repository))
    throw new Error("Invalid GHCR model repository; expected lowercase owner/package");
  source = await realpath(source);
  destination = resolve(destination);
  if (contains(source, destination) || contains(destination, source))
    throw new Error("Model distribution directories must not overlap");
  await mkdir(destination, { recursive: true });
  destination = await realpath(destination);
  if (contains(source, destination) || contains(destination, source))
    throw new Error("Model distribution directories must not overlap");
  await check(source);
  const indexBytes = await readFile(join(source, "bundle.json"));
  const revision = createHash("sha256").update(indexBytes).digest("hex");
  const index: { files: { path: string; size: number; sha256: string }[] } = JSON.parse(indexBytes.toString());
  for (const file of index.files) {
    if (!/^(anime|codec)\/[A-Za-z0-9_-](?:[A-Za-z0-9_.-]*[A-Za-z0-9_-])?$/.test(file.path))
      throw new Error("Invalid distribution model path");
    if (!Number.isSafeInteger(file.size) || file.size < 1) throw new Error("Empty or invalid model file");
  }
  const asset = `${revision}-models.zip`;
  const output = join(destination, asset);
  const temporaryArchive = `${output}.tmp`;
  let archive: { format: "zip"; asset: string; size: number; sha256: string };
  try {
    execFileSync(
      process.platform === "win32" ? "python" : "python3",
      [join(root, "scripts/model_archive.py"), source, temporaryArchive],
      { stdio: "inherit" },
    );
    archive = { format: "zip" as const, asset, ...(await fingerprint(temporaryArchive)) };
    if (archive.size > maxArchiveSize) throw new Error("Model archive exceeds GHCR's 10 GB layer limit");
    await rename(temporaryArchive, output);
  } finally {
    await rm(temporaryArchive, { force: true });
  }
  for (const path of await filesIn(destination)) {
    if (path !== asset && path !== "model-distribution.json") await rm(join(destination, path));
  }
  const manifest = {
    schema: 2,
    model: "anime",
    revision,
    repository,
    archive,
    files: index.files,
  };
  const temporary = join(destination, "model-distribution.json.tmp");
  await writeFile(temporary, `${JSON.stringify(manifest, null, 2)}\n`);
  await rename(temporary, join(destination, "model-distribution.json"));
  return manifest;
}

type Distribution = Awaited<ReturnType<typeof distribute>>;
type RunOras = (args: string[], directory: string) => void;

export async function publish(
  directory: string,
  run: RunOras = (args, cwd) => {
    execFileSync("oras", args, { cwd, stdio: "inherit" });
  },
) {
  directory = await realpath(directory);
  const distribution: Distribution = JSON.parse(await readFile(join(directory, "model-distribution.json"), "utf8"));
  const { repository, revision, archive } = distribution;
  if (
    distribution.schema !== 2 ||
    !repository ||
    !validRepository(repository) ||
    !/^[a-f0-9]{64}$/.test(revision) ||
    !distribution.files.length
  )
    throw new Error("Model distribution is not ready for GHCR publication");
  if (archive?.format !== "zip" || !/^[a-f0-9]{64}-models\.zip$/.test(archive.asset))
    throw new Error("Invalid model archive");
  if (
    !Number.isSafeInteger(archive.size) ||
    archive.size < 1 ||
    archive.size > maxArchiveSize ||
    !/^[a-f0-9]{64}$/.test(archive.sha256)
  )
    throw new Error("Invalid model archive size or digest (GHCR layer limit: 10 GB)");
  const actual = await fingerprint(join(directory, archive.asset));
  if (actual.size !== archive.size || actual.sha256 !== archive.sha256)
    throw new Error(`Model archive checksum mismatch: ${archive.asset}`);
  const temporary = await mkdtemp(join(tmpdir(), "irodoru-ghcr-"));
  try {
    const exported = join(temporary, "manifest.json");
    const reference = `ghcr.io/${repository}`;
    // Tag by archive digest so changed packaging retains older app downloads.
    // Fixed metadata keeps repeated publication of identical archives deterministic.
    run(
      [
        "push",
        `${reference}:${archive.sha256}`,
        "--artifact-type",
        "application/vnd.irodoru.models.v2",
        "--annotation",
        "org.opencontainers.image.created=1970-01-01T00:00:00Z",
        "--annotation",
        `org.opencontainers.image.source=https://github.com/${repository.replace(/-models$/, "")}`,
        "--annotation",
        `org.opencontainers.image.revision=${revision}`,
        "--export-manifest",
        exported,
        `${archive.asset}:application/zip`,
      ],
      directory,
    );
    const bytes = await readFile(exported);
    const manifest = JSON.parse(bytes.toString());
    const layer = manifest.layers?.[0];
    if (
      manifest.layers?.length !== 1 ||
      layer.digest !== `sha256:${archive.sha256}` ||
      layer.size !== archive.size ||
      layer.mediaType !== "application/zip" ||
      layer.annotations?.["org.opencontainers.image.title"] !== archive.asset
    )
      throw new Error("Published OCI layer does not match the app model archive");
    const digest = `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
    const anonymousConfig = join(temporary, "anonymous.json");
    const fetched = join(temporary, "public-manifest.json");
    await writeFile(anonymousConfig, '{"auths":{}}');
    try {
      run(
        ["manifest", "fetch", `${reference}@${digest}`, "--registry-config", anonymousConfig, "--output", fetched],
        directory,
      );
      // Descriptor-only fetch uses HEAD, without downloading the model weights again.
      run(
        [
          "blob",
          "fetch",
          `${reference}@sha256:${archive.sha256}`,
          "--descriptor",
          "--registry-config",
          anonymousConfig,
        ],
        directory,
      );
    } catch (cause) {
      throw new Error(
        `Cannot download ${reference} anonymously. New GHCR packages are private by default. Set the package visibility to Public in GitHub Package settings, then rerun this workflow.`,
        { cause },
      );
    }
    if (!bytes.equals(await readFile(fetched))) throw new Error("Public OCI manifest digest mismatch");
    console.log(`Public model artifact verified: ${reference}@${digest}`);
    return digest;
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const { values, positionals } = parseArgs({
    options: {
      source: { type: "string", default: join(root, "models") },
      destination: { type: "string", default: join(root, "pkgs/app/src-tauri/resources/models") },
      repository: { type: "string" },
    },
    allowPositionals: true,
  });
  if (positionals.length !== 1 || !["prepare", "check", "distribute", "publish"].includes(positionals[0]))
    throw new Error(
      "Usage: models.mts prepare|check|distribute|publish [--source DIR] [--destination DIR] [--repository OWNER/PACKAGE]",
    );
  if (positionals[0] === "prepare") await prepare(values.source, values.destination);
  else if (positionals[0] === "distribute")
    await distribute(values.source, values.destination, values.repository ?? null);
  else if (positionals[0] === "publish") await publish(values.destination);
  else await check(values.destination);
}
