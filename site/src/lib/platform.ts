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
