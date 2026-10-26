import { makeStyles, mergeClasses, tokens } from "@fluentui/react-components";
import { siteName, type Site } from "./api";

// Los logos (public/sources/<fuente>.png) tienen el texto en blanco: van sobre una placa
// oscura para que se lean igual con el tema claro y el oscuro.
const useStyles = makeStyles({
  plate: {
    display: "inline-flex",
    alignItems: "center",
    flexShrink: 0,
    backgroundColor: "#141414",
    borderRadius: tokens.borderRadiusMedium,
    padding: "3px 6px",
    lineHeight: 0,
  },
  img: { display: "block", width: "auto" },
});

type Props = { site: Site; height?: number; className?: string };

export default function SourceLogo({ site, height = 16, className }: Props) {
  const styles = useStyles();
  return (
    <span className={mergeClasses(styles.plate, className)} title={siteName(site)}>
      <img className={styles.img} src={`/sources/${site}.png`} alt={siteName(site)} style={{ height }} />
    </span>
  );
}
