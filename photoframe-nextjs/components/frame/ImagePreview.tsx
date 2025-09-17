import React from "react";
import Typography from "@mui/material/Typography";

interface Props {
  previewObjectUrl: string | null;
  loadingMode: "preview" | null;
  alt: string;
}

export function ImagePreview({ previewObjectUrl, loadingMode, alt }: Props) {
  return (
    <div className="overflow-auto p-2 flex items-start justify-center">
      {previewObjectUrl ? (
        // eslint-disable-next-line @next/next/no-img-element
        <img
          src={previewObjectUrl}
          alt={alt}
          className="md:max-w-none md:w-auto md:h-auto"
          draggable={false}
        />
      ) : (
        <Typography variant="body2" color="text.secondary">
          {loadingMode !== null ? "Rendering preview…" : "No preview yet"}
        </Typography>
      )}
    </div>
  );
}
