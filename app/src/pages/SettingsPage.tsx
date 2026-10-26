import { useEffect, useState } from "react";
import {
  Button,
  Field,
  Input,
  Radio,
  RadioGroup,
  Subtitle2,
  Tooltip,
  MessageBar,
  MessageBarBody,
  SpinButton,
  Spinner,
  Text,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { ArrowSyncRegular, FolderRegular } from "@fluentui/react-icons";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type Settings, type ToolSource, type ToolsStatus } from "../api";
import { ACCENTS, type ThemeMode, type ThemePrefs } from "../theme";

type UpdateResult = { ok: boolean; text: string };

const useStyles = makeStyles({
  form: { display: "flex", flexDirection: "column", gap: tokens.spacingVerticalL, maxWidth: "640px" },
  row: { display: "flex", gap: tokens.spacingHorizontalS },
  swatches: { display: "flex", flexWrap: "wrap", gap: tokens.spacingHorizontalS },
  swatch: {
    width: "36px",
    height: "36px",
    borderRadius: tokens.borderRadiusCircular,
    border: `2px solid ${tokens.colorNeutralStroke1}`,
    cursor: "pointer",
    padding: 0,
  },
  selected: { outline: `3px solid ${tokens.colorNeutralForeground1}`, outlineOffset: "2px" },
});

// Azul de marca de Fluent, para la muestra del tema por defecto
const FLUENT_BLUE = "#0f6cbd";

const SOURCE: Record<ToolSource, string> = {
  app: "copia de the-wired",
  system: "del sistema",
  missing: "no instalado",
};

type Props = {
  tools: ToolsStatus | null;
  onToolsChanged: () => void;
  themePrefs: ThemePrefs;
  onThemeChange: (p: ThemePrefs) => void;
};

export default function SettingsPage({ tools, onToolsChanged, themePrefs, onThemeChange }: Props) {
  const styles = useStyles();
  const [settings, setSettings] = useState<Settings | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [updating, setUpdating] = useState(false);
  const [update, setUpdate] = useState<UpdateResult | null>(null);

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
      <Subtitle2>Biblioteca y descargas</Subtitle2>
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

      <Subtitle2>Herramientas</Subtitle2>
      <Field label="yt-dlp" hint="Descarga los vídeos. La copia de the-wired se actualiza desde aquí; la del sistema, con su gestor de paquetes.">
        <div className={styles.row} style={{ alignItems: "center" }}>
          <Text>
            {tools?.ytdlp.version ? `Versión ${tools.ytdlp.version}` : "—"} · {tools ? SOURCE[tools.ytdlp.source] : "…"}
          </Text>
          {tools?.ytdlp.source !== "missing" && (
            <Button
              icon={updating ? <Spinner size="tiny" /> : <ArrowSyncRegular />}
              disabled={updating}
              onClick={async () => {
                setUpdating(true);
                setUpdate(null);
                try {
                  setUpdate({ ok: true, text: await api.updateYtdlp() });
                  onToolsChanged();
                } catch (e) {
                  setUpdate({ ok: false, text: String(e) });
                } finally {
                  setUpdating(false);
                }
              }}
            >
              Buscar actualizaciones
            </Button>
          )}
        </div>
      </Field>
      <Field label="ffmpeg" hint="Une los fragmentos del vídeo en un MP4 y comprueba los archivos ya descargados.">
        <Text>
          {tools?.ffmpeg.version ? `Versión ${tools.ffmpeg.version}` : "—"} · {tools ? SOURCE[tools.ffmpeg.source] : "…"}
        </Text>
      </Field>
      {update && (
        <MessageBar intent={update.ok ? "success" : "warning"}>
          <MessageBarBody>{update.text}</MessageBarBody>
        </MessageBar>
      )}

      <Subtitle2>Tema</Subtitle2>
      <Field label="Modo">
        <RadioGroup
          layout="horizontal"
          value={themePrefs.mode}
          onChange={(_, d) => onThemeChange({ ...themePrefs, mode: d.value as ThemeMode })}
        >
          <Radio value="system" label="Sistema" />
          <Radio value="light" label="Claro" />
          <Radio value="dark" label="Oscuro" />
        </RadioGroup>
      </Field>
      <Field label="Color de acento" hint="Colores de la paleta de Fluent. El azul es el de Fluent por defecto.">
        <div className={styles.swatches} role="radiogroup" aria-label="Color de acento">
          {ACCENTS.map((a) => (
            <Tooltip key={a.id} content={a.name} relationship="label">
              <button
                type="button"
                role="radio"
                aria-checked={themePrefs.accent === a.id}
                className={`${styles.swatch} ${themePrefs.accent === a.id ? styles.selected : ""}`}
                style={{ backgroundColor: a.color ?? FLUENT_BLUE }}
                onClick={() => onThemeChange({ ...themePrefs, accent: a.id })}
              />
            </Tooltip>
          ))}
        </div>
      </Field>
    </div>
  );
}
