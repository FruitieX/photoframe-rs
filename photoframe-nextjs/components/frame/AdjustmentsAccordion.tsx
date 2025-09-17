import React from "react";
import Accordion from "@mui/material/Accordion";
import AccordionSummary from "@mui/material/AccordionSummary";
import AccordionDetails from "@mui/material/AccordionDetails";
import ExpandMoreIcon from "@mui/icons-material/ExpandMore";
import Typography from "@mui/material/Typography";
import Stack from "@mui/material/Stack";
import Slider from "@mui/material/Slider";
import Select from "@mui/material/Select";
import MenuItem from "@mui/material/MenuItem";
import IconButton from "@mui/material/IconButton";
import ChevronLeftIcon from "@mui/icons-material/ChevronLeft";
import ChevronRightIcon from "@mui/icons-material/ChevronRight";
import Checkbox from "@mui/material/Checkbox";
import FormControlLabel from "@mui/material/FormControlLabel";
import type { FrameUiState, SetFrameUiState } from "./types";

interface Props {
  expanded: boolean;
  onToggle: (expanded: boolean) => void;
  uiState: FrameUiState;
  setUiState: SetFrameUiState;
  cycleDithering: (delta: number) => void;
  requestImage: (useIntermediate: boolean) => void;
}

const DITHER_OPTIONS: { value: string; label: string }[] = [
  { value: "none", label: "None (nearest)" },
  { value: "ordered_bayer_2", label: "Ordered Bayer 2×2" },
  { value: "ordered_bayer_4", label: "Ordered Bayer 4×4" },
  { value: "ordered_bayer_8", label: "Ordered Bayer 8×8" },
  { value: "ordered_blue_256", label: "Blue noise 256×256" },
  { value: "stark_8", label: "Stark 8×8" },
  { value: "yliluoma1_8", label: "Yliluoma 1 (8×8)" },
  { value: "yliluoma2_8", label: "Yliluoma 2 (8×8)" },
  { value: "floyd_steinberg", label: "Floyd–Steinberg" },
  { value: "jarvis_judice_ninke", label: "Jarvis–Judice–Ninke" },
  { value: "stucki", label: "Stucki" },
  { value: "burkes", label: "Burkes" },
  { value: "sierra_3", label: "Sierra-3" },
  { value: "sierra_2", label: "Sierra-2" },
  { value: "sierra_lite", label: "Sierra-Lite" },
  { value: "atkinson", label: "Atkinson" },
  { value: "reduced_atkinson", label: "Reduced Atkinson" },
];

export function AdjustmentsAccordion(props: Props) {
  const {
    expanded,
    onToggle,
    uiState,
    setUiState,
    cycleDithering,
    requestImage,
  } = props;
  const {
    brightness,
    contrast,
    saturation,
    sharpness,
    dithering,
    showIntermediate,
  } = uiState;

  return (
    <Accordion
      expanded={expanded}
      onChange={(_, e) => onToggle(!!e)}
      slotProps={{ root: { style: { margin: 0 } } }}
    >
      <AccordionSummary expandIcon={<ExpandMoreIcon />}>
        <Typography>Adjustments</Typography>
      </AccordionSummary>
      <AccordionDetails>
        <Stack spacing={2} sx={{ px: 1 }}>
          <div>
            <Typography variant="caption" gutterBottom>
              Brightness ({brightness})
            </Typography>
            <Slider
              size="small"
              value={brightness}
              onChange={(_, v) =>
                setUiState({ ...uiState, brightness: v as number })
              }
              min={-50}
              max={50}
              step={5}
              marks
            />
          </div>
          <div>
            <Typography variant="caption" gutterBottom>
              Contrast ({contrast})
            </Typography>
            <Slider
              size="small"
              value={contrast}
              onChange={(_, v) =>
                setUiState({ ...uiState, contrast: v as number })
              }
              min={-50}
              max={50}
              step={5}
              marks
            />
          </div>
          <div>
            <Typography variant="caption" gutterBottom>
              Saturation ({saturation.toFixed(2)})
            </Typography>
            <Slider
              size="small"
              value={saturation}
              onChange={(_, v) =>
                setUiState({ ...uiState, saturation: v as number })
              }
              min={-0.25}
              max={0.25}
              step={0.025}
              marks
            />
          </div>
          <div>
            <Typography variant="caption" gutterBottom>
              Sharpness {sharpness.toFixed(2)} (
              {sharpness < 0 ? "soften" : sharpness > 0 ? "sharpen" : "neutral"}
              )
            </Typography>
            <Slider
              size="small"
              value={sharpness}
              onChange={(_, v) =>
                setUiState({ ...uiState, sharpness: v as number })
              }
              min={-5}
              max={5}
              step={0.5}
              marks
            />
          </div>
          <Stack direction="row" spacing={1} alignItems="flex-end">
            <div className="flex flex-col">
              <Typography variant="caption" gutterBottom>
                Dithering
              </Typography>
              <Stack direction="row" spacing={1} alignItems="center">
                <Select
                  size="small"
                  value={dithering}
                  onChange={(e) =>
                    setUiState({
                      ...uiState,
                      dithering: e.target.value as string,
                    })
                  }
                  sx={{ minWidth: 220 }}
                >
                  {DITHER_OPTIONS.map((opt) => (
                    <MenuItem key={opt.value} value={opt.value}>
                      {opt.label}
                    </MenuItem>
                  ))}
                </Select>
                <IconButton
                  size="small"
                  aria-label="previous dithering"
                  onClick={() => cycleDithering(-1)}
                >
                  <ChevronLeftIcon fontSize="small" />
                </IconButton>
                <IconButton
                  size="small"
                  aria-label="next dithering"
                  onClick={() => cycleDithering(1)}
                >
                  <ChevronRightIcon fontSize="small" />
                </IconButton>
              </Stack>
            </div>
          </Stack>
          <div className="flex items-center gap-2">
            <FormControlLabel
              control={
                <Checkbox
                  size="small"
                  checked={showIntermediate}
                  onChange={(_, c) => {
                    setUiState({ ...uiState, showIntermediate: c });
                    requestImage(c);
                  }}
                />
              }
              label="Show original image"
            />
          </div>
        </Stack>
      </AccordionDetails>
    </Accordion>
  );
}
