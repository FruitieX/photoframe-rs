"use client";
import React from "react";
import Container from "@mui/material/Container";
import Grid from "@mui/material/Grid";
import Paper from "@mui/material/Paper";
import Stack from "@mui/material/Stack";
import Chip from "@mui/material/Chip";
import Typography from "@mui/material/Typography";
import {
  useConfigQuery,
  useSetImmichCredentials,
  useSetImmichFilters,
} from "../../hooks/http";
import TextField from "@mui/material/TextField";
import IconButton from "@mui/material/IconButton";
import InputAdornment from "@mui/material/InputAdornment";
import Visibility from "@mui/icons-material/Visibility";
import VisibilityOff from "@mui/icons-material/VisibilityOff";
import Button from "@mui/material/Button";

const API_BASE = process.env.NEXT_PUBLIC_API_BASE ?? "/api";

export default function SourcesPage() {
  const { data, isLoading, error } = useConfigQuery(API_BASE);
  const entries = data ? Object.entries(data.sources) : [];
  return (
    <Container className="py-6">
      <h1 className="text-xl font-semibold mb-4">Sources</h1>
      {isLoading && <p>Loading…</p>}
      {error && <p className="text-red-600">Error</p>}
      <Grid container spacing={2}>
        {entries.map(([id, src]) => (
          <Grid key={id} size={12}>
            <Paper className="p-3">
              <Stack spacing={1}>
                <Stack direction="row" spacing={1} alignItems="center">
                  <Typography variant="h6" className="font-medium">
                    {id}
                  </Typography>
                  <Chip size="small" label={src.kind} />
                </Stack>
                {src.kind === "immich" && (
                  <ImmichInlineOnboard apiBase={API_BASE} sourceId={id} />
                )}
                {src.kind === "filesystem" && (
                  <div className="text-sm opacity-80">
                    <p>
                      Glob: <code>{src.filesystem?.glob || "(none)"}</code>
                    </p>
                    <p>
                      Order: <code>{src.filesystem?.order || "random"}</code>
                    </p>
                  </div>
                )}
              </Stack>
            </Paper>
          </Grid>
        ))}
      </Grid>
    </Container>
  );
}

function ImmichInlineOnboard({
  apiBase,
  sourceId,
}: {
  apiBase: string;
  sourceId: string;
}) {
  const creds = useSetImmichCredentials(apiBase, sourceId);
  const filtersMut = useSetImmichFilters(apiBase, sourceId);
  const { data } = useConfigQuery(apiBase);
  const current = data?.sources?.[sourceId]?.immich;
  const [baseUrl, setBaseUrl] = React.useState("");
  const [apiKey, setApiKey] = React.useState("");
  const [showKey, setShowKey] = React.useState(false);
  const [filtersText, setFiltersText] = React.useState(
    '{\n  "albumIds": [],\n  "personIds": []\n}',
  );
  const [filtersError, setFiltersError] = React.useState<string | null>(null);

  // Prefill from config when it loads/changes
  React.useEffect(() => {
    if (current) {
      if (current.base_url && current.base_url !== baseUrl)
        setBaseUrl(current.base_url);
      if (current.api_key && current.api_key !== apiKey)
        setApiKey(current.api_key);
      if (current.filters) {
        try {
          const pretty = JSON.stringify(current.filters, null, 2);
          if (pretty !== filtersText) setFiltersText(pretty);
        } catch {}
      }
    }
  }, [current, baseUrl, apiKey, filtersText]);

  const validate = React.useCallback((txt: string) => {
    try {
      const parsed = JSON.parse(txt);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        setFiltersError(null);
        return parsed as Record<string, unknown>;
      }
      setFiltersError("Must be a JSON object");
      return null;
    } catch (e) {
      setFiltersError("Invalid JSON");
      return null;
    }
  }, []);

  const onSaveFilters = () => {
    const parsed = validate(filtersText);
    if (!parsed) return;
    filtersMut.mutate({ filters: parsed });
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex gap-2 items-center">
        <TextField
          size="small"
          label="Immich Base URL"
          value={baseUrl}
          onChange={(e) => setBaseUrl(e.target.value)}
          className="flex-1"
        />
        <TextField
          size="small"
          label="API Key"
          type={showKey ? "text" : "password"}
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          className="flex-1"
          InputProps={{
            endAdornment: (
              <InputAdornment position="end">
                <IconButton
                  aria-label={showKey ? "Hide API key" : "Show API key"}
                  onClick={() => setShowKey((v) => !v)}
                  edge="end"
                  size="small"
                >
                  {showKey ? (
                    <VisibilityOff fontSize="small" />
                  ) : (
                    <Visibility fontSize="small" />
                  )}
                </IconButton>
              </InputAdornment>
            ),
          }}
        />
        <Button
          size="small"
          variant="contained"
          onClick={() => creds.mutate({ base_url: baseUrl, api_key: apiKey })}
          disabled={!baseUrl || !apiKey || creds.isPending}
        >
          Save
        </Button>
      </div>
      <div className="flex flex-col gap-2">
        <Typography variant="body2" className="opacity-70">
          Search Filters (JSON for Immich /api/search/asset). type=["IMAGE"] is
          always enforced server-side.
        </Typography>
        <TextField
          size="small"
          label="Filters JSON"
          value={filtersText}
          onChange={(e) => {
            setFiltersText(e.target.value);
            validate(e.target.value);
          }}
          multiline
          minRows={3}
          error={!!filtersError}
          helperText={filtersError || "albumIds, personIds, dateAfter, etc."}
        />
        <div className="flex gap-2">
          <Button
            size="small"
            variant="outlined"
            onClick={onSaveFilters}
            disabled={!!filtersError || filtersMut.isPending}
          >
            Save Filters
          </Button>
          {filtersMut.isSuccess && (
            <span className="text-green-600 text-xs">Saved.</span>
          )}
          {filtersMut.isError && (
            <span className="text-red-600 text-xs">Error saving.</span>
          )}
        </div>
      </div>
    </div>
  );
}
