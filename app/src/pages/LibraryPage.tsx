import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Badge,
  Button,
  Card,
  CardHeader,
  CardPreview,
  Input,
  Text,
  Tooltip,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { ArrowClockwiseRegular, FilterRegular, FolderOpenRegular, SearchRegular } from "@fluentui/react-icons";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, isFinished, type Job, type LibraryAnime, type SearchResult, type Site } from "../api";
import Loading from "../Loading";
import SourceLogo from "../SourceLogo";

const useStyles = makeStyles({
  bar: { display: "flex", gap: tokens.spacingHorizontalS, alignItems: "center", flexWrap: "wrap" },
  summary: { marginLeft: "auto", color: tokens.colorNeutralForeground3 },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(190px, 1fr))",
    gap: tokens.spacingHorizontalL,
  },
  card: { cursor: "pointer" },
  poster: { aspectRatio: "2 / 3", objectFit: "cover", width: "100%" },
  noPoster: {
    aspectRatio: "2 / 3",
    width: "100%",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: tokens.colorNeutralBackground3,
    color: tokens.colorNeutralForeground3,
    fontSize: "48px",
    fontWeight: tokens.fontWeightSemibold,
  },
  // Hueco "logo" de CardPreview: por defecto 32×32 abajo; aquí, tamaño natural arriba
  logo: { width: "auto", height: "auto", bottom: "auto", top: "8px", left: "8px" },
  badges: { display: "flex", gap: tokens.spacingHorizontalXS, flexWrap: "wrap" },
  footer: { display: "flex", justifyContent: "space-between", alignItems: "center" },
});

function formatSize(bytes: number) {
  return bytes >= 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : `${Math.round(bytes / 1e6)} MB`;
}

type Props = {
  /** Fuentes activas: al cambiar en Ajustes se vuelve a leer. */
  sources: Site[];
  jobs: Job[];
  onOpen: (r: SearchResult) => void;
  /** Anime sin enlace guardado (bajado con el script): se busca por nombre. */
  onSearch: (source: Site, query: string) => void;
};

export default function LibraryPage({ sources, jobs, onOpen, onSearch }: Props) {
  const styles = useStyles();
  const [list, setList] = useState<LibraryAnime[] | null>(null);
  const [filter, setFilter] = useState("");

  const reload = useCallback(() => {
    api.scanLibrary().then(setList).catch(() => setList([]));
  }, []);
  const sourcesKey = sources.join(",");
  useEffect(reload, [reload, sourcesKey]);

  // Portadas y enlaces que faltan: se piden de uno en uno en segundo plano y quedan
  // guardados en el índice. Cada anime se intenta una vez por sesión.
  const tried = useRef(new Set<string>());
  useEffect(() => {
    if (!list) return;
    const pending = list.filter((a) => !a.image && !tried.current.has(`${a.source}/${a.name}`));
    if (pending.length === 0) return;
    let cancelled = false;
    (async () => {
      for (const a of pending) {
        if (cancelled) return;
        const k = `${a.source}/${a.name}`;
        tried.current.add(k);
        const found = await api.resolveLibraryAnime(a.source, a.name).catch(() => null);
        if (found && !cancelled)
          setList((prev) =>
            prev?.map((x) =>
              `${x.source}/${x.name}` === k ? { ...x, url: found.url, title: found.title, image: found.image } : x,
            ) ?? prev,
          );
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [list]);

  // Al terminar una descarga cambia lo que hay en disco
  const finished = jobs.filter((j) => j.status.state === "done").length;
  useEffect(() => {
    if (finished > 0) reload();
  }, [finished, reload]);

  const shown = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return (list ?? []).filter((a) => !q || a.name.toLowerCase().includes(q) || a.title?.toLowerCase().includes(q));
  }, [list, filter]);

  if (!list) return <Loading label="Leyendo la biblioteca…" />;

  const total = list.reduce((n, a) => n + a.bytes, 0);
  const activeFor = (a: LibraryAnime) =>
    jobs.filter((j) => j.anime === a.name && !isFinished(j) && j.status.state !== "paused").length;

  return (
    <>
      <div className={styles.bar}>
        <Input
          contentBefore={<FilterRegular />}
          placeholder="Filtrar por nombre…"
          value={filter}
          onChange={(_, d) => setFilter(d.value)}
        />
        <Tooltip content="Volver a leer las carpetas" relationship="label">
          <Button icon={<ArrowClockwiseRegular />} appearance="subtle" onClick={reload} />
        </Tooltip>
        <Text className={styles.summary}>
          {list.length} anime{list.length === 1 ? "" : "s"} · {formatSize(total)}
        </Text>
      </div>

      {list.length === 0 && (
        <Text>
          Todavía no hay nada en la biblioteca. Lo que descargues aparecerá aquí, igual que lo que ya tengas en la
          carpeta de cada fuente (Ajustes).
        </Text>
      )}

      <div className={styles.grid}>
        {shown.map((a) => {
          const active = activeFor(a);
          const open = () =>
            a.url
              ? onOpen({ source: a.source, title: a.title ?? a.name, url: a.url, image: a.image, kind: null, status: null })
              : onSearch(a.source, a.name.replace(/\s*\(\d{4}\)$/, ""));
          return (
            <Card key={`${a.source}/${a.name}`} className={styles.card} onClick={open}>
              <CardPreview logo={{ className: styles.logo, children: <SourceLogo site={a.source} height={12} /> }}>
                {a.image ? (
                  <img className={styles.poster} src={a.image} alt="" loading="lazy" />
                ) : (
                  <div className={styles.noPoster}>{a.name.charAt(0).toUpperCase()}</div>
                )}
              </CardPreview>
              <CardHeader
                header={<Text weight="semibold">{a.name}</Text>}
                description={
                  <div className={styles.badges}>
                    <Badge appearance="tint">
                      {a.episodes} ep.{a.seasons.length > 1 ? ` · ${a.seasons.length} temporadas` : ""}
                    </Badge>
                    {active > 0 && <Badge color="brand">descargando {active}</Badge>}
                    {a.partial > 0 && (
                      <Tooltip
                        content="Episodios con restos de una descarga cortada. Ábrelo y descárgalos de nuevo para completarlos."
                        relationship="description"
                      >
                        <Badge color="warning">{a.partial} a medias</Badge>
                      </Tooltip>
                    )}
                  </div>
                }
              />
              <div className={styles.footer}>
                <Text size={200}>{formatSize(a.bytes)}</Text>
                <div>
                  {!a.url && (
                    <Tooltip content="Sin enlace guardado: buscar por nombre" relationship="label">
                      <Button icon={<SearchRegular />} appearance="subtle" size="small" onClick={open} />
                    </Tooltip>
                  )}
                  <Tooltip content="Abrir la carpeta" relationship="label">
                    <Button
                      icon={<FolderOpenRegular />}
                      appearance="subtle"
                      size="small"
                      onClick={(e) => {
                        e.stopPropagation();
                        revealItemInDir(a.path);
                      }}
                    />
                  </Tooltip>
                </div>
              </div>
            </Card>
          );
        })}
      </div>
    </>
  );
}
