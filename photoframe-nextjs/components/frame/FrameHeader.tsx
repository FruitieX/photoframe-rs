import React from "react";
import CardHeader from "@mui/material/CardHeader";
import Stack from "@mui/material/Stack";
import Chip from "@mui/material/Chip";
import PauseCircleIcon from "@mui/icons-material/PauseCircle";
import BuildCircleIcon from "@mui/icons-material/BuildCircle";

interface Props {
  title: string;
  unsaved: boolean;
  paused: boolean;
  dummy: boolean;
  onOpenMetadata?: () => void;
}

export function FrameHeader({
  title,
  unsaved,
  paused,
  dummy,
  onOpenMetadata,
}: Props) {
  return (
    <CardHeader
      title={title}
      action={
        <Stack direction="row" spacing={1} alignItems="center">
          {unsaved && <Chip size="small" color="warning" label="Unsaved" />}
          {paused && (
            <Chip
              size="small"
              color="default"
              icon={<PauseCircleIcon fontSize="small" />}
              label="Paused"
            />
          )}
          {dummy && (
            <Chip
              size="small"
              color="default"
              icon={<BuildCircleIcon fontSize="small" />}
              label="Dummy"
            />
          )}
          {!!onOpenMetadata && (
            <Chip
              size="small"
              color="primary"
              label="Metadata"
              onClick={onOpenMetadata}
              clickable
            />
          )}
        </Stack>
      }
    />
  );
}
