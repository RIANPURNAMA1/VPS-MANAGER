import { beforeEach, describe, expect, it } from "vitest";
import { applyTheme, loadTheme, resolveTheme, saveTheme } from "../src/lib/theme";

describe("theme", () => {
  beforeEach(() => { localStorage.clear(); document.documentElement.className = ""; });
  it("defaults to system and persists the choice locally", () => {
    expect(loadTheme()).toBe("system");
    saveTheme("dark");
    expect(loadTheme()).toBe("dark");
    localStorage.setItem("vpsm.theme", "nonsense");
    expect(loadTheme()).toBe("system");
  });
  it("resolves system against OS preference", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
    expect(resolveTheme("light", true)).toBe("light");
  });
  it("applies the dark class", () => {
    expect(applyTheme("dark")).toBe("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    applyTheme("light");
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });
});
