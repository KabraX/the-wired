import { useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { Button, MessageBar, MessageBarActions, MessageBarBody, MessageBarTitle, ProgressBar } from "@fluentui/react-components";
import { ArrowSyncRegular } from "@fluentui/react-icons";

/** Comprueba al arrancar si hay una versión nueva publicada en GitHub Releases. */
export default function UpdateBanner() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<{ done: number; total?: number } | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // Sin conexión o en desarrollo no hay nada que mostrar: se ignora el error
    check().then(setUpdate).catch(() => setUpdate(null));
  }, []);

  if (!update) return null;

  return (
    <MessageBar intent={error ? "error" : "info"} layout="multiline">
      <MessageBarBody>
        <MessageBarTitle>the-wired {update.version} disponible</MessageBarTitle>
        {error ?? (busy ? "Descargando la actualización…" : `Tienes la ${update.currentVersion}.`)}
        {busy && (
          <ProgressBar
            value={progress?.total ? progress.done / progress.total : undefined}
            style={{ marginTop: 8 }}
          />
        )}
      </MessageBarBody>
      <MessageBarActions>
        <Button
          appearance="primary"
          icon={<ArrowSyncRegular />}
          disabled={busy}
          onClick={async () => {
            setBusy(true);
            setError(null);
            let done = 0;
            try {
              await update.downloadAndInstall((e) => {
                if (e.event === "Started") setProgress({ done: 0, total: e.data.contentLength });
                if (e.event === "Progress") {
                  done += e.data.chunkLength;
                  setProgress((p) => ({ done, total: p?.total }));
                }
              });
              await relaunch();
            } catch (e) {
              setError(`No se pudo actualizar: ${e}`);
              setBusy(false);
            }
          }}
        >
          Actualizar y reiniciar
        </Button>
      </MessageBarActions>
    </MessageBar>
  );
}
