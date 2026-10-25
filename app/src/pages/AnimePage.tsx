import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Badge,
  Button,
  Checkbox,
  Field,
  MessageBar,
  MessageBarBody,
  SpinButton,
  Spinner,
  Text,
  Title2,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import {
  ArrowDownloadRegular,
  ArrowLeftRegular,
  CheckmarkCircleRegular,
  FolderOpenRegular,
} from "@fluentui/react-icons";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type AnimeInfo, type Job, type SearchResult } from "../api";

const useStyles = makeStyles({
  header: { display: "flex", gap: tokens.spacingHorizontalXL, alignItems: "flex-start" },
  poster: { width: "160px", aspectRatio: "2 / 3", objectFit: "cover", borderRadius: tokens.borderRadiusMedium },
  meta: { display: "flex", flexDirection: "column", gap: tokens.spacingVerticalS },
  actions: { display: "flex", gap: tokens.spacingHorizontalS, flexWrap: "wrap", alignItems: "end" },
  list: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))",
    gap: tokens.spacingVerticalXS,
  },
  row: { display: "flex", alignItems: "center", gap: tokens.spacingHorizontalXS },
  done: { color: tokens.colorPaletteGreenForeground1 },
});

type Props = { result: SearchResult; jobs: Job[]; onBack: () => void; onQueued: () => void };

export default function AnimePage({ result, jobs, onBack, onQueued }: Props) {
  const styles = useStyles();
  const [info, setInfo] = useState<AnimeInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [season, setSeason] = useState(1);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [onDisk, setOnDisk] = useState<Set<number>>(new Set());

  useEffect(() => {
    setInfo(null);
    setError(null);
    api.anime(result.url).then(setInfo).catch((e) => setError(errorMessage(e)));
  }, [result.url]);

  const numbers = useMemo(() => info?.episodes.map((e) => e.number) ?? [], [info]);

  // Qué episodios ya están en la biblioteca (se recalcula al terminar descargas)
  const finishedCount = jobs.filter((j) => j.anime === info?.title && j.status.state === "done").length;
  const refreshDisk = useCallback(() => {
    if (!info) return;
    api.episodesOnDisk(info.title, season, numbers).then((n) => setOnDisk(new Set(n)));
  }, [info, season, numbers]);
  useEffect(refreshDisk, [refreshDisk, finishedCount]);

  if (error)
    return (
      <>
        <Button icon={<ArrowLeftRegular />} appearance="subtle" onClick={onBack}>Volver</Button>
        <MessageBar intent="error"><MessageBarBody>{error}</MessageBarBody></MessageBar>
      </>
    );
  if (!info) return <Spinner label={`Cargando ${result.title}…`} />;

  const inQueue = new Set(
    jobs
      .filter((j) => j.anime === info.title && j.season === season && !["done", "skipped", "failed", "cancelled"].includes(j.status.state))
      .map((j) => j.episode),
  );
  const pending = numbers.filter((n) => !onDisk.has(n));

  async function download(episodes: number[]) {
    if (!info || episodes.length === 0) return;
    await api.enqueueDownloads({
      anime: info.title,
      animeUrl: info.url,
      isMovie: info.is_movie,
      season,
      episodes,
    });
    setSelected(new Set());
    onQueued();
  }

  const toggle = (n: number, on: boolean) => {
    const s = new Set(selected);
    if (on) s.add(n);
    else s.delete(n);
    setSelected(s);
  };

  return (
    <>
      <Button icon={<ArrowLeftRegular />} appearance="subtle" onClick={onBack} style={{ alignSelf: "flex-start" }}>
        Volver a los resultados
      </Button>

      <div className={styles.header}>
        {info.image && <img className={styles.poster} src={info.image} alt="" />}
        <div className={styles.meta}>
          <Title2>{info.title}</Title2>
          <Text>
            {info.kind && <Badge appearance="tint">{info.kind}</Badge>}{" "}
            {info.episodes.length} episodio{info.episodes.length === 1 ? "" : "s"} · {onDisk.size} en tu biblioteca
          </Text>
          <div className={styles.actions}>
            {!info.is_movie && (
              <Field label="Temporada (para Jellyfin)">
                <SpinButton value={season} min={1} max={99} onChange={(_, d) => setSeason(d.value ?? season)} />
              </Field>
            )}
            <Button
              appearance="primary"
              icon={<ArrowDownloadRegular />}
              disabled={pending.length === 0}
              onClick={() => download(pending)}
            >
              {info.is_movie ? "Descargar película" : `Descargar temporada (${pending.length})`}
            </Button>
            {!info.is_movie && (
              <Button icon={<ArrowDownloadRegular />} disabled={selected.size === 0} onClick={() => download([...selected])}>
                Descargar seleccionados ({selected.size})
              </Button>
            )}
            <Button
              icon={<FolderOpenRegular />}
              appearance="subtle"
              disabled={onDisk.size === 0}
              onClick={async () => revealItemInDir(await api.animeFolder(info.title))}
            >
              Abrir carpeta
            </Button>
          </div>
        </div>
      </div>

      {!info.is_movie && (
        <div className={styles.list}>
          {info.episodes.map((e) => (
            <div key={e.number} className={styles.row}>
              {onDisk.has(e.number) ? (
                <Text className={styles.done}>
                  <CheckmarkCircleRegular /> Episodio {e.number}
                </Text>
              ) : (
                <Checkbox
                  label={`Episodio ${e.number}${inQueue.has(e.number) ? " · en cola" : ""}`}
                  checked={selected.has(e.number)}
                  disabled={inQueue.has(e.number)}
                  onChange={(_, d) => toggle(e.number, !!d.checked)}
                />
              )}
            </div>
          ))}
        </div>
      )}
    </>
  );
}
