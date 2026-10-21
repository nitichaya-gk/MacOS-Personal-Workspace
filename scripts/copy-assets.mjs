// Copies Excalidraw fonts into public/ so canvases render offline (no CDN).
import { cpSync, existsSync } from "node:fs";

const src = "node_modules/@excalidraw/excalidraw/dist/prod/fonts";
const dest = "public/fonts";
if (!existsSync(dest)) {
  cpSync(src, dest, { recursive: true });
  console.log("copied Excalidraw fonts to public/fonts");
}
