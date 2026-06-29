import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";

type PackageManifest = {
  name?: unknown;
  version?: unknown;
};

type PackageToPublish = {
  dir: string;
  name: string;
  version: string;
};

function fail(message: string): never {
  console.error(message);
  process.exit(1);
}

function run(
  command: string,
  args: string[],
  options: { stdio?: "inherit" | "pipe"; env?: NodeJS.ProcessEnv } = {},
): { stderr: string; stdout: string; status: number } {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    env: options.env,
    shell: process.platform === "win32",
    stdio: options.stdio === "inherit" ? "inherit" : "pipe",
  });

  if (result.error !== undefined) {
    fail(result.error.message);
  }

  return {
    stderr: typeof result.stderr === "string" ? result.stderr : "",
    stdout: typeof result.stdout === "string" ? result.stdout : "",
    status: result.status ?? 1,
  };
}

function readPackage(dir: string): PackageToPublish {
  const manifest = JSON.parse(readFileSync(join(dir, "package.json"), "utf8")) as PackageManifest;
  if (typeof manifest.name !== "string" || manifest.name.length === 0) {
    fail(`${dir}/package.json must declare a package name`);
  }

  if (typeof manifest.version !== "string" || manifest.version.length === 0) {
    fail(`${dir}/package.json must declare a package version`);
  }

  return {
    dir,
    name: manifest.name,
    version: manifest.version,
  };
}

function packageNotFound(result: { stderr: string; stdout: string; status: number }): boolean {
  if (result.status === 0) {
    return false;
  }

  const output = `${result.stdout}\n${result.stderr}`;
  return (
    output.includes("E404") ||
    output.includes("404 Not Found") ||
    output.includes("is not in this registry")
  );
}

function isPublished(pkg: PackageToPublish): boolean {
  const result = run("npm", [
    "view",
    `${pkg.name}@${pkg.version}`,
    "version",
    "--registry",
    "https://registry.npmjs.org",
  ]);

  if (result.status === 0) {
    const publishedVersion = result.stdout.trim();
    if (publishedVersion === pkg.version) {
      return true;
    }

    fail(`npm view returned ${publishedVersion} for ${pkg.name}@${pkg.version}`);
  }

  if (packageNotFound(result)) {
    return false;
  }

  fail(
    [
      `Could not determine whether ${pkg.name}@${pkg.version} is already published.`,
      result.stdout.trim(),
      result.stderr.trim(),
    ]
      .filter(Boolean)
      .join("\n"),
  );
}

function distTagForVersion(version: string): string {
  const prerelease = /^[0-9]+\.[0-9]+\.[0-9]+-([0-9A-Za-z-]+)(?:\.|$)/.exec(version)?.[1];
  if (prerelease === undefined) {
    return "latest";
  }

  if (["alpha", "beta", "rc"].includes(prerelease)) {
    return prerelease;
  }

  return "next";
}

function publishPackage(pkg: PackageToPublish): void {
  const distTag = distTagForVersion(pkg.version);
  const result = run(
    "pnpm",
    [
      "--dir",
      pkg.dir,
      "publish",
      "--access",
      "public",
      "--no-git-checks",
      "--provenance",
      "--tag",
      distTag,
    ],
    { stdio: "inherit" },
  );

  if (result.status !== 0) {
    fail(`Failed to publish ${pkg.name}@${pkg.version}`);
  }
}

const args = process.argv.slice(2);
const dryRun = args[0] === "--dry-run";
const packageDirs = dryRun ? args.slice(1) : args;

if (packageDirs.length === 0) {
  fail("Usage: node scripts/publish-npm-packages.mts [--dry-run] <package-dir>...");
}

for (const pkg of packageDirs.map(readPackage)) {
  const distTag = distTagForVersion(pkg.version);
  if (dryRun) {
    console.log(
      `[dry-run] would publish ${pkg.name}@${pkg.version} from ${pkg.dir} with dist-tag ${distTag}`,
    );
    continue;
  }

  if (isPublished(pkg)) {
    console.log(`Skipping ${pkg.name}@${pkg.version}; it is already published.`);
    continue;
  }

  console.log(`Publishing ${pkg.name}@${pkg.version} from ${pkg.dir} with dist-tag ${distTag}...`);
  publishPackage(pkg);
}
