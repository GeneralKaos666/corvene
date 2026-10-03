export interface DocItem {
  slug: string;
  title: string;
  description: string;
  category: string;
}

export const DOC_ITEMS: DocItem[] = [
  {
    slug: "overview",
    title: "Overview",
    description: "What Corvene is, what it keeps from GitHub Desktop and what it replaces.",
    category: "Getting started",
  },
  {
    slug: "quickstart",
    title: "Quickstart",
    description: "Install Corvene, sign in, add your repositories and import them from GitHub Desktop.",
    category: "Getting started",
  },
  {
    slug: "installation",
    title: "Installation",
    description: "Requirements and packages for macOS, Windows, Linux and Android, and the Standard and Full editions.",
    category: "Getting started",
  },
  {
    slug: "architecture",
    title: "Architecture",
    description: "The crates, GPUI rendering, gitoxide reads, git CLI writes and the redb store.",
    category: "Internals",
  },
  {
    slug: "parity",
    title: "Parity harness",
    description: "How Corvene is checked against GitHub Desktop 3.6.6, and where it deliberately differs.",
    category: "Internals",
  },
  {
    slug: "benchmarks",
    title: "Performance",
    description: "The performance budgets, what has been measured and how to reproduce it.",
    category: "Reference",
  },
  {
    slug: "flags",
    title: "Flags",
    description: "Presets, the Flags dialog and CORVENE_FLAGS: every deviation from GitHub Desktop is a switch.",
    category: "Reference",
  },
  {
    slug: "building",
    title: "Building from source",
    description: "Toolchain, Linux build dependencies, the checks CI runs and the Android build.",
    category: "Contributing",
  },
];

export const CATEGORIES = ["Getting started", "Internals", "Reference", "Contributing"];

export function docHref(base: string, slug: string): string {
  return slug === "overview" ? `${base}/docs/` : `${base}/docs/${slug}`;
}
