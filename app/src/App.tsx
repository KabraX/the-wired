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
  webDarkTheme,
  webLightTheme,
} from "@fluentui/react-components";
import { ArrowDownloadRegular, SearchRegular, SettingsRegular } from "@fluentui/react-icons";
import { api, isFinished, type AppInfo, type Job, type SearchResult } from "./api";
import SearchPage from "./pages/SearchPage";
import AnimePage from "./pages/AnimePage";
import DownloadsPage from "./pages/DownloadsPage";
import SettingsPage from "./pages/SettingsPage";

type Page = "buscar" | "descargas" | "ajustes";

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

// Sigue el tema claro/oscuro del sistema
function useSystemTheme() {
  const query = useMemo(() => window.matchMedia("(prefers-color-scheme: dark)"), []);
  const [dark, setDark] = useState(query.matches);
  useEffect(() => {
    const onChange = (e: MediaQueryListEvent) => setDark(e.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, [query]);
  return dark ? webDarkTheme : webLightTheme;
}

const titles: Record<Page, string> = { buscar: "Buscar anime", descargas: "Descargas", ajustes: "Ajustes" };

export default function App() {
  const theme = useSystemTheme();
  const styles = useStyles();
  const [page, setPage] = useState<Page>("buscar");
  const [opened, setOpened] = useState<SearchResult | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [jobs, setJobs] = useState<Job[]>([]);

  const reloadJobs = useCallback(() => api.listDownloads().then(setJobs), []);

  useEffect(() => {
    api.appInfo().then(setInfo).catch(() => setInfo(null));
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
  }, [reloadJobs]);

  const active = jobs.filter((j) => !isFinished(j)).length;
  // La búsqueda se conserva al entrar en un anime y volver
  const searchPage = <SearchPage onOpen={setOpened} />;

  return (
    <FluentProvider theme={theme}>
      <div className={styles.root}>
        <nav className={styles.nav}>
          <Title2>the-wired</Title2>
          <TabList vertical selectedValue={page} onTabSelect={(_, d) => setPage(d.value as Page)}>
            <Tab value="buscar" icon={<SearchRegular />}>Buscar</Tab>
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
          {!(page === "buscar" && opened) && <Title2>{titles[page]}</Title2>}
          <div style={{ display: page === "buscar" && !opened ? "contents" : "none" }}>{searchPage}</div>
          {page === "buscar" && opened && (
            <AnimePage result={opened} jobs={jobs} onBack={() => setOpened(null)} onQueued={() => setPage("descargas")} />
          )}
          {page === "descargas" && <DownloadsPage jobs={jobs} onChanged={reloadJobs} />}
          {page === "ajustes" && <SettingsPage info={info} />}
        </main>
      </div>
    </FluentProvider>
  );
}
