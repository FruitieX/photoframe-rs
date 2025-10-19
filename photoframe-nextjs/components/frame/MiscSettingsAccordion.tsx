import React, { useRef } from "react";
import Accordion from "@mui/material/Accordion";
import AccordionSummary from "@mui/material/AccordionSummary";
import AccordionDetails from "@mui/material/AccordionDetails";
import ExpandMoreIcon from "@mui/icons-material/ExpandMore";
import Typography from "@mui/material/Typography";
import Stack from "@mui/material/Stack";
import Checkbox from "@mui/material/Checkbox";
import FormControlLabel from "@mui/material/FormControlLabel";
import Button from "@mui/material/Button";
import LinearProgress from "@mui/material/LinearProgress";
import LayersClearIcon from "@mui/icons-material/LayersClear";
import FileUploadIcon from "@mui/icons-material/FileUpload";
import PauseCircleIcon from "@mui/icons-material/PauseCircle";
import BuildCircleIcon from "@mui/icons-material/BuildCircle";
import ScreenRotationIcon from "@mui/icons-material/ScreenRotation";
import PlayArrowIcon from "@mui/icons-material/PlayArrow";
import BlockIcon from "@mui/icons-material/Block";

interface PaletteInfo {
  palette: { input: string; hex: string; rgb: [number, number, number] }[];
}

interface Props {
  expanded: boolean;
  onToggle: (expanded: boolean) => void;
  uiState: import("./types").FrameUiState;
  setUiState: import("./types").SetFrameUiState;
  flipPending: boolean;
  pausePending: boolean;
  dummyPending: boolean;
  onUpload: (file: File) => void;
  uploadPending: boolean;
  onClear: () => void;
  clearPending: boolean;
  onTrigger: () => void;
  triggerPending: boolean;
  onBlacklist: (assetId: string, sourceId: string) => void;
  blacklistPending: boolean;
  currentAssetId?: string;
  currentSourceId?: string;
  palette: PaletteInfo | undefined;
}

export function MiscSettingsAccordion(props: Props) {
  const {
    expanded,
    onToggle,
    uiState,
    setUiState,
    flipPending,
    pausePending,
    dummyPending,
    onUpload,
    uploadPending,
    onClear,
    clearPending,
    onTrigger,
    triggerPending,
    onBlacklist,
    blacklistPending,
    currentAssetId,
    currentSourceId,
    palette,
  } = props;
  const { flip, paused, dummy } = uiState;

  const fileInputRef = useRef<HTMLInputElement | null>(null);

  return (
    <Accordion
      expanded={expanded}
      disableGutters
      onChange={(_, e) => onToggle(!!e)}
    >
      <AccordionSummary expandIcon={<ExpandMoreIcon />}>
        <Typography>Misc Settings</Typography>
      </AccordionSummary>
      <AccordionDetails>
        <Stack spacing={2} sx={{ px: 1 }}>
          <div className="flex flex-col gap-1">
            <FormControlLabel
              control={
                <Checkbox
                  size="small"
                  checked={flip}
                  onChange={(_, c) => setUiState({ ...uiState, flip: c })}
                />
              }
              label={
                flipPending ? (
                  "Updating…"
                ) : (
                  <span className="flex items-center gap-1">
                    <ScreenRotationIcon fontSize="small" /> Flip 180°
                  </span>
                )
              }
            />
            <FormControlLabel
              control={
                <Checkbox
                  size="small"
                  checked={paused}
                  onChange={(_, c) => setUiState({ ...uiState, paused: c })}
                />
              }
              label={
                pausePending ? (
                  "Updating…"
                ) : (
                  <span className="flex items-center gap-1">
                    <PauseCircleIcon fontSize="small" />
                    Pause schedule
                  </span>
                )
              }
            />
            <FormControlLabel
              control={
                <Checkbox
                  size="small"
                  checked={dummy}
                  onChange={(_, c) => setUiState({ ...uiState, dummy: c })}
                />
              }
              label={
                dummyPending ? (
                  "Updating…"
                ) : (
                  <span className="flex items-center gap-1">
                    <BuildCircleIcon fontSize="small" /> Dummy mode
                  </span>
                )
              }
            />
          </div>

          <input
            ref={fileInputRef}
            hidden
            type="file"
            accept="image/*"
            onChange={(e) => {
              if (e.target.files?.length) onUpload(e.target.files[0]);
            }}
          />

          <Stack
            direction="column"
            spacing={1}
            alignItems="flex-start"
            flexWrap="wrap"
          >
            <Button
              size="small"
              startIcon={<LayersClearIcon fontSize="small" />}
              onClick={onClear}
              disabled={clearPending}
            >
              Clear screen
            </Button>
            <Button
              size="small"
              startIcon={<PlayArrowIcon fontSize="small" />}
              onClick={onTrigger}
              disabled={triggerPending}
            >
              Trigger schedule manually
            </Button>
            <Button
              size="small"
              startIcon={<FileUploadIcon fontSize="small" />}
              onClick={() => fileInputRef.current?.click()}
              disabled={uploadPending}
            >
              Upload image (auto pause schedule)
            </Button>
            {uploadPending && (
              <LinearProgress sx={{ width: 100, height: 4, borderRadius: 1 }} />
            )}
            <Button
              size="small"
              startIcon={<BlockIcon fontSize="small" />}
              onClick={() => {
                if (currentAssetId && currentSourceId) {
                  onBlacklist(currentAssetId, currentSourceId);
                }
              }}
              disabled={blacklistPending || !currentAssetId || !currentSourceId}
            >
              Blacklist current image
            </Button>
          </Stack>

          {palette ? (
            <div className="p-2">
              <Typography variant="caption" gutterBottom>
                Resolved palette
              </Typography>
              <div className="flex flex-wrap gap-6 mt-1">
                {palette.palette.map((p, idx) => (
                  <div key={idx} className="flex items-center gap-2">
                    <div
                      className="w-6 h-6 rounded border"
                      style={{
                        backgroundColor:
                          p.hex !== "invalid" ? p.hex : "#000000",
                      }}
                      title={p.hex}
                    />
                    <Typography variant="caption">
                      {p.input} → {p.hex} ({p.rgb[0]},{p.rgb[1]},{p.rgb[2]})
                    </Typography>
                  </div>
                ))}
              </div>
            </div>
          ) : (
            <Typography variant="caption" color="text.secondary">
              Loading palette…
            </Typography>
          )}
        </Stack>
      </AccordionDetails>
    </Accordion>
  );
}
