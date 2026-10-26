import { useState } from "react";
import { Button, type ButtonProps } from "@fluentui/react-components";
import { CheckmarkRegular, CopyRegular } from "@fluentui/react-icons";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { api } from "./api";

/** Copia el registro (versión, herramientas y últimas líneas del log) para enviarlo con un aviso de error. */
export default function CopyLogButton(props: ButtonProps) {
  const [state, setState] = useState<"idle" | "copied" | "error">("idle");
  return (
    <Button
      {...props}
      icon={state === "copied" ? <CheckmarkRegular /> : <CopyRegular />}
      onClick={async () => {
        try {
          await writeText(await api.readLog());
          setState("copied");
        } catch {
          setState("error");
        }
        setTimeout(() => setState("idle"), 2500);
      }}
    >
      {state === "copied" ? "Copiado" : state === "error" ? "No se pudo copiar" : "Copiar registro"}
    </Button>
  );
}
