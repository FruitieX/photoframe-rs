import React from "react";
import Accordion from "@mui/material/Accordion";
import AccordionSummary from "@mui/material/AccordionSummary";
import AccordionDetails from "@mui/material/AccordionDetails";
import ExpandMoreIcon from "@mui/icons-material/ExpandMore";
import Typography from "@mui/material/Typography";
import Stack from "@mui/material/Stack";
import Slider from "@mui/material/Slider";
import Switch from "@mui/material/Switch";
import FormControl from "@mui/material/FormControl";
import InputLabel from "@mui/material/InputLabel";
import Select from "@mui/material/Select";
import MenuItem from "@mui/material/MenuItem";
import FormControlLabel from "@mui/material/FormControlLabel";
import TextField from "@mui/material/TextField";
import type { FrameUiState, SetFrameUiState } from "./types";
import type { TimestampPosition, TimestampColor, TimestampStrokeColor } from "../../hooks/http";

interface Props {
  expanded: boolean;
  onToggle: (expanded: boolean) => void;
  uiState: FrameUiState;
  setUiState: SetFrameUiState;
}

const POSITION_OPTIONS: { value: TimestampPosition; label: string }[] = [
  { value: "bottom_left", label: "Bottom Left" },
  { value: "bottom_center", label: "Bottom Center" },
  { value: "bottom_right", label: "Bottom Right" },
  { value: "top_left", label: "Top Left" },
  { value: "top_center", label: "Top Center" },
  { value: "top_right", label: "Top Right" },
];

const COLOR_OPTIONS: { value: TimestampColor; label: string }[] = [
  { value: "transparent_white_text", label: "Transparent (White Text)" },
  { value: "transparent_black_text", label: "Transparent (Black Text)" },
  { value: "transparent_auto_text", label: "Transparent (Auto Text Color)" },
  { value: "white_background", label: "White Background" },
  { value: "black_background", label: "Black Background" },
];

const STROKE_COLOR_OPTIONS: { value: TimestampStrokeColor; label: string }[] = [
  { value: "auto", label: "Auto" },
  { value: "white", label: "White" },
  { value: "black", label: "Black" },
];

