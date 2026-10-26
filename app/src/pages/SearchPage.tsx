import { useState } from "react";
import {
  Badge,
  Button,
  Card,
  CardHeader,
  CardPreview,
  Input,
  MessageBar,
  MessageBarBody,
  Text,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { ChevronLeftRegular, ChevronRightRegular, SearchRegular } from "@fluentui/react-icons";
import { api, errorMessage, type SearchResult } from "../api";
import Loading from "../Loading";

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

type Props = { onOpen: (r: SearchResult) => void };

export default function SearchPage({ onOpen }: Props) {
  const styles = useStyles();
  const [query, setQuery] = useState("");
  const [searched, setSearched] = useState("");
  const [page, setPage] = useState(1);
  const [results, setResults] = useState<SearchResult[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function run(q: string, p: number) {
    if (!q.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const r = await api.search(q, p);
      setResults(r);
      setSearched(q);
      setPage(p);
    } catch (e) {
      setError(errorMessage(e));
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

      {loading && <Loading label="Buscando en jkanime…" />}
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
