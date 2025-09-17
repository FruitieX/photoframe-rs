import React from "react";
import Accordion from "@mui/material/Accordion";
import AccordionSummary from "@mui/material/AccordionSummary";
import AccordionDetails from "@mui/material/AccordionDetails";
import ExpandMoreIcon from "@mui/icons-material/ExpandMore";
import Typography from "@mui/material/Typography";
import Stack from "@mui/material/Stack";
import Slider from "@mui/material/Slider";
import type { FrameUiState, SetFrameUiState } from "./types";

interface Props {
  expanded: boolean;
  onToggle: (expanded: boolean) => void;
  uiState: FrameUiState;
  setUiState: SetFrameUiState;
}

export function PaddingAccordion({
  expanded,
  onToggle,
  uiState,
  setUiState,
}: Props) {
  const { left, right, top, bottom } = uiState;
  return (
    <Accordion expanded={expanded} onChange={(_, e) => onToggle(!!e)}>
      <AccordionSummary expandIcon={<ExpandMoreIcon />}>
        <Typography>Padding</Typography>
      </AccordionSummary>
      <AccordionDetails>
        <Stack spacing={2} sx={{ px: 1 }}>
          <div>
            <Typography variant="caption" gutterBottom>
              Left ({left})
            </Typography>
            <Slider
              size="small"
              value={left}
              onChange={(_, v) => setUiState({ ...uiState, left: v as number })}
              min={0}
              max={200}
              step={1}
            />
          </div>
          <div>
            <Typography variant="caption" gutterBottom>
              Right ({right})
            </Typography>
            <Slider
              size="small"
              value={right}
              onChange={(_, v) =>
                setUiState({ ...uiState, right: v as number })
              }
              min={0}
              max={200}
              step={1}
            />
          </div>
          <div>
            <Typography variant="caption" gutterBottom>
              Top ({top})
            </Typography>
            <Slider
              size="small"
              value={top}
              onChange={(_, v) => setUiState({ ...uiState, top: v as number })}
              min={0}
              max={200}
              step={1}
            />
          </div>
          <div>
            <Typography variant="caption" gutterBottom>
              Bottom ({bottom})
            </Typography>
            <Slider
              size="small"
              value={bottom}
              onChange={(_, v) =>
                setUiState({ ...uiState, bottom: v as number })
              }
              min={0}
              max={200}
              step={1}
            />
          </div>
        </Stack>
      </AccordionDetails>
    </Accordion>
  );
}
