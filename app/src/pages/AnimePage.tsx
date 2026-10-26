import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Badge,
  Button,
  Checkbox,
  Field,
  Input,
  MessageBar,
  MessageBarBody,
  SpinButton,
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
import { api, errorMessage, siteName, type AnimeInfo, type Job, type SearchResult } from "../api";
import Loading from "../Loading";

const useStyles = makeStyles({
  header: { display: "flex", gap: tokens.spacingHorizontalXL, alignItems: "flex-start" },
  poster: { width: "clamp(160px, 22vw, 240px)", flexShrink: 0, aspectRatio: "2 / 3", objectFit: "cover", borderRadius: tokens.borderRadiusMedium },
  meta: { display: "flex", flexDirection: "column", gap: tokens.spacingVerticalS, minWidth: 0 },
  actions: { display: "flex", gap: tokens.spacingHorizontalS, flexWrap: "wrap", alignItems: "end" },
  list: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))",
    gap: tokens.spacingVerticalXS,
  },
  row: { display: "flex", alignItems: "center", gap: tokens.spacingHorizontalXS },
  done: { color: tokens.colorPaletteGreenForeground1 },
  hint: { color: tokens.colorNeutralForeground3, maxWidth: "60ch" },
});

type Props = { result: SearchResult; jobs: Job[]; onBack: () => void; onQueued: () => void };

export default function AnimePage({ result, jobs, onBack, onQueued }: Props) {
  const styles = useStyles();
  const [info, setInfo] = useState<AnimeInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [jellyfin, setJellyfin] = useState(true);
  // Con formato Jellyfin: serie (carpeta) y temporada en la que se guarda este anime
  const [series, setSeries] = useState("");
  const [season, setSeason] = useState(1);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [onDisk, setOnDisk] = useState<Set<number>>(new Set());
  const [seasonsOnDisk, setSeasonsOnDisk] = useState<number[]>([]);

  useEffect(() => {
    setInfo(null);
    setError(null);
    api.getSettings().then((s) => setJellyfin(s.sources[result.source]?.jellyfin_layout ?? true));
    api
      .anime(result.url)
      .then(async (i) => {
        // Estos sitios publican cada temporada como un anime aparte ("Overlord II", "... Season 2")
        const [name, n] = await api.splitSeason(i.title);
        setSeries(name);
        setSeason(n);
        setInfo(i);
      })
      .catch((e) => setError(errorMessage(e, result.source)));
  }, [result.url, result.source]);

  const numbers = useMemo(() => info?.episodes.map((e) => e.number) ?? [], [info]);
  const useSeasons = jellyfin && !info?.is_movie;
  // Nombre con el que se guarda en la biblioteca y temporada efectiva
  const folder = (useSeasons ? series.trim() : "") || info?.title || "";
  const effSeason = useSeasons ? season : 1;

  // Qué episodios ya están en la biblioteca (se recalcula al terminar descargas)
  const finishedCount = jobs.filter((j) => j.anime === folder && j.status.state === "done").length;
  const refreshDisk = useCallback(() => {
    if (!info || !folder) return;
    api.episodesOnDisk(info.source, folder, info.is_movie, effSeason, numbers).then((n) => setOnDisk(new Set(n)));
    api.seasonsOnDisk(info.source, folder).then(setSeasonsOnDisk);
  }, [info, folder, effSeason, numbers]);
  useEffect(refreshDisk, [refreshDisk, finishedCount]);

  if (error)
    return (
      <>
        <Button icon={<ArrowLeftRegular />} appearance="subtle" onClick={onBack}>Volver</Button>
        <MessageBar intent="error"><MessageBarBody>{error}</MessageBarBody></MessageBar>
      </>
    );
  if (!info) return <Loading label={`Cargando ${result.title}…`} />;

  const inQueue = new Set(
    jobs
      .filter((j) => j.anime === folder && j.season === effSeason && !["done", "skipped", "failed", "cancelled"].includes(j.status.state))
      .map((j) => j.episode),
  );
  const pending = numbers.filter((n) => !onDisk.has(n));

  async function download(episodes: number[]) {
    if (!info || episodes.length === 0) return;
    await api.enqueueDownloads({
      anime: folder,
      animeUrl: info.url,
      title: info.title,
      image: info.image,
      isMovie: info.is_movie,
      season: effSeason,
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
            {info.quality && <Badge appearance="outline">{info.quality}</Badge>}{" "}
            {info.episodes.length} episodio{info.episodes.length === 1 ? "" : "s"} en {siteName(info.source)} ·{" "}
            {onDisk.size === 0 ? "ninguno descargado" : `${onDisk.size} descargado${onDisk.size === 1 ? "" : "s"}`}
            {useSeasons && ` en la temporada ${season}`}
          </Text>
          {useSeasons && (
            <div className={styles.actions}>
              <Field label="Serie en Jellyfin">
                <Input value={series} onChange={(_, d) => setSeries(d.value)} />
              </Field>
              <Field label="Temporada">
                <SpinButton value={season} min={1} max={99} onChange={(_, d) => setSeason(d.value ?? season)} />
              </Field>
            </div>
          )}
          {useSeasons && (
            <Text size={200} className={styles.hint}>
              {siteName(info.source)} publica cada temporada como un anime aparte. Aquí eliges en qué serie y temporada de tu
              biblioteca se guarda este.
            </Text>
          )}
          {useSeasons && onDisk.size === 0 && seasonsOnDisk.some((n) => n !== season) && (
            <MessageBar intent="info">
              <MessageBarBody>
                Ya tienes episodios de «{folder}» en la temporada{" "}
                {seasonsOnDisk.filter((n) => n !== season).map((n, i) => (
                  <span key={n}>
                    {i > 0 && ", "}
                    <Button appearance="transparent" size="small" onClick={() => setSeason(n)}>
                      {n}
                    </Button>
                  </span>
                ))}
              </MessageBarBody>
            </MessageBar>
          )}
          <div className={styles.actions}>
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
              onClick={async () => revealItemInDir(await api.animeFolder(info.source, folder))}
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
