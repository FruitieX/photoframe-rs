import React from "react";
import Stack from "@mui/material/Stack";
import Button from "@mui/material/Button";
import UndoIcon from "@mui/icons-material/Undo";
import SaveIcon from "@mui/icons-material/Save";
import SkipNextIcon from "@mui/icons-material/SkipNext";
import SendIcon from "@mui/icons-material/Send";

interface Props {
  onRevert: () => void;
  onSave: () => void;
  onNext: () => void;
  onPush: () => void;
  saving: boolean;
  nextPending: boolean;
  pushPending: boolean;
}

export function ActionsBar({
  onRevert,
  onSave,
  onNext,
  onPush,
  saving,
  nextPending,
  pushPending,
}: Props) {
  return (
    <Stack direction="row" gap={2} flexWrap={"wrap"}>
      <Button
        size="small"
        type="button"
        startIcon={<UndoIcon fontSize="small" />}
        onClick={onRevert}
      >
        Revert
      </Button>
      <Button
        size="small"
        type="button"
        variant="outlined"
        startIcon={<SaveIcon fontSize="small" />}
        onClick={onSave}
        disabled={saving}
      >
        Save configuration
      </Button>
      <div className="flex-1" />
      <Button
        size="small"
        startIcon={<SkipNextIcon fontSize="small" />}
        onClick={onNext}
        disabled={nextPending}
      >
        Next image
      </Button>
      <Button
        size="small"
        type="button"
        variant="contained"
        startIcon={<SendIcon fontSize="small" />}
        onClick={onPush}
        disabled={pushPending}
      >
        Push to frame
      </Button>
    </Stack>
  );
}
