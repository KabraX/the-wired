import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Badge,
  Button,
  Checkbox,
  Dropdown,
  Field,
  MessageBar,
  MessageBarBody,
  Option,
  Radio,
  RadioGroup,
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
import {
  api,
  errorMessage,
  isFinished,
  type Job,
  type SearchResult,
  type YtFormat,
  type YtInfo,
  type YtItem,
} from "../api";
import Loading from "../Loading";

const useStyles = makeStyles({
  header: { display: "flex", gap: tokens.spacingHorizontalXL, alignItems: "flex-start", flexWrap: "wrap" },
  thumb: {
    width: "clamp(220px, 32vw, 360px)",
    aspectRatio: "16 / 9",
    objectFit: "cover",
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground3,
  },
  meta: { display: "flex", flexDirection: "column", gap: tokens.spacingVerticalS, minWidth: 0, flex: 1 },
  row: { display: "flex", alignItems: "center", gap: tokens.spacingHorizontalXS },
  actions: { display: "flex", gap: tokens.spacingHorizontalS, flexWrap: "wrap", alignItems: "end" },
  list: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(260px, 1fr))",
    gap: tokens.spacingVerticalXS,
  },
  done: {
    color: tokens.colorPaletteGreenForeground1,
    display: "flex",
    alignItems: "center",
    gap: tokens.spacingHorizontalXS,
  },
  hint: { color: tokens.colorNeutralForeground3, maxWidth: "70ch" },
});

const QUALITIES: { label: string; value: number | null }[] = [
  { label: "La mejor disponible", value: null },
  { label: "1080p", value: 1080 },
  { label: "720p", value: 720 },
  { label: "480p", value: 480 },
  { label: "360p", value: 360 },
];

