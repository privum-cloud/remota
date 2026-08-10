import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";

// OS clipboard via the Tauri plugin (via Rust) — works in the WebKitGTK webview, where the
// browser `navigator.clipboard` API is blocked.

export async function clipRead(): Promise<string> {
  try {
    return (await readText()) ?? "";
  } catch {
    return "";
  }
}

export async function clipWrite(text: string): Promise<void> {
  try {
    await writeText(text);
  } catch {
    /* noop */
  }
}
