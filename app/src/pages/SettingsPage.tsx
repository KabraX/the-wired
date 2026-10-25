import { useEffect, useState } from "react";
import {
  Button,
  Field,
  Input,
  MessageBar,
  MessageBarBody,
  SpinButton,
  Text,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { FolderRegular } from "@fluentui/react-icons";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type AppInfo, type Settings } from "../api";

const useStyles = makeStyles({
  form: { display: "flex", flexDirection: "column", gap: tokens.spacingVerticalL, maxWidth: "640px" },
  row: { display: "flex", gap: tokens.spacingHorizontalS },
});

export default function SettingsPage({ info }: { info: AppInfo | null }) {
  const styles = useStyles();
  const [settings, setSettings] = useState<Settings | null>(null);
  const [saved, setSaved] = useState<string | null>(null);

  useEffect(() => {
    api.getSettings().then(setSettings);
  }, []);

  if (!settings) return null;

  async function save(next: Settings, note?: string) {
    setSettings(await api.setSettings(next));
    setSaved(note ?? "Guardado.");
  }

  return (
    <div className={styles.form}>
      <Field label="Carpeta de la biblioteca de anime" hint="Las descargas se guardan como <carpeta>/<Anime>/Season 01/<Anime> - S01E01.mp4, el formato que espera Jellyfin.">
        <div className={styles.row}>
          <Input style={{ flex: 1 }} value={settings.library_dir} readOnly />
          <Button
            icon={<FolderRegular />}
            onClick={async () => {
              const dir = await open({ directory: true, defaultPath: settings.library_dir });
              if (typeof dir === "string") save({ ...settings, library_dir: dir });
            }}
          >
            Cambiar…
          </Button>
        </div>
      </Field>

      <Field label="Descargas a la vez" hint="Se aplica al reiniciar la app.">
        <SpinButton
          value={settings.concurrent_downloads}
          min={1}
          max={6}
          onChange={(_, d) =>
            d.value != null && save({ ...settings, concurrent_downloads: d.value }, "Guardado. Reinicia la app para aplicarlo.")
          }
        />
      </Field>

      {saved && (
        <MessageBar intent="success">
          <MessageBarBody>{saved}</MessageBarBody>
        </MessageBar>
      )}

      <Field label="yt-dlp">
        {info?.ytdlp ? (
          <Text>Versión {info.ytdlp}</Text>
        ) : (
          <MessageBar intent="error">
            <MessageBarBody>No se encontró yt-dlp: sin él no se puede descargar.</MessageBarBody>
          </MessageBar>
        )}
      </Field>
    </div>
  );
}
