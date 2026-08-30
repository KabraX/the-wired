#!/bin/bash


INSTALL_DIR="/usr/local/bin"

if [ ! -f "$INSTALL_DIR/the-wired" ]; then
    echo "El script the-wired no está instalado en el sistema."
    exit 1
fi

if [ ! -f "$INSTALL_DIR/excepciones.json" ]; then
    exit 1
fi

sudo rm "$INSTALL_DIR/the-wired"
sudo rm "$INSTALL_DIR/excepciones.json"
sudo rm -r ~/the-wired/history.db

echo "El script the-wired ha sido desinstalado correctamente."
