import { useCallback, useEffect, useRef, useState } from "react";

/** Copy text to the clipboard and report "copied" for a moment. */
export function useCopy(resetAfterMs = 2000) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  const copy = useCallback(
    async (text: string) => {
      try {
        if (navigator.clipboard?.writeText) {
          await navigator.clipboard.writeText(text);
        } else {
          // http:// and old browsers: the deprecated fallback still works
          const area = document.createElement("textarea");
          area.value = text;
          area.setAttribute("readonly", "");
          area.style.position = "fixed";
          area.style.opacity = "0";
          document.body.appendChild(area);
          area.select();
          document.execCommand("copy");
          area.remove();
        }
        setCopied(true);
        window.clearTimeout(timer.current);
        timer.current = window.setTimeout(() => setCopied(false), resetAfterMs);
      } catch {
        setCopied(false);
      }
    },
    [resetAfterMs]
  );

  return { copied, copy };
}
