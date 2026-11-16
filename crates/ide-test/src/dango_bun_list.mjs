// Prints the test files `bun test` runs in the working folder, one a line, as bun's naming rule picks them.
import { readdirSync } from "node:fs";
import { join, relative } from "node:path";

const named = /(\.test|_test|\.spec|_spec)\.(js|jsx|ts|tsx|mjs|cjs|mts|cts)$/;

function walk(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.name.startsWith(".") || entry.name === "node_modules") continue;
    const path = join(dir, entry.name);
    if (entry.isDirectory()) walk(path);
    else if (named.test(entry.name)) console.log(relative(process.cwd(), path));
  }
}

walk(process.cwd());
