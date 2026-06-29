import { createHash } from "node:crypto";
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { dirname, resolve } from "node:path";

type PlatformBinary = {
  sourcePath: string;
  checksumPath: string;
};

const platformBinaries: PlatformBinary[] = [
  {
    sourcePath: "artifacts/verso-darwin-arm64/verso",
    checksumPath: "verso-darwin-arm64/verso",
  },
  {
    sourcePath: "artifacts/verso-darwin-x64/verso",
    checksumPath: "verso-darwin-x64/verso",
  },
  {
    sourcePath: "artifacts/verso-linux-arm64/verso",
    checksumPath: "verso-linux-arm64/verso",
  },
  {
    sourcePath: "artifacts/verso-linux-x64/verso",
    checksumPath: "verso-linux-x64/verso",
  },
  {
    sourcePath: "artifacts/verso-win32-x64/verso.exe",
    checksumPath: "verso-win32-x64/verso.exe",
  },
];

const outputPath = process.argv[2] ?? "artifacts/SHA256SUMS.txt";
const lines: string[] = [];

for (const platformBinary of platformBinaries) {
  const bytes = await readFile(platformBinary.sourcePath);
  const hash = createHash("sha256").update(bytes).digest("hex");
  lines.push(`${hash}  ${platformBinary.checksumPath}`);
}

await mkdir(dirname(resolve(outputPath)), { recursive: true });
await writeFile(outputPath, `${lines.join("\n")}\n`);

console.log(`Wrote platform binary checksums to ${outputPath}`);
