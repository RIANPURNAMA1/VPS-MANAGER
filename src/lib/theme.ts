export type ThemeChoice = "light" | "dark" | "system";
const KEY = "vpsm.theme";

export function loadTheme(): ThemeChoice {
  try {
    const v = localStorage.getItem(KEY);
    if (v === "light" || v === "dark" || v === "system") return v;
  } catch { /* storage tidak tersedia */ }
  return "system";
}

export function saveTheme(t: ThemeChoice): void {
  try { localStorage.setItem(KEY, t); } catch { /* abaikan */ }
}

export function resolveTheme(t: ThemeChoice, systemDark: boolean): "light" | "dark" {
  return t === "system" ? (systemDark ? "dark" : "light") : t;
}

export function applyTheme(t: ThemeChoice): "light" | "dark" {
  const systemDark = typeof window !== "undefined" && window.matchMedia?.("(prefers-color-scheme: dark)").matches;
  const r = resolveTheme(t, !!systemDark);
  document.documentElement.classList.toggle("dark", r === "dark");
  document.documentElement.style.colorScheme = r;
  return r;
}
