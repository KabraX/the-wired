import {
  Button,
  Caption1,
  Card,
  MessageBar,
  MessageBarBody,
  ProgressBar,
  Text,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { DeleteRegular, DismissCircleRegular, DismissRegular, FolderOpenRegular } from "@fluentui/react-icons";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, formatEta, formatSpeed, isFinished, type Job } from "../api";

const useStyles = makeStyles({
  toolbar: { display: "flex", gap: tokens.spacingHorizontalM, alignItems: "center" },
  list: { display: "flex", flexDirection: "column", gap: tokens.spacingVerticalS },
  card: { display: "grid", gridTemplateColumns: "1fr auto", gap: tokens.spacingVerticalXS, padding: tokens.spacingVerticalM },
  error: { color: tokens.colorPaletteRedForeground1 },
});

function statusText(j: Job): string {
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

type Props = { jobs: Job[]; onChanged: () => void };

export default function DownloadsPage({ jobs, onChanged }: Props) {
  const styles = useStyles();
  const count = (st: string) => jobs.filter((j) => j.status.state === st).length;
  const active = jobs.filter((j) => !isFinished(j)).length;
  const queued = jobs.filter((j) => j.status.state === "queued");

  if (jobs.length === 0)
    return <Text>No hay descargas. Busca un anime y elige episodios o una temporada.</Text>;

  return (
    <>
      <div className={styles.toolbar}>
        <Text>
          {active > 0 ? `${active} en curso · ` : ""}
          Descargados: {count("done")} · Ya existentes: {count("skipped")} · Fallidos: {count("failed")}
        </Text>
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

      {count("failed") > 0 && (
        <MessageBar intent="warning">
          <MessageBarBody>
            Si fallan muchos episodios a la vez, puede que jkanime esté caído o haya cambiado su web.
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
          return (
            <Card key={j.id} className={styles.card}>
              <div>
                <Text weight="semibold">
                  {j.anime} — {j.is_movie ? "Película" : `T${j.season} E${j.episode}`}
                </Text>
                <ProgressBar
                  value={value}
                  thickness="large"
                  color={s.state === "failed" ? "error" : s.state === "done" || s.state === "skipped" ? "success" : "brand"}
                />
                <Caption1 className={s.state === "failed" ? styles.error : undefined}>{statusText(j)}</Caption1>
              </div>
              <div>
                {isFinished(j) ? (
                  (s.state === "done" || s.state === "skipped") && (
                    <Button icon={<FolderOpenRegular />} appearance="subtle" onClick={() => revealItemInDir(j.output)} title="Mostrar en la carpeta" />
                  )
                ) : (
                  <Button icon={<DismissRegular />} appearance="subtle" onClick={() => api.cancelDownload(j.id)} title="Cancelar" />
                )}
              </div>
            </Card>
          );
        })}
      </div>
    </>
  );
}
