import { WebglAddon } from "@xterm/addon-webgl";
import type { Terminal } from "@xterm/xterm";
import type { Platform } from "../../generated/Platform";

export type RendererKind = "webgl" | "dom";

/** WebGL when available; xterm's DOM renderer otherwise (xterm 6 has no canvas renderer). */
export function loadRenderer(term: Terminal, platform: Platform, onChange: (kind: RendererKind) => void): void {
  try {
    // WebKitGTK shows a WebGL canvas one frame behind unless the drawing buffer is preserved (WebKit bug 324549).
    const webgl = new WebglAddon(platform === "linux");
    webgl.onContextLoss(() => {
      webgl.dispose();
      onChange("dom");
    });
    term.loadAddon(webgl);
    onChange("webgl");
  } catch {
    onChange("dom");
  }
}
