import { useEffect, useState } from "react";
import {
  Button,
  Caption1,
  Card,
  MessageBar,
  MessageBarBody,
  ProgressBar,
  Text,
  Tooltip,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import {
  ArrowClockwiseRegular,
  DeleteRegular,
  DismissCircleRegular,
  DismissRegular,
  FolderOpenRegular,
  PauseRegular,
  PlayRegular,
} from "@fluentui/react-icons";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, formatEta, formatSpeed, isFinished, type Job } from "../api";

const useStyles = makeStyles({
  toolbar: { display: "flex", gap: tokens.spacingHorizontalS, alignItems: "center", flexWrap: "wrap" },
  summary: { marginRight: "auto" },
  list: { display: "flex", flexDirection: "column", gap: tokens.spacingVerticalS },
  card: {
    display: "grid",
    gridTemplateColumns: "1fr auto",
    gap: tokens.spacingVerticalXS,
    padding: tokens.spacingVerticalM,
  },
  actions: { display: "flex", alignItems: "center" },
  error: { color: tokens.colorPaletteRedForeground1 },
});

const MAX_RETRIES = 3;

function statusText(j: Job, now: number): string {
  const s = j.status;
  switch (s.state) {
    case "queued":
      return "En cola";
    case "resolving":
      return "Buscando el vídeo en jkanime…";
    case "downloading":
      return [
        s.percent != null ? `${s.percent.toFixed(1)} %` : "Descargando",
        formatSpeed(s.speed),
        s.eta != null ? `quedan ${formatEta(s.eta)}` : "",
      ]
        .filter(Boolean)
        .join(" · ");
    case "paused":
      return "En pausa";
    case "waiting": {
      const left = Math.max(0, s.retry_at - now);
      return `Reintento ${s.attempt} de ${MAX_RETRIES} en ${formatEta(left)} — ${s.message}`;
    }
    case "done":
      return "Descargado";
    case "skipped":
      return "Ya estaba en la biblioteca";
    case "failed":
      return `Error: ${s.message}`;
    case "cancelled":
      return "Cancelado";
  }
}

/** Segundos Unix actuales, actualizados cada segundo mientras haya reintentos en espera. */
function useNow(active: boolean) {
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));
  useEffect(() => {
    if (!active) return;
    const t = setInterval(() => setNow(Math.floor(Date.now() / 1000)), 1000);
    return () => clearInterval(t);
  }, [active]);
  return now;
}

type Props = { jobs: Job[]; onChanged: () => void };

export default function DownloadsPage({ jobs, onChanged }: Props) {
  const styles = useStyles();
  const now = useNow(jobs.some((j) => j.status.state === "waiting"));
  const count = (st: string) => jobs.filter((j) => j.status.state === st).length;
  const running = jobs.filter((j) => !isFinished(j) && j.status.state !== "paused");
  const queued = jobs.filter((j) => j.status.state === "queued");

  if (jobs.length === 0)
    return <Text>No hay descargas. Busca un anime y elige episodios o una temporada.</Text>;

  return (
    <>
      <div className={styles.toolbar}>
        <Text className={styles.summary}>
          {running.length > 0 ? `${running.length} en curso · ` : ""}
          {count("paused") > 0 ? `${count("paused")} en pausa · ` : ""}
          Descargados: {count("done")} · Ya existentes: {count("skipped")} · Fallidos: {count("failed")}
        </Text>
        <Button icon={<PauseRegular />} appearance="subtle" disabled={running.length === 0} onClick={() => api.pauseAll()}>
          Pausar todo
        </Button>
        <Button icon={<PlayRegular />} appearance="subtle" disabled={count("paused") === 0} onClick={() => api.resumeAll()}>
          Reanudar todo
        </Button>
        <Button
          icon={<DismissCircleRegular />}
          appearance="subtle"
          disabled={queued.length === 0}
          onClick={() => Promise.all(queued.map((j) => api.cancelDownload(j.id)))}
        >
          Cancelar pendientes ({queued.length})
        </Button>
        <Button
          icon={<DeleteRegular />}
          appearance="subtle"
          disabled={jobs.every((j) => !isFinished(j))}
          onClick={async () => {
            await api.clearFinished();
            onChanged();
          }}
        >
          Limpiar terminados
        </Button>
      </div>

      {count("failed") + count("waiting") > 1 && (
        <MessageBar intent="warning">
          <MessageBarBody>
            Varios episodios están fallando: puede que jkanime esté caído. Los fallos temporales se reintentan solos.
          </MessageBarBody>
        </MessageBar>
      )}

      <div className={styles.list}>
        {jobs.map((j) => {
          const s = j.status;
          // Sin valor, la barra de Fluent se anima ("trabajando"): solo para lo que está en marcha
          const value =
            s.state === "downloading" ? (s.percent != null ? s.percent / 100 : undefined) :
            s.state === "resolving" ? undefined :
            s.state === "done" || s.state === "skipped" || s.state === "failed" ? 1 : 0;
          const color =
            s.state === "failed" ? "error" :
            s.state === "waiting" || s.state === "paused" ? "warning" :
            s.state === "done" || s.state === "skipped" ? "success" : "brand";
          return (
            <Card key={j.id} className={styles.card}>
              <div>
                <Text weight="semibold">
                  {j.anime} — {j.is_movie ? "Película" : `T${j.season} E${j.episode}`}
                </Text>
                <ProgressBar value={value} thickness="large" color={color} />
                <Caption1 className={s.state === "failed" ? styles.error : undefined}>{statusText(j, now)}</Caption1>
              </div>
              <div className={styles.actions}>
                {(s.state === "done" || s.state === "skipped") && (
                  <Tooltip content="Mostrar en la carpeta" relationship="label">
                    <Button icon={<FolderOpenRegular />} appearance="subtle" onClick={() => revealItemInDir(j.output)} />
                  </Tooltip>
                )}
                {(s.state === "failed" || s.state === "cancelled") && (
                  <Tooltip content="Reintentar" relationship="label">
                    <Button icon={<ArrowClockwiseRegular />} appearance="subtle" onClick={() => api.resumeDownload(j.id)} />
                  </Tooltip>
                )}
                {s.state === "paused" && (
                  <Tooltip content="Reanudar" relationship="label">
                    <Button icon={<PlayRegular />} appearance="subtle" onClick={() => api.resumeDownload(j.id)} />
                  </Tooltip>
                )}
                {!isFinished(j) && s.state !== "paused" && (
                  <Tooltip content="Pausar" relationship="label">
                    <Button icon={<PauseRegular />} appearance="subtle" onClick={() => api.pauseDownload(j.id)} />
                  </Tooltip>
                )}
                {!isFinished(j) && (
                  <Tooltip content="Cancelar" relationship="label">
                    <Button icon={<DismissRegular />} appearance="subtle" onClick={() => api.cancelDownload(j.id)} />
                  </Tooltip>
                )}
              </div>
            </Card>
          );
        })}
      </div>
    </>
  );
}
