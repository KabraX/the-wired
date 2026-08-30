# the-wired

> Fork de [ani-es](https://github.com/Zhuchii/ani-es) con soporte de descarga integrado para bibliotecas **Jellyfin**.

Reproductor **+ descargador** de anime en español desde la terminal. Manteniendo el scraper original de ani-es intacto, añade la capacidad de descargar episodios y temporadas completas directamente a una estructura de carpetas compatible con Jellyfin.

---

## ⚠️ Aviso importante

**Este fork solo se ha testeado en el siguiente entorno:**

- **OS:** Windows 11
- **Shell:** Git Bash / MINGW64 (NO WSL)
- **Terminal:** Windows Terminal con perfil Git Bash
- **Biblioteca Jellyfin:** `N:/Multimedy/Anime`

No se garantiza funcionamiento en Linux, macOS, WSL u otras configuraciones de Windows. El script *debería* seguir funcionando en Linux (la lógica de reproducción de ani-es no se modificó), pero las funciones de descarga se probaron exclusivamente en Git Bash sobre Windows 11.

---

## ✨ Qué hay de nuevo

ani-es original era solo un reproductor. the-wired añade un sistema de descarga integrado sin reescribir el scraper:

### Descarga de episodios individuales

Tras seleccionar un episodio, en lugar de reproducirlo automáticamente, aparece un menú:

```
Reproducir
Descargar episodio
Descargar temporada
Atrás
```

Seleccionar **Descargar episodio** obtiene una URL fresca del stream (`.m3u8`) y la descarga con `yt-dlp`, generando un MP4 compatible con Jellyfin.

### Descarga de temporadas completas

**Descargar temporada** itera todos los episodios disponibles. Cada episodio obtiene su propia URL fresca justo antes de descargarse — **no se cachean URLs** porque los parámetros `st` y `e` del `.m3u8` son temporales.

Al finalizar muestra un resumen:

```
====================================
      DESCARGA COMPLETADA
====================================

Anime: Elfen Lied
Temporada: 1

Descargados: 11
Ya existentes: 2
Fallidos: 0
Total: 13
```

### Estructura Jellyfin

Las descargas se guardan con el formato que Jellyfin espera:

```
<ANIME_DOWNLOAD_DIR>/
  Elfen Lied/
    Season 01/
      Elfen Lied - S01E01.mp4
      Elfen Lied - S01E02.mp4
      ...
```

La ruta por defecto es `N:/Multimedy/Anime` pero se puede sobrescribir:

```bash
export ANIME_DOWNLOAD_DIR="D:/Otra/Biblioteca/Anime"
```

### Modo descarga (`-d` / `--download`)

```bash
ani-es -d
ani-es --download
```

Oculta la opción "Reproducir" del menú, dejando solo las opciones de descarga. Útil si usas the-wired principalmente como descargador.

### Detección de episodios ya descargados

Antes de descargar un episodio, the-wired comprueba si el archivo MP4 ya existe y es válido (con `ffprobe`). Si es así, lo omite. Si existe un archivo parcial (`.part`), lo redescarga. No se sobrescriben descargas completas.

### Sin recodificación

El servidor entrega vídeo H.264 1920x1080 + audio AAC. `yt-dlp` con `--merge-output-format mp4` solo remuxea (cambia el contenedor) sin recodificar, por lo que la descarga es rápida y sin pérdida de calidad.

---

## 🔧 Qué NO se modificó

Para minimizar el riesgo de romper algo que ya funcionaba:

- **Scraper:** el bloque de scraping de `videocap()` se extrajo a una función `scrape_episode()` pero el código es **idéntico** (mismas 48 líneas, diff vacío). No se tocaron los proveedores ni la lógica de obtención de URLs.
- **Búsqueda de anime:** sin cambios.
- **Selección de anime y episodios:** sin cambios.
- **Reproducción con mpv:** sin cambios. El historial (`history.json`) sigue funcionando igual y las descargas no lo modifican.
- **Comandos existentes:** `-h`, `-v`, `-c` siguen funcionando igual.
- **Auto-actualización:** se mantiene del original.

---

## 📦 Instalación (Windows 11 + Git Bash)

> La guía de instalación sigue el mismo enfoque que [ani-cli](https://github.com/pystardust/ani-cli) para Windows, ya que ambos scripts corren sobre Git Bash / MINGW64.

### 1. Instalar Scoop

Abre **PowerShell** y sigue las instrucciones de [scoop.sh](https://scoop.sh/) (quickstart).

### 2. Instalar Windows Terminal

Viene preinstalado en Windows 11. Si no lo tienes:

```powershell
scoop bucket add extras
scoop install extras/windows-terminal
```

### 3. Instalar Git

Si ya lo tienes, actualízalo. Si no:

```powershell
scoop install git
```

### 4. Configurar perfil Git Bash en Windows Terminal

Asegúrate de que **Git Bash** aparece en el menú de perfiles de Windows Terminal (botón de nueva pestaña).

Si no aparece, añádelo manualmente:

1. Ve a **Settings → Profiles → Add a new profile → + New empty profile**
2. **Name:** `Git Bash`
3. **Command line:**
   - Si instalaste git con scoop: `%GIT_INSTALL_ROOT%\bin\bash.exe -i -l`
   - Si instalaste git por otros medios: `C:\Program Files\Git\bin\bash.exe -i -l`
4. **Icon:** `%GIT_INSTALL_ROOT%\mingw64\share\git\git-for-windows.ico` (o la ruta equivalente)
5. **Starting directory:** `%USERPROFILE%`
6. **Hide profile from dropdown:** Off
7. Guarda los cambios

Puedes establecer este perfil como predeterminado en **Startup** para no tener que cambiarlo cada vez.

Reinicia Windows Terminal.

### 5. Instalar dependencias

En el perfil **Git Bash** de Windows Terminal:

```bash
scoop bucket add extras
scoop install fzf ffmpeg mpv yt-dlp jq wget curl python
```

> `yt-dlp` y `ffmpeg` son **obligatorios** para las descargas. Sin ellos, the-wired mostrará un error al intentar descargar y no continuará.

### 6. Instalar the-wired

```bash
cd ~/
git clone https://github.com/KabraX/the-wired.git
cd the-wired
chmod +x the-wired
```

Para poder ejecutarlo desde cualquier sitio, crea un alias o añade el directorio al PATH. Lo más sencillo es un symlink:

```bash
ln -s "$(pwd)/the-wired" /usr/local/bin/the-wired
```

> Nota: el `install.sh` original de ani-es está pensado para Linux (usa `apt-get`, `dnf`, etc.) y **no funciona en Git Bash**. La instalación manual de arriba es la que se ha testeado en este entorno.

---

## 🖥️ Uso

### Reproducir (comportamiento original de ani-es)

```bash
the-wired
```

O buscando directamente:

```bash
the-wired elfen lied
```

Selecciona el anime → selecciona el episodio → elige **Reproducir**.

### Descargar

```bash
the-wired
```

Selecciona el anime → selecciona el episodio → elige **Descargar episodio** o **Descargar temporada**.

### Modo descarga

```bash
the-wired -d
the-wired --download
```

### Continuar donde lo dejaste

```bash
the-wired -c
```

### Ayuda y versión

```bash
the-wired --help
the-wired --version
```

---

## 🔧 Dependencias

| Comando | Reproducir | Descargar | Cómo instalar (scoop) |
|---------|:----------:|:---------:|------------------------|
| `wget` | ✅ | ✅ | `scoop install wget` |
| `curl` | ✅ | ✅ | `scoop install curl` |
| `jq` | ✅ | ✅ | `scoop install jq` |
| `fzf` | ✅ | ✅ | `scoop install fzf` |
| `grep` | ✅ | ✅ | viene con Git Bash |
| `sed` | ✅ | ✅ | viene con Git Bash |
| `python3` | ✅ | ✅ | `scoop install python` |
| `mpv` | ✅ | — | `scoop install mpv` |
| `yt-dlp` | — | ✅ | `scoop install yt-dlp` |
| `ffmpeg` | — | ✅ | `scoop install ffmpeg` |
| `ffprobe` | — | ✅ | viene con ffmpeg |

---

## ⚙️ Configuración

### Ruta de descarga

Por defecto las descargas van a `N:/Multimedy/Anime`. Sobrescríbela con:

```bash
export ANIME_DOWNLOAD_DIR="D:/Mi/Biblioteca/Anime"
```

Añádelo a tu `~/.bashrc` para que persista entre sesiones.

### Estructura de carpetas Jellyfin

```
<ANIME_DOWNLOAD_DIR>/
  <Anime>/
    Season 01/
      <Anime> - S01E01.mp4
      <Anime> - S01E02.mp4
```

Los nombres de archivo se sanitizan para Windows (se eliminan `< > : " / \ | ? *`). Los caracteres Unicode (acentos, etc.) se preservan.

---

## 💾 Historial

the-wired mantiene el sistema de historial de ani-es (`history.json`) para recordar el último episodio visto y el progreso de reproducción.

**Las descargas no modifican el historial.** Si descargas un episodio, tu estado de reproducción no se ve afectado.

---

## 🧠 Cómo funciona la descarga

```
Usuario selecciona "Descargar episodio"
  ↓
scrape_episode() obtiene URL fresca del .m3u8 (mismo scraper que ani-es)
  ↓
download_episode() llama a yt-dlp con la URL
  ↓
yt-dlp descarga los fragmentos HLS (364 fragmentos aprox.)
  ↓
FFmpeg remuxea a MP4 (sin recodificar: H.264 + AAC → contenedor MP4)
  ↓
Archivo final: <Anime> - S01E01.mp4
```

Para **Descargar temporada**, este proceso se repite por cada episodio, obteniendo una URL nueva cada vez:

```
episodio 1 → scrape_episode() → URL fresca → yt-dlp
episodio 2 → scrape_episode() → URL fresca → yt-dlp
episodio 3 → scrape_episode() → URL fresca → yt-dlp
...
```

No se cachean URLs porque los parámetros `st` (start) y `e` (expire) del `.m3u8` expiran.

---

## ❌ Desinstalar

```bash
rm /usr/local/bin/the-wired
rm -rf ~/the-wired
```

---

## 📜 Créditos

- **[Zhuchii/ani-es](https://github.com/Zhuchii/ani-es)** — el proyecto original. the-wired es un fork que respeta y reutiliza todo el scraper original.
- **[ani-cli](https://github.com/pystardust/ani-cli)** — la guía de instalación en Windows (Git Bash + Windows Terminal + Scoop) está basada en su documentación.
- **[yt-dlp](https://github.com/yt-dlp/yt-dlp)** — motor de descarga HLS.
- **[FFmpeg](https://ffmpeg.org/)** — remuxeo a MP4.

---

## ⚠️ Nota legal

Este proyecto es solo una interfaz CLI para interactuar con sitios de streaming públicos. No aloja, distribuye ni facilita contenido propio. El usuario es responsable del uso que haga de la herramienta.

---

## ❤️ Contribuir

Las contribuciones son bienvenidas, pero ten en cuenta que el entorno de test principal es Windows 11 + Git Bash. Si añades features, por favor prueba que:

1. La reproducción con mpv sigue funcionando.
2. El scraper original no se rompe.
3. `history.json` no se corrompe.
4. Las descargas generan MP4 válidos (verificables con `ffprobe`).
