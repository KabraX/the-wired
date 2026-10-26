import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Button, MessageBar, MessageBarActions, MessageBarBody, MessageBarTitle, ProgressBar } from "@fluentui/react-components";
import { ArrowDownloadRegular } from "@fluentui/react-icons";
import { api, toolsMissing, type ToolsProgress, type ToolsStatus } from "./api";

type Props = { status: ToolsStatus | null; onChange: (s: ToolsStatus) => void };

/** Aviso para descargar yt-dlp (y ffmpeg en Windows) si faltan. */
export default function ToolsBanner({ status, onChange }: Props) {
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<ToolsProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const un = listen<ToolsProgress>("tools-progress", ({ payload }) => setProgress(payload));
    return () => {
      un.then((f) => f());
    };
  }, []);

  if (!status || !toolsMissing(status)) return null;
  const names = [
    status.ytdlp.source === "missing" && "yt-dlp",
    status.can_install_ffmpeg && status.ffmpeg.source === "missing" && "ffmpeg",
  ].filter(Boolean);

  const mb = (n: number) => `${(n / 1048576).toFixed(0)} MB`;
  return (
    <MessageBar intent={error ? "error" : "warning"} layout="multiline">
      <MessageBarBody>
        <MessageBarTitle>Faltan herramientas para descargar</MessageBarTitle>
        {error ??
          (busy && progress
            ? `Descargando ${progress.tool}… ${mb(progress.downloaded)}${progress.total ? ` de ${mb(progress.total)}` : ""}`
            : `the-wired necesita ${names.join(" y ")}. Se descargan una sola vez desde sus repositorios oficiales de GitHub.`)}
        {busy && (
          <ProgressBar
            value={progress?.total ? progress.downloaded / progress.total : undefined}
            style={{ marginTop: 8 }}
          />
        )}
      </MessageBarBody>
      <MessageBarActions>
        <Button
          appearance="primary"
          icon={<ArrowDownloadRegular />}
          disabled={busy}
          onClick={async () => {
            setBusy(true);
            setError(null);
            try {
              onChange(await api.installTools());
            } catch (e) {
              setError(String(e));
            } finally {
              setBusy(false);
              setProgress(null);
            }
          }}
        >
          Descargar
        </Button>
      </MessageBarActions>
    </MessageBar>
  );
}
