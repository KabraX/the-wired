# the-wired

> Fork de [ani-es](https://github.com/Zhuchii/ani-es) con soporte de descarga integrado para bibliotecas **Jellyfin**.

Descargador de anime en español con dos formas de usarlo:

| | Para quién | Qué hace |
|---|---|---|
| 🖥️ **[App de escritorio](#app-de-escritorio-windows-y-linux)** | La mayoría: instalar y usar | Buscar, descargar con cola, pausa y reintentos, y ver tu biblioteca. Windows y Linux |
| ⌨️ **[Script de terminal](#script-de-terminal)** | Quien vive en la terminal | El reproductor + descargador original (ani-es), con `fzf` y `mpv` |

Los dos guardan las descargas con la misma estructura de carpetas compatible con Jellyfin, así que puedes usar uno u otro sobre la misma biblioteca.

---

## App de escritorio (Windows y Linux)

### Descargar

Descarga el instalador de la **[última versión](https://github.com/KabraX/the-wired/releases/latest)**:

| Sistema | Archivo |
|---|---|
| **Windows 10/11** | `the-wired_X.Y.Z_x64-setup.exe` (recomendado) o `.msi` |
| **Linux** (cualquier distro) | `the-wired_X.Y.Z_amd64.AppImage` |
| **Debian / Ubuntu** | `.deb` |
| **Fedora / openSUSE** | `.rpm` |

Los archivos `.sig` que acompañan a cada instalador son firmas para las actualizaciones automáticas: no hace falta descargarlos.

- **Windows:** el instalador no está firmado con un certificado de pago, así que SmartScreen puede avisar de que *"Windows protegió su PC"*. Pulsa **Más información → Ejecutar de todos modos**. Se instala para tu usuario, sin pedir permisos de administrador.
- **AppImage:** dale permiso de ejecución (`chmod +x the-wired_*.AppImage`) y ábrelo.

### Qué hace

- **Buscar** un anime y ver su ficha: episodios, tipo, calidad y cuáles ya tienes descargados.
- **Descargar** episodios sueltos o la temporada entera, con una **cola** que se puede **pausar y reanudar** y que se retoma sola al volver a abrir la app.
- **Reintentos automáticos** (a los 1, 5 y 15 minutos) cuando la web falla, y **notificación** al terminar.
- **Icono en la bandeja**: si cierras la ventana con descargas en marcha, siguen en segundo plano.
- **Biblioteca**: todo lo que tienes descargado (también lo bajado con el script), con portadas, episodios, tamaño y avisos de descargas a medias.
- **Formato Jellyfin** opcional (`Season 01/<Anime> - S01E01.mp4`) y detección de temporadas desde el título (*"Overlord II"* → serie *Overlord*, temporada 2).
- **Varias fuentes**: jkanime y HentaiJK (contenido +18, **desactivada por defecto**), cada una con su propia carpeta de biblioteca.
- Tema claro / oscuro con colores de acento de Fluent.

### Primer arranque

La app no incluye las herramientas de descarga en el instalador. La primera vez ofrece descargarlas de sus repositorios oficiales:

- **yt-dlp** (Windows y Linux) y **ffmpeg** (solo Windows; en Linux se usa el del gestor de paquetes).
- Si ya las tienes instaladas en el sistema, la app usa esas.

### Actualizaciones

Al abrirse, la app comprueba si hay una versión nueva y la ofrece con un aviso: un clic la descarga, la instala y reinicia. Las actualizaciones van firmadas y la app rechaza cualquiera que no lo esté.

### Si algo falla

En **Ajustes → Registro → Copiar registro** se copia la versión, el sistema, las herramientas y lo último que hizo la app. Pégalo al [abrir un issue](https://github.com/KabraX/the-wired/issues).

### Dónde guarda sus datos

| | Windows | Linux |
|---|---|---|
| Ajustes | `%APPDATA%\io.github.kabrax.thewired\` | `~/.config/io.github.kabrax.thewired/` |
| Cola, índice de la biblioteca, yt-dlp/ffmpeg propios | `%APPDATA%\io.github.kabrax.thewired\` | `~/.local/share/io.github.kabrax.thewired/` |
| Registro | `%LOCALAPPDATA%\io.github.kabrax.thewired\logs\` | `~/.local/share/io.github.kabrax.thewired/logs/` |

Tu carpeta de biblioteca solo contiene los vídeos: la app no guarda nada más ahí.

### Compilar desde el código

Requiere [Rust](https://rustup.rs/), Node.js y [pnpm](https://pnpm.io/), además de las [dependencias de Tauri](https://tauri.app/start/prerequisites/) de tu sistema.

```bash
cd app
pnpm install
pnpm tauri dev      # modo desarrollo
pnpm tauri build    # instaladores en app/src-tauri/target/release/bundle/
```

Diseño y fases del proyecto: [`docs/app-tauri.md`](docs/app-tauri.md).

---

## Script de terminal

Reproductor **+ descargador** de anime en español desde la terminal. Manteniendo el scraper original de ani-es intacto, añade la capacidad de descargar episodios y temporadas completas directamente a una estructura de carpetas compatible con Jellyfin.

---

## ⚠️ Aviso importante

**Entornos probados (script):**

- **Windows 11** con Git Bash / MINGW64 (no WSL) en Windows Terminal
- **Linux:** CachyOS (Arch) con bash; la descarga va directa a una biblioteca de Jellyfin

No se ha probado en macOS ni en WSL.

the-wired (script y app) depende de la web de jkanime.net: si el sitio cambia su HTML, la búsqueda o la obtención de vídeos pueden dejar de funcionar hasta que se actualice el scraper. En ese caso el script lo indica y sugiere `the-wired -u`, y la app avisa de que *"la web parece haber cambiado"*.

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

La ruta por defecto es `<carpeta de Vídeos>/Anime` (en Linux, `~/Videos/Anime` o la que indique `xdg-user-dir VIDEOS`). Se puede cambiar con `ANIME_DOWNLOAD_DIR` (ver «Configuración»).

### Modo descarga (`-d` / `--download`)

```bash
the-wired -d
the-wired --download
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
- **Auto-actualización:** reescrita en la 1.4.2 (ver «Actualizaciones» en Configuración).

---

## 📦 Instalación del script en Linux

```bash
git clone https://github.com/KabraX/the-wired.git
cd the-wired
./install.sh          # instala dependencias y copia the-wired a ~/.local/bin
# ./install.sh --link # o: enlace al clon, y actualizas con git pull
```

No copia nada como root: solo usa `sudo` para instalar las dependencias con el gestor de paquetes (pacman, apt, dnf o zypper). Para quitarlo: `./uninstall.sh` (añade `--purge` para borrar también el historial).

## 📦 Instalación del script (Windows 11 + Git Bash)

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

> Nota: `install.sh` es para Linux y **no funciona en Git Bash**. En Windows usa la instalación manual de arriba.

---

## ⌨️ Uso del script

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
the-wired -u        # buscar actualizaciones ahora
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

## ⚙️ Configuración del script

### Ruta de descarga

Por defecto las descargas van a `<carpeta de Vídeos>/Anime`. Para usar tu biblioteca de Jellyfin:

```bash
# bash / Git Bash (añádelo a ~/.bashrc para que persista)
export ANIME_DOWNLOAD_DIR="/ruta/a/tu/biblioteca/Anime"   # Windows: "D:/Multimedy/Anime"

# fish
set -Ux ANIME_DOWNLOAD_DIR /ruta/a/tu/biblioteca/Anime
```

### Actualizaciones

- the-wired busca una versión nueva **como mucho una vez al día** y solo la ofrece si su número de versión es **mayor** que el instalado (nunca baja de versión).
- Antes de instalarla comprueba que el archivo descargado es un script válido.
- `the-wired -u` fuerza la comprobación; `THE_WIRED_NO_UPDATE=1` la desactiva.
- Si lo instalaste como enlace a un clon de git, no se reescribe: actualiza con `git pull`.

### Archivos temporales

Las páginas que descarga el scraper se guardan en una carpeta temporal que se borra al salir, así que puedes ejecutar `the-wired` desde cualquier carpeta sin que se llene de archivos.

### Estructura de carpetas Jellyfin

```
<ANIME_DOWNLOAD_DIR>/
  <Anime>/
    Season 01/
      <Anime> - S01E01.mp4
      <Anime> - S01E02.mp4
```

Los nombres de archivo se sanitizan con las reglas de Windows (también en Linux, para que la biblioteca funcione en ambos) (se eliminan `< > : " / \ | ? *`). Los caracteres Unicode (acentos, etc.) se preservan.

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

**App de escritorio:** desde *Configuración → Aplicaciones* en Windows, o con tu gestor de paquetes en Linux (`.deb` / `.rpm`). La AppImage basta con borrarla. Tus vídeos no se tocan.

**Script en Linux:**

```bash
./uninstall.sh          # desde el clon; añade --purge para borrar también el historial
```

**Script en Windows (Git Bash):**

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
- **[Tauri](https://tauri.app/)** y **[Fluent UI](https://react.fluentui.dev/)** — base de la app de escritorio.

---

## ⚠️ Nota legal

Este proyecto (script y app) es solo una interfaz para interactuar con sitios de streaming públicos. No aloja, distribuye ni facilita contenido propio. El usuario es responsable del uso que haga de la herramienta.

---

## ❤️ Contribuir

Las contribuciones son bienvenidas.

**App:** antes de enviar cambios, ejecuta los tests y el linter (la CI los pasa en Linux y Windows):

```bash
cd app/src-tauri && cargo test && cargo clippy --all-targets
cd .. && pnpm exec tsc --noEmit
```

**Script:** el entorno de test principal es Windows 11 + Git Bash, y la CI pasa ShellCheck. Si añades features, por favor prueba que:

1. La reproducción con mpv sigue funcionando.
2. El scraper original no se rompe.
3. `history.json` no se corrompe.
4. Las descargas generan MP4 válidos (verificables con `ffprobe`).
