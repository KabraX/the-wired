// Tipos y llamadas al núcleo en Rust (comandos de Tauri).
import { invoke } from "@tauri-apps/api/core";

export type SearchResult = {
  title: string;
  url: string;
  image: string | null;
  kind: string | null;
  status: string | null;
};

export type Episode = { number: number; title: string };

export type AnimeInfo = {
  url: string;
  title: string;
  image: string | null;
  kind: string | null;
  is_movie: boolean;
  episodes: Episode[];
};

export type Settings = { library_dir: string; concurrent_downloads: number };

export type AppInfo = { version: string; os: string; arch: string; ytdlp: string | null };

export type JobStatus =
  | { state: "queued" }
  | { state: "resolving" }
  | { state: "downloading"; percent: number | null; speed: number | null; eta: number | null }
  | { state: "done" }
  | { state: "skipped" }
  | { state: "failed"; message: string }
  | { state: "cancelled" };

export type Job = {
  id: number;
  anime: string;
  anime_url: string;
  is_movie: boolean;
  season: number;
  episode: number;
  output: string;
  status: JobStatus;
};

// Error del scraper tal como lo serializa Rust (JkError)
type JkError = { kind: "Unavailable" | "NotFound" | "SiteChanged"; detail: string };

/** Mensaje claro para el usuario a partir de un error del núcleo. */
export function errorMessage(e: unknown): string {
  const err = e as Partial<JkError>;
  switch (err?.kind) {
    case "Unavailable":
      return "jkanime.net no responde. Comprueba tu conexión o vuelve a intentarlo en un rato.";
    case "NotFound":
      return "No se encontró la página en jkanime.net.";
    case "SiteChanged":
      return "La web de jkanime parece haber cambiado y the-wired necesita una actualización.";
    default:
      return typeof e === "string" ? e : "Error inesperado.";
  }
}

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  search: (query: string, page = 1) => invoke<SearchResult[]>("search", { query, page }),
  anime: (url: string) => invoke<AnimeInfo>("anime", { url }),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<Settings>("set_settings", { settings }),
  enqueueDownloads: (args: {
    anime: string;
    animeUrl: string;
    isMovie: boolean;
    season: number;
    episodes: number[];
  }) => invoke<number[]>("enqueue_downloads", { args }),
  listDownloads: () => invoke<Job[]>("list_downloads"),
  cancelDownload: (id: number) => invoke<void>("cancel_download", { id }),
  clearFinished: () => invoke<void>("clear_finished_downloads"),
  episodesOnDisk: (anime: string, season: number, episodes: number[]) =>
    invoke<number[]>("episodes_on_disk", { anime, season, episodes }),
  animeFolder: (anime: string) => invoke<string>("anime_folder", { anime }),
  updateYtdlp: () => invoke<string>("update_ytdlp"),
};

export function formatSpeed(bps: number | null): string {
  if (!bps || bps <= 0) return "";
  const units = ["B/s", "KB/s", "MB/s", "GB/s"];
  let v = bps;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
}

export function formatEta(s: number | null): string {
  if (s == null) return "";
  const m = Math.floor(s / 60);
  const sec = s % 60;
  return m >= 60 ? `${Math.floor(m / 60)} h ${m % 60} min` : m > 0 ? `${m} min ${sec} s` : `${sec} s`;
}

export const isFinished = (j: Job) =>
  ["done", "skipped", "failed", "cancelled"].includes(j.status.state);
