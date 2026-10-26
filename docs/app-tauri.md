# the-wired app — diseño de la versión de escritorio (Tauri)

Estado: **aprobado** (2026-10-25). Publicada la 1.0.0 (fases 0–3). En desarrollo la 1.1.0: cola
persistente con pausa/reanudar, reintentos automáticos (1, 5 y 15 min) y notificaciones + icono de bandeja.

## Objetivo

Publicar en GitHub Releases una **app de escritorio** para Windows y Linux (macOS opcional)
que haga lo mismo que el script: buscar anime en jkanime, ver la lista de episodios,
reproducir y descargar episodios o temporadas a una biblioteca de Jellyfin.

El modelo es Parabolic: un núcleo con la lógica, una interfaz encima, y binarios
generados por GitHub Actions al crear una etiqueta.

## Lo que NO cambia

- El **script de bash** sigue siendo la forma principal de uso en terminal:
  `git clone` + `./install.sh` + `the-wired`, `the-wired -d`, `the-wired -c`.
- No se compila el bash ni se reemplaza. La app es un **segundo producto** en el mismo repo.
- Su autoactualización por `VERSION` y su `install.sh` siguen igual.

## Por qué la app no puede llamar al script

| Opción | Problema |
|---|---|
| Tauri ejecuta `the-wired` por debajo | En Windows exige Git Bash, wget, grep con `-P`, jq, fzf y python. Deja de ser un ejecutable nativo. Además el script está hecho para una persona (menús de fzf, preguntas con `read`), no para que otro programa lo controle |
| **Reescribir la lógica en Rust dentro de la app** (elegida) | Hay que traducir el scraper. A cambio, la app funciona sola, sin bash ni herramientas externas salvo las que ella misma incluye |

Consecuencia: habrá **dos implementaciones del scraper** (bash y Rust). Ver
"Reglas compartidas" para reducir el coste de mantener las dos.

## Estructura del repo

```
the-wired/
├── the-wired              script de bash (sin cambios)
├── install.sh / uninstall.sh
├── excepciones.json
├── scraper-rules.json     NUEVO: patrones y URLs que comparten bash y app (fase 3)
├── docs/app-tauri.md      este documento
└── app/                   NUEVO
    ├── package.json       interfaz: React + TypeScript + Vite (pnpm)
    ├── src/               pantallas y componentes
    └── src-tauri/
        ├── Cargo.toml
        ├── tauri.conf.json
        ├── binaries/      yt-dlp y ffmpeg por plataforma (descargados en CI, no versionados)
        └── src/
            ├── main.rs        arranque de Tauri y registro de comandos
            ├── commands.rs    funciones que llama la interfaz
            ├── jkanime/       el scraper (módulo aislado)
            │   ├── search.rs      búsqueda y paginación
            │   ├── anime.rs       ficha: tipo, nº de episodios, API de episodios
            │   ├── episode.rs     cadena de fuentes de vídeo
            ├── download.rs    cola de descargas con yt-dlp
            ├── history.rs     historial compartido con el script
            ├── library.rs     rutas y nombres estilo Jellyfin
            └── settings.rs    ajustes de la app
```

## Correspondencia script → módulos de Rust

| En el script (bash) | En la app (Rust) | Notas |
|---|---|---|
| Búsqueda: `wget -p .../buscar/<query>/` + `grep '<h5><a ...>'` | `jkanime::search` | `reqwest` + parser HTML (`scraper`) en vez de regex sobre archivos. Paginación `.../buscar/<q>/<n>/` |
| `excepciones.json` remoto (Zhuchii) | *No hace falta* | El script adivina la URL a partir del título y corrige los fallos con esa lista. La app usa el enlace real de cada resultado de búsqueda |
| Ficha: token CSRF + `ajax/episodes/<id>` → `total` | `jkanime::anime` | Cookie jar de `reqwest`; POST con `X-CSRF-TOKEN`. La API pagina de 16 en 16 con la página **en la ruta** (`.../ajax/episodes/<id>/<página>`; `?p=` se ignora). Tipo desde `<span>Tipo:</span>` |
| `scrape_episode`: jkplayer → jk.php → `var servers` (Mediafire) | `jkanime::episode` | Misma cadena y mismo orden. La URL `atob(...)` se decodifica en base64 |
| `download_episode` / `download_season` con `yt-dlp --merge-output-format mp4` | `download` | yt-dlp incluido como *sidecar*; progreso leído de su salida y enviado a la interfaz por eventos; cola con N descargas simultáneas |
| `ffprobe` para saltar episodios ya descargados | `download` | ffprobe incluido junto a ffmpeg |
| `sanitize_filename`, `Season 01`, `Nombre - S01E01.mp4` | `library` | Mismas reglas, para que el script y la app produzcan rutas idénticas |
| `~/the-wired/history.json` | `history` | **Mismo archivo y formato**: lo visto en la terminal aparece en la app y al revés |
| `ANIME_DOWNLOAD_DIR`, por defecto `<Vídeos>/Anime` | `settings` | La app usa la variable si existe; si no, su propio ajuste, con el mismo valor por defecto |
| Reproducción con `mpv` | Ver "Reproducción" | |

