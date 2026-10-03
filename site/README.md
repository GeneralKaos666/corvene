# Corvene website

The landing page and documentation at <https://wasi-master.github.io/corvene/>, built with [Astro](https://astro.build), Tailwind CSS 4 and a few React islands.

```bash
npm ci
npm run dev        # http://localhost:4321/corvene/
npm run build      # dist/
npx astro check    # types
```

## Layout

- `src/pages/index.astro`: the landing page. `src/pages/docs/*.astro`: one documentation page each, plain HTML styled by `.docs-prose` in `src/styles/global.css`; `src/data/docs.ts` orders them in the sidebar.
- `src/components/*.tsx`: the interactive parts. `HeroDownload` picks the visitor's platform, `DownloadsTable` lists every package of the latest release, `BenchmarkComparison` draws `src/data/benchmarks.ts`, `MacQuarantineModal` explains Gatekeeper after a macOS download, `AssetPopover` shows a file's size and sha256 on hover.
- `src/lib/github.ts`: the two GitHub API calls (latest release, stars), fetched once per page and cached in `sessionStorage`. `src/lib/assets.ts`: the asset names `packaging/release.md` documents. `src/lib/version.ts`: the workspace version from `Cargo.toml`, used before the API answers.
- `src/styles/global.css`: the Primer colour tokens as Tailwind utilities (`bg-canvas`, `text-fg-muted`, `border-edge`, `text-accent`…), the fonts (Mona Sans, Hubot Sans, JetBrains Mono, bundled from npm), buttons and the docs typography.

## Numbers

`src/data/benchmarks.ts` holds every figure the site shows, with its source. Launch time, idle memory and installed size for Corvene, GitHub Desktop and GitKraken come from `tools/perf/apps.py` (raw results in `tools/perf/results/`); the interaction latencies from the project's latency bench. A `null` competitor value renders as "not measured yet".

## Screenshot

`public/app-screenshot.webp` is rendered by `scripts/screenshot.py` from a Corvene build with the `snapshots` feature, over its control socket (see `tools/parity`): 1367×814 at 2× with the empty title bar cropped, dark theme, on the parity fixture repository.

## Deploying

`.github/workflows/deploy-pages.yml` builds `site/` and publishes `dist/` to GitHub Pages on every push to `main` that touches it. The repository's Pages source must be set to "GitHub Actions" once.
