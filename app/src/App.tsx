import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  CounterBadge,
  FluentProvider,
  Tab,
  TabList,
  Text,
  Title2,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { ArrowDownloadRegular, LibraryRegular, SearchRegular, SettingsRegular } from "@fluentui/react-icons";
import {
  api,
  enabledSites,
  isFinished,
  type AppInfo,
  type Job,
  type SearchResult,
  type Settings,
  type Site,
  type ToolsStatus,
} from "./api";
import ToolsBanner from "./ToolsBanner";
import UpdateBanner from "./UpdateBanner";
import SearchPage from "./pages/SearchPage";
import AnimePage from "./pages/AnimePage";
import DownloadsPage from "./pages/DownloadsPage";
import SettingsPage from "./pages/SettingsPage";
import LibraryPage from "./pages/LibraryPage";
import { buildTheme, loadPrefs, savePrefs, type ThemePrefs } from "./theme";

type Page = "buscar" | "biblioteca" | "descargas" | "ajustes";

const useStyles = makeStyles({
  root: {
    display: "flex",
    height: "100vh",
    backgroundColor: tokens.colorNeutralBackground2,
    color: tokens.colorNeutralForeground1,
  },
  nav: {
    width: "220px",
    flexShrink: 0,
    padding: tokens.spacingVerticalL,
    display: "flex",
    flexDirection: "column",
    gap: tokens.spacingVerticalL,
    borderRight: `1px solid ${tokens.colorNeutralStroke2}`,
  },
  content: {
    flex: 1,
    overflowY: "auto",
    padding: tokens.spacingHorizontalXXL,
    display: "flex",
    flexDirection: "column",
    gap: tokens.spacingVerticalL,
  },
  footer: { marginTop: "auto", color: tokens.colorNeutralForeground3 },
});

// ¿El sistema está en modo oscuro? (se actualiza si cambia)
function useSystemDark() {
  const query = useMemo(() => window.matchMedia("(prefers-color-scheme: dark)"), []);
  const [dark, setDark] = useState(query.matches);
  useEffect(() => {
    const onChange = (e: MediaQueryListEvent) => setDark(e.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, [query]);
  return dark;
}

const titles: Record<Page, string> = {
  buscar: "Buscar anime",
  biblioteca: "Biblioteca",
  descargas: "Descargas",
  ajustes: "Ajustes",
};

export default function App() {
  const systemDark = useSystemDark();
  const [prefs, setPrefs] = useState<ThemePrefs>(loadPrefs);
  const dark = prefs.mode === "system" ? systemDark : prefs.mode === "dark";
  const theme = useMemo(() => buildTheme(prefs.accent, dark), [prefs.accent, dark]);
  const changePrefs = (p: ThemePrefs) => {
    setPrefs(p);
    savePrefs(p);
  };
  const styles = useStyles();
  const [page, setPage] = useState<Page>("buscar");
  const [opened, setOpened] = useState<SearchResult | null>(null);
  // Anime abierto desde la biblioteca (su "Volver" regresa a la biblioteca)
  const [libOpened, setLibOpened] = useState<SearchResult | null>(null);
  // Búsqueda pedida desde otra página (un anime de la biblioteca sin enlace guardado)
  const [searchRequest, setSearchRequest] = useState<{ source: Site; query: string; n: number } | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [jobs, setJobs] = useState<Job[]>([]);
  const [tools, setTools] = useState<ToolsStatus | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const saveSettings = async (s: Settings) => {
    const saved = await api.setSettings(s);
    setSettings(saved);
    return saved;
  };
  const reloadTools = useCallback(() => api.toolsStatus().then(setTools), []);

  const reloadJobs = useCallback(() => api.listDownloads().then(setJobs), []);

  useEffect(() => {
    api.appInfo().then(setInfo).catch(() => setInfo(null));
    api.getSettings().then(setSettings);
    reloadTools();
    reloadJobs();
    // Cada cambio de una descarga llega como evento desde Rust
    const un = listen<Job>("download-updated", ({ payload }) =>
      setJobs((prev) => {
        const i = prev.findIndex((j) => j.id === payload.id);
        if (i === -1) return [...prev, payload];
        const next = prev.slice();
        next[i] = payload;
        return next;
      }),
    );
    return () => {
      un.then((f) => f());
    };
  }, [reloadJobs, reloadTools]);

  const active = jobs.filter((j) => !isFinished(j) && j.status.state !== "paused").length;
  // La búsqueda se conserva al entrar en un anime y volver
  const searchPage = <SearchPage sources={enabledSites(settings)} request={searchRequest} onOpen={setOpened} />;
  const animeOpen = (page === "buscar" && opened) || (page === "biblioteca" && libOpened);

  return (
    <FluentProvider theme={theme}>
      <div className={styles.root}>
        <nav className={styles.nav}>
          <Title2>the-wired</Title2>
          <TabList vertical selectedValue={page} onTabSelect={(_, d) => setPage(d.value as Page)}>
            <Tab value="buscar" icon={<SearchRegular />}>Buscar</Tab>
            <Tab value="biblioteca" icon={<LibraryRegular />}>Biblioteca</Tab>
            <Tab value="descargas" icon={<ArrowDownloadRegular />}>
              Descargas {active > 0 && <CounterBadge count={active} size="small" />}
            </Tab>
            <Tab value="ajustes" icon={<SettingsRegular />}>Ajustes</Tab>
          </TabList>
          <Text size={200} className={styles.footer}>
            {info ? `v${info.version} · ${info.os} ${info.arch}` : "Conectando con el núcleo…"}
          </Text>
        </nav>
        <main className={styles.content}>
          <UpdateBanner />
          <ToolsBanner status={tools} onChange={setTools} />
          {!animeOpen && <Title2>{titles[page]}</Title2>}
          <div style={{ display: page === "buscar" && !opened ? "contents" : "none" }}>{searchPage}</div>
          {page === "buscar" && opened && (
            <AnimePage result={opened} jobs={jobs} onBack={() => setOpened(null)} onQueued={() => setPage("descargas")} />
          )}
          {/* La biblioteca sigue montada al abrir un anime: volver no relee el disco */}
          <div style={{ display: page === "biblioteca" && !libOpened ? "contents" : "none" }}>
            <LibraryPage
              sources={enabledSites(settings)}
              jobs={jobs}
              onOpen={setLibOpened}
              onSearch={(source, query) => {
                setOpened(null);
                setPage("buscar");
                setSearchRequest({ source, query, n: Date.now() });
              }}
            />
          </div>
          {page === "biblioteca" && libOpened && (
            <AnimePage
              result={libOpened}
              jobs={jobs}
              onBack={() => setLibOpened(null)}
              onQueued={() => setPage("descargas")}
            />
          )}
          {page === "descargas" && <DownloadsPage jobs={jobs} onChanged={reloadJobs} />}
          {page === "ajustes" && <SettingsPage
              settings={settings}
              onSettingsChange={saveSettings}
              tools={tools}
              onToolsChanged={reloadTools}
              themePrefs={prefs}
              onThemeChange={changePrefs}
            />}
        </main>
      </div>
    </FluentProvider>
  );
}
