import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
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
import {
  ArrowDownloadRegular,
  SearchRegular,
  SettingsRegular,
} from "@fluentui/react-icons";

type AppInfo = { version: string; os: string; arch: string };
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
    padding: tokens.spacingVerticalL,
    display: "flex",
    flexDirection: "column",
    gap: tokens.spacingVerticalL,
    borderRight: `1px solid ${tokens.colorNeutralStroke2}`,
  },
  content: {
    flex: 1,
    padding: tokens.spacingHorizontalXXL,
    display: "flex",
    flexDirection: "column",
    gap: tokens.spacingVerticalM,
  },
  footer: {
    marginTop: "auto",
    color: tokens.colorNeutralForeground3,
  },
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

const titles: Record<Page, string> = {
  buscar: "Buscar anime",
  descargas: "Descargas",
  ajustes: "Ajustes",
};

export default function App() {
  const theme = useSystemTheme();
  const styles = useStyles();
  const [page, setPage] = useState<Page>("buscar");
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    invoke<AppInfo>("app_info").then(setInfo).catch(() => setInfo(null));
  }, []);

  return (
    <FluentProvider theme={theme}>
      <div className={styles.root}>
        <nav className={styles.nav}>
          <Title2>the-wired</Title2>
          <TabList
            vertical
            selectedValue={page}
            onTabSelect={(_, d) => setPage(d.value as Page)}
          >
            <Tab value="buscar" icon={<SearchRegular />}>Buscar</Tab>
            <Tab value="descargas" icon={<ArrowDownloadRegular />}>Descargas</Tab>
            <Tab value="ajustes" icon={<SettingsRegular />}>Ajustes</Tab>
          </TabList>
          <Text size={200} className={styles.footer}>
            {info ? `v${info.version} · ${info.os} ${info.arch}` : "Conectando con el núcleo…"}
          </Text>
        </nav>
        <main className={styles.content}>
          <Title2>{titles[page]}</Title2>
          <Text>En construcción (fase 0).</Text>
        </main>
      </div>
    </FluentProvider>
  );
}