export function TimestampAccordion({
  expanded,
  onToggle,
  uiState,
  setUiState,
}: Props) {
  const {
    timestampEnabled,
    timestampPosition,
    timestampFontSize,
    timestampColor,
    timestampFullWidthBanner,
    timestampBannerHeight,
    timestampStrokeEnabled,
    timestampStrokeWidth,
    timestampStrokeColor,
    timestampFormat,
  } = uiState;

  return (
    <Accordion
      expanded={expanded}
      disableGutters
      onChange={(_, e) => onToggle(!!e)}
    >
      <AccordionSummary expandIcon={<ExpandMoreIcon />}>
        <Typography>Timestamp</Typography>
      </AccordionSummary>
      <AccordionDetails>
        <Stack spacing={3} sx={{ px: 1 }}>
          <FormControlLabel
            control={
              <Switch
                checked={timestampEnabled}
                onChange={(e) =>
                  setUiState({
                    ...uiState,
                    timestampEnabled: e.target.checked,
                  })
                }
              />
            }
            label="Enable Timestamp"
          />

          {timestampEnabled && (
            <>
              <FormControl size="small" fullWidth>
                <InputLabel>Position</InputLabel>
                <Select
                  value={timestampPosition}
                  label="Position"
                  onChange={(e) =>
                    setUiState({
                      ...uiState,
                      timestampPosition: e.target.value as string,
                    })
                  }
                >
                  {POSITION_OPTIONS.map((option) => (
                    <MenuItem key={option.value} value={option.value}>
                      {option.label}
                    </MenuItem>
                  ))}
                </Select>
              </FormControl>

              <div>
                <Typography variant="caption" gutterBottom>
                  Font Size ({timestampFontSize}px)
                </Typography>
                <Slider
                  size="small"
                  value={timestampFontSize}
                  onChange={(_, v) =>
                    setUiState({
                      ...uiState,
                      timestampFontSize: v as number,
                    })
                  }
                  min={8}
                  max={72}
                  step={1}
                />
              </div>

              <FormControl size="small" fullWidth>
                <InputLabel>Color</InputLabel>
                <Select
                  value={timestampColor}
                  label="Color"
                  onChange={(e) =>
                    setUiState({
                      ...uiState,
                      timestampColor: e.target.value as string,
                    })
                  }
                >
                  {COLOR_OPTIONS.map((option) => (
                    <MenuItem key={option.value} value={option.value}>
                      {option.label}
                    </MenuItem>
                  ))}
                </Select>
              </FormControl>

              <FormControlLabel
                control={
                  <Switch
                    checked={timestampFullWidthBanner}
                    onChange={(e) =>
                      setUiState({
                        ...uiState,
                        timestampFullWidthBanner: e.target.checked,
                      })
                    }
                  />
                }
                label="Full Width Banner"
              />

              {timestampFullWidthBanner && (
                <TextField
                  size="small"
                  type="number"
                  label="Banner Height (px)"
                  value={timestampBannerHeight}
                  onChange={(e) =>
                    setUiState({
                      ...uiState,
                      timestampBannerHeight: parseInt(e.target.value) || 0,
                    })
                  }
                  inputProps={{ min: 0 }}
                />
              )}

              <TextField
                size="small"
                label="Timestamp Format"
                value={timestampFormat ?? ""}
                placeholder="%Y-%m-%d"
                helperText="Chrono format, e.g. %Y-%m-%d %H:%M"
                onChange={(e) =>
                  setUiState({
                    ...uiState,
                    timestampFormat: e.target.value || undefined,
                  })
                }
              />

              <div>
                <Typography variant="caption" gutterBottom>
                  Horizontal Padding ({uiState.timestampPaddingHorizontal}px)
                </Typography>
                <Slider
                  size="small"
                  value={uiState.timestampPaddingHorizontal}
                  onChange={(_, v) =>
                    setUiState({
                      ...uiState,
                      timestampPaddingHorizontal: v as number,
                    })
                  }
                  min={0}
                  max={100}
                  step={1}
                />
              </div>

              <div>
                <Typography variant="caption" gutterBottom>
                  Vertical Padding ({uiState.timestampPaddingVertical}px)
                </Typography>
                <Slider
                  size="small"
                  value={uiState.timestampPaddingVertical}
                  onChange={(_, v) =>
                    setUiState({
                      ...uiState,
                      timestampPaddingVertical: v as number,
                    })
                  }
                  min={0}
                  max={100}
                  step={1}
                />
              </div>

              <FormControlLabel
                control={
                  <Switch
                    checked={timestampStrokeEnabled}
                    onChange={(e) =>
                      setUiState({
                        ...uiState,
                        timestampStrokeEnabled: e.target.checked,
                      })
                    }
                  />
                }
                label="Text Stroke (Outline)"
              />

              {timestampStrokeEnabled && (
                <>
                  <div>
                    <Typography variant="caption" gutterBottom>
                      Stroke Width ({timestampStrokeWidth}px)
                    </Typography>
                    <Slider
                      size="small"
                      value={timestampStrokeWidth}
                      onChange={(_, v) =>
                        setUiState({
                          ...uiState,
                          timestampStrokeWidth: v as number,
                        })
                      }
                      min={1}
                      max={12}
                      step={1}
                    />
                  </div>

                  <FormControl size="small" fullWidth>
                    <InputLabel>Stroke Color</InputLabel>
                    <Select
                      value={timestampStrokeColor}
                      label="Stroke Color"
                      onChange={(e) =>
                        setUiState({
                          ...uiState,
                          timestampStrokeColor: e.target.value as string,
                        })
                      }
                    >
                      {STROKE_COLOR_OPTIONS.map((option) => (
                        <MenuItem key={option.value} value={option.value}>
                          {option.label}
                        </MenuItem>
                      ))}
                    </Select>
                  </FormControl>
                </>
              )}
            </>
          )}
        </Stack>
      </AccordionDetails>
    </Accordion>
  );
}