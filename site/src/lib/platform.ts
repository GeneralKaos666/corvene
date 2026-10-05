export type Platform = "macos" | "windows" | "linux" | "android" | "other";

/** The visitor's operating system, from the user agent. */
export function detectPlatform(): Platform {
  if (typeof navigator === "undefined") return "other";
  const ua = navigator.userAgent;
  const uaPlatform =
    (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ||
    navigator.platform ||
    "";
  const hay = `${uaPlatform} ${ua}`;

  if (/android/i.test(hay)) return "android";
  if (/windows|win32|win64/i.test(hay)) return "windows";
  if (/cros/i.test(ua)) return "android"; // ChromeOS runs the Android package
  if (/iphone|ipad|ipod/i.test(ua)) return "other";
  if (/mac/i.test(hay)) return "macos";
  if (/linux|x11|freebsd|openbsd/i.test(hay)) return "linux";
  return "other";
}

/** A CPU family as the download picker names it. */
export type Arch = "x86_64" | "arm64" | "x86" | "armv7";

/** The visitor is on ChromeOS, which runs the Android packages (and Linux ones in its container). */
export function isChromeOS(): boolean {
  return typeof navigator !== "undefined" && /\bCrOS\b/.test(navigator.userAgent);
}

/** The visitor is on an iPhone or iPad, for which there is no build. */
export function isIOS(): boolean {
  if (typeof navigator === "undefined") return false;
  // iPadOS asks for desktop pages as a Mac with a touch screen
  return /iphone|ipad|ipod/i.test(navigator.userAgent) || (/mac/i.test(navigator.platform) && navigator.maxTouchPoints > 1);
}

/**
 * A best guess at the visitor's CPU, or null. Chromium browsers answer
 * through User-Agent Client Hints; elsewhere the user agent string (Linux and
 * ChromeOS name the machine) and, on a Mac, the WebGL renderer ("Apple M2",
 * "Apple GPU") stand in. Browsers freeze or hide these, so it is a hint.
 */
export async function detectArch(platform: Platform): Promise<Arch | null> {
  if (typeof navigator === "undefined") return null;
  const ua = navigator.userAgent;
  type HighEntropy = { architecture?: string; bitness?: string };
  const uaData = (navigator as Navigator & { userAgentData?: { getHighEntropyValues?: (hints: string[]) => Promise<HighEntropy> } })
    .userAgentData;

  // Android reports no architecture through the hints
  if (uaData?.getHighEntropyValues && platform !== "android") {
    try {
      const { architecture, bitness } = await uaData.getHighEntropyValues(["architecture", "bitness"]);
      if (architecture === "arm") return bitness === "32" ? "armv7" : "arm64";
      if (architecture === "x86") return bitness === "32" ? "x86" : "x86_64";
    } catch {
      // the hints were refused: fall through
    }
  }

  if (/aarch64|arm64/i.test(ua)) return "arm64";
  // "armv8l" is a 32-bit browser on a 64-bit CPU: no answer
  if (/armv7|armhf/i.test(ua)) return "armv7";
  if (/x86_64|amd64|x64|win64|wow64/i.test(ua)) return "x86_64";
  if (/i[3-6]86/i.test(ua)) return "x86";

  if (platform === "macos") {
    // every Mac browser says "Intel Mac OS X"; the GPU tells Apple Silicon apart
    try {
      const gl = document.createElement("canvas").getContext("webgl");
      const info = gl?.getExtension("WEBGL_debug_renderer_info");
      const renderer = gl && info ? String(gl.getParameter(info.UNMASKED_RENDERER_WEBGL)) : "";
      if (/apple (m\d|gpu)/i.test(renderer)) return "arm64";
      if (/intel|amd|radeon|nvidia/i.test(renderer)) return "x86_64";
    } catch {
      // no WebGL
    }
  }
  if (platform === "windows" && /windows nt/i.test(ua)) return "x86";
  return null;
}
