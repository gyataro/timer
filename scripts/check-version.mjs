import { readFile } from "node:fs/promises";

const packageJson = JSON.parse(await readFile("package.json", "utf8"));
const tauriConfig = JSON.parse(await readFile("src-tauri/tauri.conf.json", "utf8"));
const cargoToml = await readFile("src-tauri/Cargo.toml", "utf8");
const cargoVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];

const versions = new Map([
  ["package.json", packageJson.version],
  ["src-tauri/Cargo.toml", cargoVersion],
  ["src-tauri/tauri.conf.json", tauriConfig.version],
]);

const expected = packageJson.version;
const mismatches = [...versions].filter(([, version]) => version !== expected);

if (mismatches.length > 0) {
  const details = [...versions]
    .map(([file, version]) => `  ${file}: ${version ?? "missing"}`)
    .join("\n");
  throw new Error(`Application versions do not match:\n${details}`);
}

const releaseTag = process.env.RELEASE_TAG;
if (releaseTag && releaseTag !== `v${expected}`) {
  throw new Error(`Release tag ${releaseTag} does not match application version v${expected}.`);
}

console.log(`Application version ${expected} is consistent.`);
