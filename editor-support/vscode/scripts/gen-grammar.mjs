import { execSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const workspaceRoot = resolve(__dirname, "../../..");
const binary = resolve(workspaceRoot, "target/debug/hnl");

if (!existsSync(binary)) {
  console.error(
    "hnl binary not found at " + binary + ". Run: cargo build -p handl-host-cli",
  );
  process.exit(1);
}

const outputPath = resolve(__dirname, "../grammars/hnl.tmLanguage.json");
execSync('"' + binary + '" gen-grammar "' + outputPath + '"', {
  stdio: "inherit",
});
console.log("Grammar written to " + outputPath);
