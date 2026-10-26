import { useEffect, useState } from "react";
import {
  Badge,
  Button,
  Card,
  CardHeader,
  CardPreview,
  Dropdown,
  Input,
  Option,
  MessageBar,
  MessageBarBody,
  Text,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { ChevronLeftRegular, ChevronRightRegular, SearchRegular } from "@fluentui/react-icons";
import { api, errorMessage, siteName, type SearchResult, type Site } from "../api";
import Loading from "../Loading";
import SourceLogo from "../SourceLogo";

const useStyles = makeStyles({
  bar: { display: "flex", gap: tokens.spacingHorizontalS, maxWidth: "640px" },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(190px, 1fr))",
    gap: tokens.spacingHorizontalL,
  },
  card: { cursor: "pointer" },
  poster: { aspectRatio: "2 / 3", objectFit: "cover", width: "100%" },
  noPoster: { aspectRatio: "2 / 3", backgroundColor: tokens.colorNeutralBackground3 },
  pager: { display: "flex", alignItems: "center", gap: tokens.spacingHorizontalM },
});

type Props = {
  sources: Site[];
  /** Búsqueda pedida desde fuera (p. ej. desde la biblioteca). */
  request?: { source: Site; query: string; n: number } | null;
  onOpen: (r: SearchResult) => void;
};

const SOURCE_KEY = "the-wired.search-source";

function lastSource(): Site {
  try {
    return (localStorage.getItem(SOURCE_KEY) as Site | null) ?? "jkanime";
  } catch {
    return "jkanime";
  }
}

export default function SearchPage({ sources, request, onOpen }: Props) {
  const styles = useStyles();
  // Última fuente usada; si se desactivó en Ajustes, se vuelve a jkanime
  const [chosen, setChosen] = useState<Site>(lastSource);
  const source: Site = sources.includes(chosen) ? chosen : "jkanime";
  const changeSource = (s: Site) => {
    setChosen(s);
    try {
      localStorage.setItem(SOURCE_KEY, s);
    } catch {
      /* sin almacenamiento: se recuerda solo en esta sesión */
    }
  };
  const [query, setQuery] = useState("");
  const [searched, setSearched] = useState("");
  const [page, setPage] = useState(1);
  const [results, setResults] = useState<SearchResult[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Los resultados son de un sitio: al cambiar de fuente se descartan
  useEffect(() => {
    setResults(null);
    setError(null);
  }, [source]);

  useEffect(() => {
    if (!request) return;
    changeSource(request.source);
    setQuery(request.query);
    // Tras el cambio de fuente (que limpia los resultados), lanzar la búsqueda
    const t = setTimeout(() => run(request.query, 1, request.source), 0);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [request?.n]);

  async function run(q: string, p: number, src: Site = source) {
    if (!q.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const r = await api.search(src, q, p);
      setResults(r);
      setSearched(q);
      setPage(p);
    } catch (e) {
      setError(errorMessage(e, src));
    } finally {
      setLoading(false);
    }
  }

  return (
    <>
      <form
        className={styles.bar}
        onSubmit={(e) => {
          e.preventDefault();
          run(query, 1);
        }}
      >
        {sources.length > 1 && (
          <Dropdown
            aria-label="Fuente"
            style={{ minWidth: "150px" }}
            value={siteName(source)}
            button={{ children: <SourceLogo site={source} /> }}
            selectedOptions={[source]}
            onOptionSelect={(_, d) => d.optionValue && changeSource(d.optionValue as Site)}
          >
            {sources.map((s) => (
              <Option key={s} value={s} text={siteName(s)}>
                <SourceLogo site={s} />
              </Option>
            ))}
          </Dropdown>
        )}
        <Input
          style={{ flex: 1 }}
          contentBefore={<SearchRegular />}
          placeholder="Nombre del anime…"
          value={query}
          onChange={(_, d) => setQuery(d.value)}
          autoFocus
        />
        <Button appearance="primary" type="submit" disabled={loading || !query.trim()}>
          Buscar
        </Button>
      </form>

      {loading && <Loading label={`Buscando en ${siteName(source)}…`} />}
      {error && (
        <MessageBar intent="error">
          <MessageBarBody>{error}</MessageBarBody>
        </MessageBar>
      )}

      {!loading && results && results.length === 0 && (
        <Text>
          {page > 1 ? "No hay más resultados." : `No hay resultados para «${searched}».`}
        </Text>
      )}

      {!loading && results && results.length > 0 && (
        <>
          <div className={styles.grid}>
            {results.map((r) => (
              <Card key={r.url} className={styles.card} onClick={() => onOpen(r)}>
                <CardPreview>
                  {r.image ? (
                    <img className={styles.poster} src={r.image} alt="" loading="lazy" />
                  ) : (
                    <div className={styles.noPoster} />
                  )}
                </CardPreview>
                <CardHeader
                  header={<Text weight="semibold">{r.title}</Text>}
                  description={
                    <span>
                      {r.kind && <Badge appearance="tint">{r.kind}</Badge>} {r.status ?? ""}
                    </span>
                  }
                />
              </Card>
            ))}
          </div>
        </>
      )}

      {!loading && results && (results.length > 0 || page > 1) && (
        <div className={styles.pager}>
          <Button icon={<ChevronLeftRegular />} disabled={page <= 1} onClick={() => run(searched, page - 1)}>
            Anterior
          </Button>
          <Text>Página {page}</Text>
          <Button
            icon={<ChevronRightRegular />}
            iconPosition="after"
            disabled={results.length === 0}
            onClick={() => run(searched, page + 1)}
          >
            Siguiente
          </Button>
        </div>
      )}
    </>
  );
}
