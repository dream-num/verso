import { readdir, readFile, writeFile } from "node:fs/promises";
import { dirname, extname, join } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const distRoot = join(packageRoot, "dist");
const relativeImportPattern = /(from\s+["']|import\s*\(\s*["'])(\.{1,2}\/[^"']+)(["'])/g;

for (const fileName of await readdir(distRoot)) {
  if (extname(fileName) !== ".js") {
    continue;
  }

  const filePath = join(distRoot, fileName);
  const source = await readFile(filePath, "utf8");
  const rewritten = source.replace(
    relativeImportPattern,
    (_match, prefix: string, specifier: string, quote: string) => {
      if (extname(specifier) !== "") {
        return `${prefix}${specifier}${quote}`;
      }

      return `${prefix}${specifier}.js${quote}`;
    },
  );

  if (rewritten !== source) {
    await writeFile(filePath, rewritten);
  }
}
