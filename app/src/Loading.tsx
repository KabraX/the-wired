import { Text, makeStyles, tokens } from "@fluentui/react-components";

// Sustituye al Spinner de Fluent, que en WebKitGTK (Linux) se dibuja a trozos:
// un arco SVG girando se ve igual en todos los motores.
const useStyles = makeStyles({
  root: { display: "inline-flex", alignItems: "center", gap: tokens.spacingHorizontalS },
  svg: {
    flexShrink: 0,
    animationName: { from: { transform: "rotate(0deg)" }, to: { transform: "rotate(360deg)" } },
    animationDuration: "0.9s",
    animationIterationCount: "infinite",
    animationTimingFunction: "linear",
  },
  track: { stroke: tokens.colorNeutralStroke2 },
  arc: { stroke: tokens.colorBrandStroke1 },
});

type Props = { label?: string; size?: number };

export default function Loading({ label, size = 24 }: Props) {
  const styles = useStyles();
  return (
    <span className={styles.root} role="status">
      <svg className={styles.svg} width={size} height={size} viewBox="0 0 24 24" fill="none" aria-hidden>
        <circle className={styles.track} cx="12" cy="12" r="10" strokeWidth="2.5" />
        <circle
          className={styles.arc}
          cx="12"
          cy="12"
          r="10"
          strokeWidth="2.5"
          strokeLinecap="round"
          strokeDasharray="20 63"
        />
      </svg>
      {label && <Text>{label}</Text>}
    </span>
  );
}