## Pantallas

1. **Buscar**: caja de búsqueda y resultados en cuadrícula, con póster si la página lo trae y paginación.
2. **Anime**: póster, tipo, número de episodios, último visto (del historial compartido) y lista de episodios con botones *Reproducir* / *Descargar*, más *Descargar temporada*.
3. **Descargas**: cola con progreso, velocidad, tiempo restante, pausar/cancelar, y el resumen final que ya da el script (descargados / ya existentes / fallidos).
4. **Ajustes**: carpeta de descargas, descargas simultáneas, reproductor (mpv externo o integrado), idioma y comprobación de actualizaciones.

Mensajes de error claros con el mismo criterio que el script v1.5:
"jkanime no responde" frente a "la web cambió, actualiza la app".

## Reproducción

Los episodios son streams HLS (`.m3u8`):

- **Reproductor integrado** (por defecto): el webview de Tauri reproduce HLS con `hls.js`.
  No requiere nada instalado. Guarda la posición en el historial compartido.
- **mpv externo** (opcional, en Ajustes): si el usuario tiene mpv, la app lo lanza como el
  script, con mejor calidad de escalado y sus propios atajos.

Pendiente de verificar: si los servidores de vídeo exigen cabeceras (`Referer`) que el
reproductor integrado no pueda enviar. Si es así, la app haría de proxy local del stream.

## Empaquetado y dependencias incluidas

| Plataforma | Formatos (Tauri bundler) | Incluye |
|---|---|---|
| Windows | `.msi` y `.exe` (NSIS) | yt-dlp.exe, ffmpeg.exe, ffprobe.exe |
| Linux | AppImage, `.deb`, `.rpm` | yt-dlp; ffmpeg del sistema en `.deb`/`.rpm` (dependencia), incluido en AppImage |

- yt-dlp (Unlicense) y ffmpeg (builds LGPL) se descargan en CI y no se versionan en git.
- yt-dlp cambia a menudo para seguir a las webs: la app lo actualiza sola (`yt-dlp -U`
  sobre su copia) o al menos avisa.

## Publicación (GitHub Actions)

Workflow `release-app.yml`, al crear una etiqueta `app-vX.Y.Z`:

1. Matriz `windows-latest` y `ubuntu-22.04`.
2. Descarga de yt-dlp y ffmpeg por plataforma en `src-tauri/binaries/`.
3. `tauri-apps/tauri-action` compila y adjunta los instaladores a una Release.
4. Genera el `latest.json` del **updater de Tauri**, firmado con una clave guardada en
   los *secrets* del repo, para que la app se actualice sola desde Releases.

Etiquetas separadas: `vX.Y.Z` para el script y `app-vX.Y.Z` para la app, para que sus
versiones evolucionen por separado.

## Reglas compartidas (fase 3)

Lo que más cambia cuando jkanime modifica su web son los **patrones**: el selector del
título en la búsqueda, la URL de la API de episodios, el marcador de jkplayer, etc.
Moverlos a `scraper-rules.json`, publicado con GitHub Pages como `excepciones.json`,
permite arreglar ambos productos cambiando un archivo, sin publicar versión.
Primero se hace la app con los patrones en código; esto se añade cuando funcione.

## Fases

| Fase | Entregable | Criterio de "hecho" |
|---|---|---|
| 0 | Andamiaje Tauri en `app/`, CI que compila en Windows y Linux | Un "hola mundo" instalable desde una Release de prueba |
| 1 | Núcleo Rust: búsqueda, ficha, episodios, fuentes de vídeo | Tests contra páginas HTML guardadas (sin red) y una prueba real con un anime |
| 2 | Interfaz de buscar/anime + **descargas** con yt-dlp incluido, cola y resumen | Descarga una temporada a la estructura de Jellyfin desde la app |
| 3 | Releases con updater, ajustes, pulido | Instalación limpia en un Windows sin nada previo |
| 4 | Reproducción (integrada con hls.js y/o mpv) + historial compartido | Se puede ver un episodio y el historial coincide con el del script |
| 5 | `scraper-rules.json` compartido | Un cambio de patrón arregla bash y app a la vez |

## Riesgos

- **jkanime cambia o cae**: es el riesgo principal del proyecto, igual que hoy. El módulo
  aislado y las reglas compartidas lo mitigan; los tests con HTML guardado avisan antes.
- **Distribución**: Flathub y Microsoft Store probablemente rechacen la app por su función.
  Se distribuye solo por GitHub Releases.
- **Firma**: sin certificado, Windows SmartScreen y macOS Gatekeeper avisarán al instalar.
- **Mantener dos scrapers** (bash y Rust): mitigado en la fase 5.

## Decisiones tomadas

1. Plataformas: **Windows y Linux**. macOS queda fuera por ahora.
2. Interfaz: **Fluent UI React v9** en ambas. Más adelante, opcionalmente, un tema al estilo
   libadwaita en Linux (Parabolic usa GTK4/libadwaita en Linux y WinUI en Windows).
3. Orden: primero núcleo y **descargas**; la reproducción pasa a la fase 4.
4. Nombre: **the-wired**, igual que el script.
