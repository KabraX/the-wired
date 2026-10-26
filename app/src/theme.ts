// Temas de Fluent: azul por defecto (el de Fluent) y otros colores de su paleta.
import {
  createDarkTheme,
  createLightTheme,
  webDarkTheme,
  webLightTheme,
  type BrandVariants,
  type Theme,
} from "@fluentui/react-components";

export type ThemeMode = "system" | "light" | "dark";

export type AccentPreset = { id: string; name: string; color: string | null };

// Colores de la paleta compartida de Fluent. `null` = el azul de marca original.
export const ACCENTS: AccentPreset[] = [
  { id: "fluent", name: "Azul (Fluent)", color: null },
  { id: "cornflower", name: "Aciano", color: "#4f6bed" },
  { id: "teal", name: "Verde azulado", color: "#038387" },
  { id: "green", name: "Verde", color: "#107c10" },
  { id: "marigold", name: "Caléndula", color: "#c19c00" },
  { id: "pumpkin", name: "Calabaza", color: "#ca5010" },
  { id: "red", name: "Rojo", color: "#d13438" },
  { id: "berry", name: "Baya", color: "#c239b3" },
  { id: "grape", name: "Uva", color: "#881798" },
  { id: "lavender", name: "Lavanda", color: "#7160e8" },
  { id: "steel", name: "Acero", color: "#005b70" },
];

export type ThemePrefs = { mode: ThemeMode; accent: string };

const KEY = "the-wired.theme";
export const DEFAULT_PREFS: ThemePrefs = { mode: "system", accent: "fluent" };

export function loadPrefs(): ThemePrefs {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) return { ...DEFAULT_PREFS, ...JSON.parse(raw) };
  } catch {
    // almacenamiento no disponible: usar los valores por defecto
  }
  return DEFAULT_PREFS;
}

export function savePrefs(p: ThemePrefs) {
  try {
    localStorage.setItem(KEY, JSON.stringify(p));
  } catch {
    // sin almacenamiento, el cambio dura solo esta sesión
  }
}

function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.replace("#", ""), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function mix(a: [number, number, number], b: [number, number, number], t: number): string {
  const c = a.map((v, i) => Math.round(v + (b[i] - v) * t));
  return "#" + c.map((v) => v.toString(16).padStart(2, "0")).join("");
}

/** Gama de 16 tonos de marca (10 = más oscuro … 160 = más claro) con el color base en el 80. */
export function brandRamp(hex: string): BrandVariants {
  const base = hexToRgb(hex);
  const black: [number, number, number] = [0, 0, 0];
  const white: [number, number, number] = [255, 255, 255];
  const ramp: Record<number, string> = {};
  for (let i = 1; i <= 16; i++) {
    const step = i * 10;
    ramp[step] =
      step < 80 ? mix(base, black, ((80 - step) / 70) * 0.85) : step === 80 ? hex : mix(base, white, ((step - 80) / 80) * 0.85);
  }
  return ramp as unknown as BrandVariants;
}

export function buildTheme(accent: string, dark: boolean): Theme {
  const preset = ACCENTS.find((a) => a.id === accent);
  if (!preset?.color) return dark ? webDarkTheme : webLightTheme;
  const brand = brandRamp(preset.color);
  if (!dark) return createLightTheme(brand);
  // En oscuro, Fluent usa tonos más claros de la marca para mantener el contraste
  const theme = createDarkTheme(brand);
  theme.colorBrandForeground1 = brand[110];
  theme.colorBrandForeground2 = brand[120];
  return theme;
}
