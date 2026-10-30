import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((f) => {
    const p = join(dir, f);
    return statSync(p).isDirectory() ? walk(p) : [p];
  });
}

/** Menjaga aturan desain: hanya hitam/putih/abu-abu netral, tanpa warna aksen atau gradient. */
describe("desain monokrom", () => {
  const files = walk("src").filter((f) => /\.(tsx?|css)$/.test(f));
  const hexes = new Set<string>();
  for (const f of files) {
    for (const m of readFileSync(f, "utf8").matchAll(/#([0-9a-fA-F]{6}|[0-9a-fA-F]{3})\b/g)) hexes.add(m[1]!.toLowerCase());
  }
  it("setiap warna hex bersifat netral (R=G=B)", () => {
    for (const h of hexes) {
      const x = h.length === 3 ? h.split("").map((c) => c + c).join("") : h;
      expect(x.slice(0, 2) === x.slice(2, 4) && x.slice(2, 4) === x.slice(4, 6), `#${h}`).toBe(true);
    }
  });
  it("tidak memakai kelas warna Tailwind berwarna atau gradient", () => {
    const bad = /\b(?:bg|text|border|ring|from|to|via|fill|stroke|accent|outline)-(?:red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose)-\d{2,3}\b|\bbg-gradient-|\bbackdrop-blur/;
    for (const f of files) expect(bad.test(readFileSync(f, "utf8")), f).toBe(false);
  });
});
