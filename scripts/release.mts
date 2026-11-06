import { execFileSync } from "node:child_process";
import { copyFile, mkdtemp, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, extname, join, resolve } from "node:path";
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

export async function prepareAppAssets(directory: string, target: string, destination: string) {
  if (!/^[a-z0-9_-]+$/.test(target)) throw new Error("Invalid build target");
  const entries = await readdir(join(directory, "target", target, "release/bundle"), {
    recursive: true,
    withFileTypes: true,
  });
  const assets: string[] = [];
  for (const entry of entries) {
    const extension = extname(entry.name);
    if (!entry.isFile() || ![".exe", ".dmg", ".deb", ".appimage"].includes(extension.toLowerCase())) continue;
    const path = join(destination, `${basename(entry.name, extension)}-${target}${extension}`);
    if (assets.includes(path)) throw new Error(`Duplicate installer name: ${entry.name}`);
    await copyFile(join(entry.parentPath, entry.name), path);
    assets.push(path);
  }
  if (!assets.length) throw new Error("No installers found");
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
