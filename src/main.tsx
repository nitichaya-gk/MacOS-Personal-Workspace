import { createRoot } from "react-dom/client";
import "@mantine/core/styles.css";
import "@blocknote/mantine/style.css";
import "./styles/global.css";
import { App } from "./app/App";

// Excalidraw loads its fonts from here (copied into public/ at build) instead of a CDN, so canvases work offline.
(window as unknown as { EXCALIDRAW_ASSET_PATH: string }).EXCALIDRAW_ASSET_PATH = "/";

createRoot(document.getElementById("root")!).render(<App />);
