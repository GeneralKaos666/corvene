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
    description: "What Corvene is and how it differs from GitHub Desktop.",
    category: "Getting started",
  },
  {
    slug: "quickstart",
    title: "Quickstart",
    description: "Install, sign in, add a repository.",
    category: "Getting started",
  },
  {
    slug: "installation",
    title: "Installation",
    description: "Every package for every platform, and Standard vs Full.",
    category: "Getting started",
  },
  {
    slug: "architecture",
    title: "Architecture",
    description: "How the code is laid out and how it talks to git.",
    category: "Internals",
  },
  {
    slug: "parity",
    title: "Parity harness",
    description: "How Corvene is compared against GitHub Desktop, screenshot by screenshot.",
    category: "Internals",
  },
  {
    slug: "benchmarks",
    title: "Performance",
    description: "Corvene vs GitHub Desktop and GitKraken, measured on one Mac.",
    category: "Reference",
  },
  {
    slug: "flags",
    title: "Flags",
    description: "Switch Corvene's changes on and off, one by one or with a preset.",
    category: "Reference",
  },
  {
    slug: "building",
    title: "Building from source",
    description: "Build it yourself, desktop and Android.",
    category: "Contributing",
  },
];

export const CATEGORIES = ["Getting started", "Internals", "Reference", "Contributing"];

export function docHref(base: string, slug: string): string {
  return slug === "overview" ? `${base}/docs/` : `${base}/docs/${slug}`;
}
