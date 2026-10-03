/**
 * Structured data (schema.org JSON-LD) and the shared SEO constants. Every
 * builder returns a plain object; BaseLayout serialises them into one
 * `<script type="application/ld+json">` per page.
 */
import { REPO_URL, RELEASES_URL } from "./github";

export const SITE_NAME = "Corvene";
export const SITE_TAGLINE = "A native GitHub Desktop clone in Rust";
export const SITE_DESCRIPTION =
  "Corvene is a fast, low-memory recreation of GitHub Desktop written in Rust: the same layout, menus, dialogs and workflow, rendered with GPUI instead of Electron. Free and open source for macOS, Windows, Linux and Android.";

export const AUTHOR = {
  "@type": "Person",
  name: "Wasi Master",
  url: "https://github.com/wasi-master",
} as const;

/** The canonical absolute URL of a page: the built site serves directories, so every page ends in "/". */
export function pageUrl(pathname: string, site: URL | undefined): string {
  const url = new URL(pathname.endsWith("/") ? pathname : `${pathname}/`, site);
  return url.toString();
}

export interface SiteContext {
  /** absolute URL of the site root with the base path, no trailing slash */
  root: string;
  /** absolute URL of this page */
  page: string;
}

export function websiteLd(ctx: SiteContext) {
  return {
    "@context": "https://schema.org",
    "@type": "WebSite",
    "@id": `${ctx.root}/#website`,
    url: `${ctx.root}/`,
    name: SITE_NAME,
    alternateName: `${SITE_NAME}: ${SITE_TAGLINE}`,
    description: SITE_DESCRIPTION,
    inLanguage: "en",
    publisher: AUTHOR,
  };
}

export function softwareApplicationLd(ctx: SiteContext, version: string, screenshot: string) {
  return {
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    "@id": `${ctx.root}/#software`,
    name: SITE_NAME,
    alternateName: "Corvene Git client",
    description: SITE_DESCRIPTION,
    url: `${ctx.root}/`,
    image: screenshot,
    screenshot,
    applicationCategory: "DeveloperApplication",
    applicationSubCategory: "Git client",
    operatingSystem: "macOS 10.15+, Windows 10+, Linux (glibc 2.35+), Android 8.0+",
    softwareVersion: version,
    downloadUrl: RELEASES_URL,
    installUrl: `${ctx.root}/docs/installation`,
    softwareHelp: { "@type": "CreativeWork", url: `${ctx.root}/docs/` },
    releaseNotes: RELEASES_URL,
    license: `${REPO_URL}/blob/main/LICENSE`,
    isAccessibleForFree: true,
    offers: { "@type": "Offer", price: "0", priceCurrency: "USD", availability: "https://schema.org/InStock" },
    author: AUTHOR,
    maintainer: AUTHOR,
    programmingLanguage: "Rust",
    codeRepository: REPO_URL,
    sameAs: [REPO_URL],
    featureList: [
      "One-to-one recreation of GitHub Desktop 3.6.6's interface and workflow",
      "GPU rendering with GPUI (Metal, Vulkan, DirectX)",
      "In-process repository reads with gitoxide, writes through your own git",
      "Syntax highlighting for 300+ languages with optional tree-sitter grammars",
      "No telemetry",
      "Every deviation from GitHub Desktop behind a switchable flag",
    ],
    keywords: "GitHub Desktop, Git client, Git GUI, Rust, GPUI, gitoxide, macOS, Windows, Linux, Android",
  };
}

export interface Crumb {
  name: string;
  url: string;
}

export function breadcrumbLd(crumbs: Crumb[]) {
  return {
    "@context": "https://schema.org",
    "@type": "BreadcrumbList",
    itemListElement: crumbs.map((crumb, index) => ({
      "@type": "ListItem",
      position: index + 1,
      name: crumb.name,
      item: crumb.url,
    })),
  };
}

export function techArticleLd(ctx: SiteContext, article: { title: string; description: string; modified: Date; image: string }) {
  return {
    "@context": "https://schema.org",
    "@type": "TechArticle",
    "@id": `${ctx.page}#article`,
    mainEntityOfPage: ctx.page,
    url: ctx.page,
    headline: article.title,
    description: article.description,
    image: article.image,
    inLanguage: "en",
    dateModified: article.modified.toISOString(),
    author: AUTHOR,
    publisher: AUTHOR,
    isPartOf: { "@id": `${ctx.root}/#website` },
    about: { "@id": `${ctx.root}/#software` },
    proficiencyLevel: "Beginner",
  };
}

export interface Faq {
  question: string;
  answer: string;
}

export function faqLd(faqs: Faq[]) {
  return {
    "@context": "https://schema.org",
    "@type": "FAQPage",
    mainEntity: faqs.map((faq) => ({
      "@type": "Question",
      name: faq.question,
      acceptedAnswer: { "@type": "Answer", text: faq.answer },
    })),
  };
}

/** The landing page's FAQ, shown on the page and as FAQPage structured data. */
export const FAQS: Faq[] = [
  {
    question: "Is Corvene made by GitHub?",
    answer:
      "No. Corvene is an independent open-source project and is not affiliated with, sponsored by or endorsed by GitHub, Inc. It recreates the interface of GitHub Desktop 3.6.6 with its own engine written in Rust.",
  },
  {
    question: "Is it free?",
    answer: "Yes. Corvene is free software under the MIT License, with no paid tiers, accounts or telemetry.",
  },
  {
    question: "Why does macOS say the app cannot be verified?",
    answer:
      "Corvene is signed with a self-signed certificate rather than notarized by Apple, so Gatekeeper blocks the first launch of a downloaded copy. Allow it once under System Settings › Privacy & Security › Open Anyway, or install with Homebrew, which clears the quarantine flag.",
  },
  {
    question: "Does it work with my existing GitHub Desktop setup?",
    answer:
      "Yes. Corvene uses your own git, so hooks, credential helpers and configuration keep working, and it can import your repository list from GitHub Desktop. Sign in once more so the token is stored in Corvene's own keychain entry.",
  },
  {
    question: "Which platforms are supported?",
    answer:
      "macOS 10.15.7 or newer (Intel) and 11 or newer (Apple Silicon), Windows 10 and 11, Linux distributions with glibc 2.35 or newer, and Android 8.0 or newer. Windows builds are new and Android is experimental.",
  },
  {
    question: "Can I make it behave exactly like GitHub Desktop?",
    answer:
      "Yes. Every deviation from GitHub Desktop 3.6.6 is a flag, and the GitHub Desktop preset switches all of them off, bugs included.",
  },
];
