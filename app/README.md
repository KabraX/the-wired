# the-wired — app de escritorio

App de escritorio (Tauri 2 + React + Fluent UI) para Windows y Linux. Hace lo mismo que el
script de terminal de la raíz del repo, sin necesitar bash ni herramientas externas.
Diseño y fases: [`../docs/app-tauri.md`](../docs/app-tauri.md).

## Desarrollo

Requisitos: Rust estable (`rustup`), Node.js LTS, pnpm y, en Linux, las
[dependencias de Tauri](https://tauri.app/start/prerequisites/) (`webkit2gtk-4.1`, etc.).

```bash
cd app
pnpm install
pnpm tauri dev      # app en modo desarrollo con recarga en caliente
cargo test --manifest-path src-tauri/Cargo.toml   # tests del núcleo
pnpm tauri build    # instaladores en src-tauri/target/release/bundle/
```

## Publicar

Crea y sube una etiqueta `app-vX.Y.Z`; el workflow `release-app.yml` compila los
instaladores de Windows y Linux y los adjunta a un borrador de Release.
Antes, sube la versión en `src-tauri/tauri.conf.json` y `src-tauri/Cargo.toml`.