function formatDuration(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}` : `${m}:${String(s).padStart(2, "0")}`;
}

type Props = { result: SearchResult; jobs: Job[]; onBack: () => void; onQueued: () => void };

export default function YoutubePage({ result, jobs, onBack, onQueued }: Props) {
  const styles = useStyles();
  const [info, setInfo] = useState<YtInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [media, setMedia] = useState<"video" | "audio">("video");
  const [codec, setCodec] = useState<"mp3" | "m4a">("mp3");
  const [height, setHeight] = useState<number | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  // URL del vídeo → ruta del archivo ya descargado
  const [onDisk, setOnDisk] = useState<Map<string, string>>(new Map());

  useEffect(() => {
    let cancelled = false;
    setInfo(null);
    setError(null);
    setSelected(new Set());
    api
      .youtubeInfo(result.url)
      .then((i) => {
        if (!cancelled) setInfo(i);
      })
      .catch((e) => {
        if (!cancelled) setError(errorMessage(e, "youtube"));
      });
    return () => {
      cancelled = true;
    };
  }, [result.url]);

  const format: YtFormat = useMemo(
    () => (media === "video" ? { kind: "video", max_height: height } : { kind: "audio", codec }),
    [media, height, codec],
  );
  const folder = info?.is_playlist ? info.title : null;
  const items: YtItem[] = useMemo(
    () =>
      (info?.entries ?? []).map((e) => ({
        url: e.url,
        title: e.title,
        index: info?.is_playlist ? e.index : null,
      })),
    [info],
  );

  // Al terminar una descarga cambia lo que hay en disco
  const finished = jobs.filter((j) => j.status.state === "done").length;
  const refreshDisk = useCallback(() => {
    if (!info) return;
    api
      .youtubeItemsOnDisk({ folder, format, items })
      .then((found) => setOnDisk(new Map(found.map((f) => [f.url, f.path]))))
      .catch(() => setOnDisk(new Map()));
  }, [info, folder, format, items]);
  useEffect(refreshDisk, [refreshDisk, finished]);

  if (error)
    return (
      <>
        <Button icon={<ArrowLeftRegular />} appearance="subtle" onClick={onBack}>
          Volver
        </Button>
        <MessageBar intent="error">
          <MessageBarBody>{error}</MessageBarBody>
        </MessageBar>
      </>
    );
  if (!info) return <Loading label={`Cargando ${result.title}…`} />;

  const inQueue = new Set(
    jobs
      .filter(
        (j) =>
          j.yt_format != null &&
          j.video_url != null &&
          items.some((i) => i.url === j.video_url) &&
          !isFinished(j),
      )
      .map((j) => j.video_url as string),
  );
  const pending = items.filter((i) => !onDisk.has(i.url) && !inQueue.has(i.url));
  const selectedPending = pending.filter((i) => selected.has(i.url));
  const extension = format.kind === "video" ? "mp4" : format.codec;
  const single = items[0];
  const singleDone = single ? onDisk.has(single.url) : false;
  const singleQueued = single ? inQueue.has(single.url) : false;

  async function download(chosen: YtItem[]) {
    if (!info || chosen.length === 0) return;
    await api.enqueueYoutubeDownloads({
      folder,
      url: info.url,
      image: info.image,
      format,
      items: chosen,
    });
    setSelected(new Set());
    onQueued();
  }

  const toggle = (url: string, on: boolean) => {
    const next = new Set(selected);
    if (on) next.add(url);
    else next.delete(url);
    setSelected(next);
  };

  return (
    <>
      <Button icon={<ArrowLeftRegular />} appearance="subtle" onClick={onBack} style={{ alignSelf: "flex-start" }}>
        Volver
      </Button>

      <div className={styles.header}>
        {info.image && <img className={styles.thumb} src={info.image} alt="" />}
        <div className={styles.meta}>
          <Title2>{info.title}</Title2>
          <Text>
            <Badge appearance="tint">{info.is_playlist ? "Playlist" : "Vídeo"}</Badge>{" "}
            {info.channel ?? ""}
            {info.is_playlist
              ? ` · ${info.entries.length} vídeo${info.entries.length === 1 ? "" : "s"}`
              : info.duration != null
                ? ` · ${formatDuration(info.duration)}`
                : ""}
          </Text>

          <div className={styles.actions}>
            <Field label="Formato">
              <RadioGroup
                layout="horizontal"
                value={media}
                onChange={(_, d) => setMedia(d.value as "video" | "audio")}
              >
                <Radio value="video" label="Vídeo (MP4)" />
                <Radio value="audio" label="Solo audio" />
              </RadioGroup>
            </Field>
            {media === "video" ? (
              <Field label="Calidad">
                <Dropdown
                  value={QUALITIES.find((q) => q.value === height)?.label ?? "La mejor disponible"}
                  selectedOptions={[height == null ? "best" : String(height)]}
                  onOptionSelect={(_, d) =>
                    setHeight(d.optionValue === "best" ? null : Number(d.optionValue))
                  }
                >
                  {QUALITIES.map((q) => (
                    <Option key={q.label} value={q.value == null ? "best" : String(q.value)} text={q.label}>
                      {q.label}
                    </Option>
                  ))}
                </Dropdown>
              </Field>
            ) : (
              <Field label="Códec">
                <RadioGroup
                  layout="horizontal"
                  value={codec}
                  onChange={(_, d) => setCodec(d.value as "mp3" | "m4a")}
                >
                  <Radio value="mp3" label="MP3" />
                  <Radio value="m4a" label="M4A" />
                </RadioGroup>
              </Field>
            )}
          </div>

          <Text size={200} className={styles.hint}>
            Se guarda en la carpeta de YouTube como{" "}
            <strong>
              {folder ? `${folder}/` : ""}
              {folder && single?.index != null ? `${String(single.index).padStart(2, "0")} - ` : ""}
              {single?.title ?? "…"}.{extension}
            </strong>
            {info.is_playlist ? " (una subcarpeta por playlist)." : "."}
          </Text>

          <div className={styles.actions}>
            <Button
              appearance="primary"
              icon={<ArrowDownloadRegular />}
              disabled={pending.length === 0}
              onClick={() => download(info.is_playlist ? pending : pending.slice(0, 1))}
            >
              {info.is_playlist
                ? `Descargar todo (${pending.length})`
                : singleDone
                  ? "Ya descargado"
                  : singleQueued
                    ? "En cola"
                    : "Descargar"}
            </Button>
            {info.is_playlist && (
              <Button
                icon={<ArrowDownloadRegular />}
                disabled={selectedPending.length === 0}
                onClick={() => download(selectedPending)}
              >
                Descargar seleccionados ({selectedPending.length})
              </Button>
            )}
            {onDisk.size > 0 && (
              <Button
                icon={<FolderOpenRegular />}
                appearance="subtle"
                onClick={() => revealItemInDir([...onDisk.values()][0])}
              >
                Abrir carpeta
              </Button>
            )}
          </div>
        </div>
      </div>

      {info.is_playlist && info.entries.length === 0 && (
        <MessageBar intent="warning">
          <MessageBarBody>La lista no tiene vídeos disponibles.</MessageBarBody>
        </MessageBar>
      )}

      {info.is_playlist && (
        <div className={styles.list}>
          {info.entries.map((e) => {
            const queued = inQueue.has(e.url);
            return (
              <div key={e.url} className={styles.row}>
                {onDisk.has(e.url) ? (
                  <Text className={styles.done}>
                    <CheckmarkCircleRegular /> {e.index}. {e.title}
                  </Text>
                ) : (
                  <Checkbox
                    label={`${e.index}. ${e.title}${e.duration != null ? ` · ${formatDuration(e.duration)}` : ""}${
                      queued ? " · en cola" : ""
                    }`}
                    checked={selected.has(e.url)}
                    disabled={queued}
                    onChange={(_, d) => toggle(e.url, !!d.checked)}
                  />
                )}
              </div>
            );
          })}
        </div>
      )}
    </>
  );
}
