// Tipos y llamadas al núcleo en Rust (comandos de Tauri).
import { invoke } from "@tauri-apps/api/core";

/** Sitios que sabe leer el núcleo (todos de la plataforma de jkanime). */
export type Site = "jkanime" | "hentaijk";
export const SITES: { id: Site; name: string; adult: boolean }[] = [
  { id: "jkanime", name: "jkanime", adult: false },
  { id: "hentaijk", name: "HentaiJK", adult: true },
];
export const siteName = (s: Site) => SITES.find((x) => x.id === s)?.name ?? s;
/** Sitio de una URL de ficha o episodio (las descargas solo guardan la URL). */
export const siteFromUrl = (url: string): Site => (/^https?:\/\/(www\.)?hentaijk\.com\//i.test(url) ? "hentaijk" : "jkanime");

export type SearchResult = {
  source: Site;
  title: string;
  url: string;
  image: string | null;
  kind: string | null;
  status: string | null;
};

export type Episode = { number: number; title: string };

export type AnimeInfo = {
  source: Site;
  url: string;
  title: string;
  image: string | null;
  kind: string | null;
  quality: string | null;
  is_movie: boolean;
  episodes: Episode[];
};

export type SourceSettings = { enabled: boolean; library_dir: string; jellyfin_layout: boolean };
export type Settings = { concurrent_downloads: number; sources: Record<Site, SourceSettings> };

/** Sitios activos en los ajustes (jkanime siempre lo está). */
export const enabledSites = (s: Settings | null): Site[] =>
  SITES.filter((x) => x.id === "jkanime" || s?.sources[x.id]?.enabled).map((x) => x.id);

/** Un anime encontrado en la carpeta de la biblioteca de una fuente. */
export type LibraryAnime = {
  source: Site;
  name: string;
  path: string;
  seasons: number[];
  episodes: number;
  partial: number;
  bytes: number;
  modified: number;
  title: string | null;
  url: string | null;
  image: string | null;
};

export type AppInfo = { version: string; os: string; arch: string };

export type ToolSource = "app" | "system" | "missing";
export type ToolStatus = { source: ToolSource; version: string | null };
export type ToolsStatus = { ytdlp: ToolStatus; ffmpeg: ToolStatus; can_install_ffmpeg: boolean };
export type ToolsProgress = { tool: string; downloaded: number; total: number | null; done: boolean };

/** Falta algo que la app puede instalar sola. */
export const toolsMissing = (t: ToolsStatus) =>
  t.ytdlp.source === "missing" || (t.can_install_ffmpeg && t.ffmpeg.source === "missing");

export type JobStatus =
  | { state: "queued" }
  | { state: "resolving" }
  | { state: "downloading"; percent: number | null; speed: number | null; eta: number | null }
  | { state: "paused" }
  | { state: "waiting"; attempt: number; retry_at: number; message: string }
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
  attempts: number;
};

// Error del scraper tal como lo serializa Rust (JkError)
type JkError = { kind: "Unavailable" | "NotFound" | "SiteChanged"; detail: string };

/** Mensaje claro para el usuario a partir de un error del núcleo. */
export function errorMessage(e: unknown, site: Site): string {
  const err = e as Partial<JkError>;
  const name = siteName(site);
  switch (err?.kind) {
    case "Unavailable":
      return `${name} no responde. Comprueba tu conexión o vuelve a intentarlo en un rato.`;
    case "NotFound":
      return `No se encontró la página en ${name}.`;
    case "SiteChanged":
      return `La web de ${name} parece haber cambiado y the-wired necesita una actualización.`;
    default:
      return typeof e === "string" ? e : "Error inesperado.";
  }
}

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  search: (source: Site, query: string, page = 1) => invoke<SearchResult[]>("search", { source, query, page }),
  anime: (url: string) => invoke<AnimeInfo>("anime", { url }),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<Settings>("set_settings", { settings }),
  enqueueDownloads: (args: {
    anime: string;
    animeUrl: string;
    title?: string;
    image?: string | null;
    isMovie: boolean;
    season: number;
    episodes: number[];
  }) => invoke<number[]>("enqueue_downloads", { args }),
  listDownloads: () => invoke<Job[]>("list_downloads"),
  cancelDownload: (id: number) => invoke<void>("cancel_download", { id }),
  pauseDownload: (id: number) => invoke<void>("pause_download", { id }),
  resumeDownload: (id: number) => invoke<void>("resume_download", { id }),
  pauseAll: () => invoke<void>("pause_all_downloads"),
  resumeAll: () => invoke<void>("resume_all_downloads"),
  clearFinished: () => invoke<void>("clear_finished_downloads"),
  episodesOnDisk: (source: Site, anime: string, isMovie: boolean, season: number, episodes: number[]) =>
    invoke<number[]>("episodes_on_disk", { source, anime, isMovie, season, episodes }),
  seasonsOnDisk: (source: Site, anime: string) => invoke<number[]>("seasons_on_disk", { source, anime }),
  splitSeason: (title: string) => invoke<[string, number]>("split_season", { title }),
  animeFolder: (source: Site, anime: string) => invoke<string>("anime_folder", { source, anime }),
  scanLibrary: () => invoke<LibraryAnime[]>("scan_library"),
  resolveLibraryAnime: (source: Site, name: string) =>
    invoke<{ title: string; url: string; image: string | null } | null>("resolve_library_anime", { source, name }),
  readLog: () => invoke<string>("read_log"),
  logFile: () => invoke<string>("log_file"),
  updateYtdlp: () => invoke<string>("update_ytdlp"),
  toolsStatus: () => invoke<ToolsStatus>("tools_status"),
  installTools: () => invoke<ToolsStatus>("install_tools"),
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
