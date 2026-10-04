import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

/**
 * The workspace version from the repository's Cargo.toml, the fallback the
 * download links use before (or without) the Releases API answering. Build
 * time only.
 */
export function workspaceVersion(): string {
  try {
    const cargoToml = readFileSync(fileURLToPath(new URL("../../../Cargo.toml", import.meta.url)), "utf8");
    const workspace = cargoToml.split(/^\[workspace\.package\]/m)[1] ?? "";
    const match = workspace.match(/^version\s*=\s*"([^"]+)"/m);
    if (match) return match[1];
  } catch {
    // building outside the repository checkout
  }
  return "0.1.0";
}
