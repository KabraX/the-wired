#!/bin/bash
# Desinstala the-wired. Conserva el historial salvo que se pida --purge.
#
#   ./uninstall.sh           quita el comando
#   ./uninstall.sh --purge   quita también el historial (~/the-wired)

set -euo pipefail

INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
PURGE=0
[[ "${1:-}" == "--purge" ]] && PURGE=1

removed=0

if [[ -e "$INSTALL_DIR/the-wired" || -L "$INSTALL_DIR/the-wired" ]]; then
    rm -f "$INSTALL_DIR/the-wired"
    echo "Eliminado $INSTALL_DIR/the-wired"
    removed=1
fi

# Instalaciones antiguas (versiones anteriores copiaban a /usr/local/bin con sudo)
for legacy in /usr/local/bin/the-wired /usr/local/bin/excepciones.json; do
    if [[ -e "$legacy" ]]; then
        sudo rm -f "$legacy"
        echo "Eliminado $legacy (instalación antigua)"
        removed=1
    fi
done

if [[ "$PURGE" == "1" && -d "$HOME/the-wired" ]]; then
    rm -rf "$HOME/the-wired"
    echo "Eliminado el historial ($HOME/the-wired)"
fi

if [[ "$removed" == "0" ]]; then
    echo "the-wired no estaba instalado."
else
    echo "the-wired desinstalado."
    if [[ "$PURGE" == "0" ]]; then
        echo "El historial sigue en $HOME/the-wired (bórralo con --purge)."
    fi
fi
