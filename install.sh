#!/bin/bash
# Instala the-wired para el usuario actual, sin copiar nada como root.
#
#   ./install.sh            instala las dependencias y copia el script a ~/.local/bin
#   ./install.sh --link     igual, pero con un enlace al clon (se actualiza con git pull)
#   ./install.sh --no-deps  no instala dependencias del sistema
#
# Variable: INSTALL_DIR (por defecto ~/.local/bin)

set -euo pipefail

INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
REPO_DIR="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
LINK=0
DEPS=1

for arg in "$@"; do
    case "$arg" in
        --link) LINK=1 ;;
        --no-deps) DEPS=0 ;;
        -h | --help)
            sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "Opción desconocida: $arg" >&2
            exit 1
            ;;
    esac
done

# Nombre del paquete de Python según el gestor
install_deps() {
    local common=(wget curl jq fzf mpv yt-dlp ffmpeg)

    if command -v pacman &>/dev/null; then
        # --needed: no reinstala lo que ya está. Sin -Sy, para no provocar una
        # actualización parcial del sistema (rompe Arch).
        sudo pacman -S --needed "${common[@]}" python
    elif command -v apt-get &>/dev/null; then
        sudo apt-get update
        sudo apt-get install -y "${common[@]}" python3
    elif command -v dnf &>/dev/null; then
        sudo dnf install -y "${common[@]}" python3
    elif command -v zypper &>/dev/null; then
        sudo zypper install -y "${common[@]}" python3
    else
        echo "Gestor de paquetes no reconocido. Instala a mano: ${common[*]} python3" >&2
        return 1
    fi
}

if [[ "$DEPS" == "1" ]]; then
    echo "Instalando dependencias (pedirá la contraseña de sudo)..."
    install_deps
fi

mkdir -p "$INSTALL_DIR"
target="$INSTALL_DIR/the-wired"

if [[ -e "$target" || -L "$target" ]]; then
    echo "Ya existe $target; se reemplaza."
    rm -f "$target"
fi

if [[ "$LINK" == "1" ]]; then
    ln -s "$REPO_DIR/the-wired" "$target"
    echo "Enlazado: $target -> $REPO_DIR/the-wired (actualiza con: git -C \"$REPO_DIR\" pull)"
else
    install -m 755 "$REPO_DIR/the-wired" "$target"
    echo "Instalado en $target (se actualiza solo; fuerza la comprobación con: the-wired -u)"
fi

# El historial lo crea el propio script con los permisos del usuario.
mkdir -p "$HOME/the-wired"

case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        echo
        echo "Aviso: $INSTALL_DIR no está en tu PATH. Añádelo, por ejemplo:"
        echo "  bash/zsh:  echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> ~/.bashrc"
        echo "  fish:      fish_add_path $INSTALL_DIR"
        ;;
esac

echo
echo "Listo. Ejecuta: the-wired"
