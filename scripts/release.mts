import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { createReadStream, createWriteStream } from "node:fs";
import { mkdtemp, readdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, extname, join, resolve } from "node:path";
import { pipeline } from "node:stream/promises";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));

export async function setVersion(tag: string, directory = root) {
  const version = tag?.replace(/^v/, "");
  if (!version || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version))
    throw new Error("Release tags must be vMAJOR.MINOR.PATCH (optional prerelease)");
  const configPath = join(directory, "pkgs/app/src-tauri/tauri.conf.json");
  const manifestPath = join(directory, "pkgs/app/src-tauri/Cargo.toml");
  const config = JSON.parse(await readFile(configPath, "utf8"));
  const manifest = await readFile(manifestPath, "utf8");
  if (!/^version = "[^"]+"/m.test(manifest)) throw new Error("Missing app package version");
  config.version = version;
  await writeFile(configPath, `${JSON.stringify(config, null, 2)}\n`);
  await writeFile(manifestPath, manifest.replace(/^version = "[^"]+"/m, `version = "${version}"`));
}

export async function prepareAppAssets(directory: string, target: string, destination: string, partSize = 1024 ** 3) {
  if (!/^[a-z0-9_-]+$/.test(target)) throw new Error("Invalid build target");
  if (!Number.isSafeInteger(partSize) || partSize < 1 || partSize >= 2 ** 31)
    throw new Error("Invalid release part size");
  const entries = await readdir(join(directory, "target", target, "release/bundle"), {
    recursive: true,
    withFileTypes: true,
  });
  const assets: string[] = [];
  const names = new Set<string>();
  const instructions = [
    "Download all parts for your platform into one folder. Join them before opening or installing the app.",
    "Windows: run the copy command in Command Prompt (cmd.exe). Extract the ZIP and run irodoru.exe.",
    "macOS/Linux: run the cat command in a terminal, then open or install the resulting file.",
    "Verify the joined file with SHA-256. Windows: Get-FileHash FILE -Algorithm SHA256 (PowerShell).",
    "macOS: shasum -a 256 -c FILE.sha256. Linux: sha256sum -c FILE.sha256.",
  ];
  let split = false;
  for (const entry of entries) {
    const extension = extname(entry.name);
    if (!entry.isFile() || ![".exe", ".dmg", ".deb", ".appimage", ".zip"].includes(extension.toLowerCase())) continue;
    const name = `${basename(entry.name, extension)}-${target}${extension}`;
    if (names.has(name)) throw new Error(`Duplicate installer name: ${entry.name}`);
    names.add(name);
    const source = join(entry.parentPath, entry.name);
    const { size } = await stat(source);
    const count = Math.max(1, Math.ceil(size / partSize));
    const parts: string[] = [];
    const hash = createHash("sha256");
    for (let part = 0; part < count; part++) {
      const filename = count === 1 ? name : `${name}.part${String(part + 1).padStart(3, "0")}`;
      const output = join(destination, filename);
      await pipeline(
        createReadStream(
          source,
          size ? { start: part * partSize, end: Math.min(size, (part + 1) * partSize) - 1 } : {},
        ),
        async function* (chunks) {
          for await (const chunk of chunks) {
            hash.update(chunk);
            yield chunk;
          }
        },
        createWriteStream(output),
      );
      assets.push(output);
      parts.push(filename);
    }
    const checksum = join(destination, `${name}.sha256`);
    await writeFile(checksum, `${hash.digest("hex")}  ${name}\n`);
    assets.push(checksum);
    if (count > 1) {
      split = true;
      instructions.push(
        "",
        name,
        `Windows: copy /b ${parts.map((part) => `"${part}"`).join("+")} "${name}"`,
        `macOS/Linux: cat ${parts.map((part) => `'${part}'`).join(" ")} > '${name}'`,
      );
    }
  }
  if (!names.size) throw new Error("No installers found");
  if (split) {
    const path = join(destination, `irodoru-${target}-README.txt`);
    await writeFile(path, `${instructions.join("\n")}\n`);
    assets.push(path);
  }
  return assets.sort();
}

export function containerInstructions(image: string, version: string, digest: string) {
  if (!image || !version || !/^sha256:[a-f0-9]{64}$/.test(digest))
    throw new Error("Missing or invalid container reference");
  image = image.toLowerCase();
  const reference = `${image}:${version}`;
  return `irodoru synthesis server (linux/amd64, linux/arm64)\n\ndocker pull ${reference}\ndocker pull ${image}@${digest}\n\ndocker run --rm -p 127.0.0.1:8080:8080 ${reference}\n`;
}

async function upload(command: string | undefined, args: string[]) {
  const directory = await mkdtemp(join(tmpdir(), "irodoru-release-"));
  try {
    let tag: string;
    let assets: string[];
    if (command === "app" && args.length === 2) {
      tag = args[0];
      assets = await prepareAppAssets(root, args[1], directory);
    } else if (command === "container" && args.length === 0) {
      const { RELEASE_TAG, IMAGE, IMAGE_TAG } = process.env;
      if (!RELEASE_TAG || !IMAGE || !IMAGE_TAG) throw new Error("Set RELEASE_TAG, IMAGE and IMAGE_TAG");
      tag = RELEASE_TAG;
      const manifest = JSON.parse(
        execFileSync(
          "docker",
          ["buildx", "imagetools", "inspect", `${IMAGE.toLowerCase()}:${IMAGE_TAG}`, "--format", "{{json .Manifest}}"],
          { encoding: "utf8" },
        ),
      );
      const path = join(directory, "container-image.txt");
      await writeFile(path, containerInstructions(IMAGE, IMAGE_TAG, manifest.digest));
      assets = [path];
    } else {
      throw new Error("Usage: release.mts version TAG | app TAG TARGET | container");
    }
    execFileSync("gh", ["release", "upload", tag, ...assets, "--clobber"], { stdio: "inherit" });
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [command, ...args] = process.argv.slice(2);
  if (command === "version" && args.length === 1) await setVersion(args[0]);
  else await upload(command, args);
}
