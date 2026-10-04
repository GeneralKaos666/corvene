import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

/**
 * When a source file last changed, from git, for `dateModified`. Build time
 * only; falls back to the build time when the file is uncommitted or git is
 * unavailable.
 */
export function lastModified(fileUrl: string | URL): Date {
  try {
    const file = fileURLToPath(fileUrl);
    const out = execFileSync("git", ["log", "-1", "--format=%cI", "--", file], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
    if (out) return new Date(out);
  } catch {
    // not a git checkout, or git missing
  }
  return new Date();
}
