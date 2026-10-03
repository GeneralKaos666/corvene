/**
 * The GitHub REST calls the site makes, each fetched once per page load and
 * remembered in sessionStorage so navigating between pages does not spend
 * the 60-per-hour unauthenticated quota.
 */

export const REPO = "wasi-master/corvene";
export const REPO_URL = `https://github.com/${REPO}`;
export const RELEASES_URL = `${REPO_URL}/releases`;
export const BREW_COMMAND = "brew install --cask wasi-master/corvene/corvene";

/** A release asset as the GitHub Releases API lists it. */
export interface ReleaseAsset {
  name: string;
  browser_download_url: string;
  size: number;
  /** `sha256:<hex>`; GitHub computes it at upload */
  digest?: string | null;
  download_count: number;
  updated_at: string;
}

export interface LatestRelease {
  /** without the leading "v" */
  version: string;
  htmlUrl: string;
  assets: ReleaseAsset[];
}

const TTL_MS = 10 * 60 * 1000;
const memo = new Map<string, Promise<unknown>>();

async function cachedJson<T>(path: string): Promise<T | null> {
  const existing = memo.get(path) as Promise<T | null> | undefined;
  if (existing) return existing;

  const key = `gh:${path}`;
  const promise = (async () => {
    try {
      const raw = sessionStorage.getItem(key);
      if (raw) {
        const cached = JSON.parse(raw) as { at: number; data: T };
        if (Date.now() - cached.at < TTL_MS) return cached.data;
      }
    } catch {
      // sessionStorage unavailable: fall through to the network
    }
    try {
      const res = await fetch(`https://api.github.com${path}`, {
        headers: { Accept: "application/vnd.github+json" },
      });
      if (!res.ok) return null;
      const data = (await res.json()) as T;
      try {
        sessionStorage.setItem(key, JSON.stringify({ at: Date.now(), data }));
      } catch {
        // quota or private mode: fine without the cache
      }
      return data;
    } catch {
      return null;
    }
  })();
  memo.set(path, promise);
  return promise;
}

interface ApiRelease {
  tag_name?: string;
  html_url?: string;
  assets?: ReleaseAsset[];
}

/** The latest non-prerelease release, or null when offline or rate limited. */
export async function fetchLatestRelease(): Promise<LatestRelease | null> {
  const data = await cachedJson<ApiRelease>(`/repos/${REPO}/releases/latest`);
  if (!data?.tag_name) return null;
  return {
    version: data.tag_name.replace(/^v/, ""),
    htmlUrl: data.html_url || RELEASES_URL,
    assets: Array.isArray(data.assets) ? data.assets : [],
  };
}

/** The repository's star count, or null. */
export async function fetchStarCount(): Promise<number | null> {
  const data = await cachedJson<{ stargazers_count?: number }>(`/repos/${REPO}`);
  return typeof data?.stargazers_count === "number" ? data.stargazers_count : null;
}

/** The download URL of a release asset by file name, from the release when it lists it. */
export function assetUrl(version: string, file: string, assets: ReleaseAsset[] = []): string {
  const live = assets.find((a) => a.name === file);
  return live ? live.browser_download_url : `${RELEASES_URL}/download/v${version}/${file}`;
}

export function formatCount(n: number): string {
  return n >= 1000 ? `${(n / 1000).toFixed(1).replace(/\.0$/, "")}k` : `${n}`;
}
