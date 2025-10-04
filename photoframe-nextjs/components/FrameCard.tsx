import React, { useState, useEffect, useRef, useCallback } from "react";
import Card from "@mui/material/Card";
import CardContent from "@mui/material/CardContent";
import {
  usePatchFrameMutation,
  useTriggerFrameMutation,
  usePushFrameMutation,
  useNextFrameMutation,
  useUploadFrameMutation,
  usePreviewFrameMutation,
  usePauseFrameMutation,
  useDummyFrameMutation,
  useClearFrameMutation,
  PreviewParams,
  FrameConfig,
  useFlipFrameMutation,
  TimestampPosition,
  TimestampColor,
  TimestampStrokeColor,
} from "../hooks/http";
import {
  FrameHeader,
  ImagePreview,
  AdjustmentsAccordion,
  PaddingAccordion,
  TimestampAccordion,
  MiscSettingsAccordion,
  ActionsBar,
} from "./frame";
import type { FrameUiState } from "./frame";
import { useDebouncedEffect } from "../hooks/useDebouncedEffect";
import { useFramePaletteQuery, useFrameMetadataQuery } from "../hooks/http";
import Dialog from "@mui/material/Dialog";
import DialogTitle from "@mui/material/DialogTitle";
import DialogContent from "@mui/material/DialogContent";
import DialogActions from "@mui/material/DialogActions";
import Button from "@mui/material/Button";

export interface Frame extends FrameConfig {
  id: string;
}

interface Props {
  frame: Frame;
  refresh: () => void;
  apiBase: string;
}

export function FrameCard({ frame, apiBase }: Props) {
  const original = useRef({
    dithering: frame.dithering || "none",
    brightness: frame.adjustments?.brightness ?? 0,
    contrast: frame.adjustments?.contrast ?? 0,
    saturation: frame.adjustments?.saturation ?? 0,
    sharpness: frame.adjustments?.sharpness ?? 0,
  });
  const [uiState, setUiState] = useState<FrameUiState>({
    dithering: original.current.dithering,
    brightness: original.current.brightness,
    contrast: original.current.contrast,
    saturation: original.current.saturation,
    sharpness: original.current.sharpness,
    left: frame.overscan?.left ?? 0,
    right: frame.overscan?.right ?? 0,
    top: frame.overscan?.top ?? 0,
    bottom: frame.overscan?.bottom ?? 0,
    paused: !!frame.paused,
    flip: !!frame.flip,
    dummy: !!frame.dummy,
    showIntermediate: false,
    tab: -1,
    // Timestamp fields
    timestampEnabled: frame.timestamp?.enabled ?? false,
    timestampPosition: frame.timestamp?.position ?? "bottom_right",
    timestampFontSize: frame.timestamp?.font_size ?? 24,
    timestampColor: frame.timestamp?.color ?? "transparent_white_text",
    timestampFullWidthBanner: frame.timestamp?.full_width_banner ?? false,
    timestampBannerHeight: frame.timestamp?.banner_height ?? 40,
    timestampPaddingHorizontal: frame.timestamp?.padding_horizontal ?? 16,
    timestampPaddingVertical: frame.timestamp?.padding_vertical ?? 16,
    timestampStrokeEnabled: frame.timestamp?.stroke_enabled ?? false,
    timestampStrokeWidth: frame.timestamp?.stroke_width ?? 2,
    timestampStrokeColor: frame.timestamp?.stroke_color ?? "auto",
    timestampFormat: frame.timestamp?.format,
  });
  const [previewObjectUrl, setPreviewObjectUrl] = useState<string | null>(null);
  const requestIdRef = useRef(0);
  const [loadingMode, setLoadingMode] = useState<"preview" | null>(null);
  const pauseMutation = usePauseFrameMutation(apiBase, frame.id);
  const dummyMutation = useDummyFrameMutation(apiBase, frame.id);
  const flipMutation = useFlipFrameMutation(apiBase, frame.id);
  const clearMutation = useClearFrameMutation(apiBase, frame.id);

  const patchMutation = usePatchFrameMutation(apiBase, frame.id, {
    onSuccess: (payload) => {
      original.current = payload;
    },
  });

  const previewMutation = usePreviewFrameMutation(apiBase, frame.id);

  const requestImage = useCallback(
    (useIntermediate: boolean) => {
      const payload: PreviewParams = {
        dithering: uiState.dithering,
        brightness: uiState.brightness,
        contrast: uiState.contrast,
        saturation: uiState.saturation,
        sharpness: uiState.sharpness,
        left: uiState.left,
        right: uiState.right,
        top: uiState.top,
        bottom: uiState.bottom,
        paused: uiState.paused,
        timestamp_enabled: uiState.timestampEnabled,
        timestamp_position: uiState.timestampPosition as TimestampPosition,
        timestamp_font_size: uiState.timestampFontSize,
        timestamp_color: uiState.timestampColor as TimestampColor,
        timestamp_full_width_banner: uiState.timestampFullWidthBanner,
        timestamp_banner_height: uiState.timestampBannerHeight,
        timestamp_padding_horizontal: uiState.timestampPaddingHorizontal,
        timestamp_padding_vertical: uiState.timestampPaddingVertical,
        timestamp_stroke_enabled: uiState.timestampStrokeEnabled,
        timestamp_stroke_width: uiState.timestampStrokeWidth,
        timestamp_stroke_color:
          uiState.timestampStrokeColor as TimestampStrokeColor,
        timestamp_format: uiState.timestampFormat,
      };
      const id = ++requestIdRef.current;
      setLoadingMode("preview");
      if (useIntermediate) {
        // fetch intermediate PNG directly
        fetch(
          `${apiBase}/frames/${encodeURIComponent(frame.id)}/intermediate?ts=${Date.now()}`,
        )
          .then((res) => {
            if (!res.ok) throw new Error("Intermediate missing");
            return res.blob();
          })
          .then((blob) => {
            if (id !== requestIdRef.current) return;
            const url = URL.createObjectURL(blob);
            setPreviewObjectUrl((prev) => {
              if (prev) URL.revokeObjectURL(prev);
              return url;
            });
            setLoadingMode(null);
          })
          .catch(() => {
            // fallback to computed preview
            previewMutation.mutate(payload, {
              onSuccess: (blob: Blob) => {
                if (id !== requestIdRef.current) return;
                const url = URL.createObjectURL(blob);
                setPreviewObjectUrl((prev) => {
                  if (prev) URL.revokeObjectURL(prev);
                  return url;
                });
                lastPreviewParams.current = payload;
                setLoadingMode(null);
              },
            });
          });
      } else {
        previewMutation.mutate(payload, {
          onSuccess: (blob: Blob) => {
            if (id !== requestIdRef.current) return; // stale
            const url = URL.createObjectURL(blob);
            setPreviewObjectUrl((prev) => {
              if (prev) URL.revokeObjectURL(prev);
              return url;
            });
            lastPreviewParams.current = payload;
            setLoadingMode(null);
          },
        });
      }
    },
    [uiState, apiBase, frame.id, previewMutation],
  );

  // Keep a stable reference to the latest requestImage
  const requestImageRef = useRef(requestImage);
  requestImageRef.current = requestImage;

  // Kick off an initial preview request on mount so the image shows without user interaction.
  useEffect(() => {
    requestImageRef.current(false);
    // run only once on mount
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const triggerMutation = useTriggerFrameMutation(apiBase, frame.id, {
    onSuccess: () => requestImageRef.current(uiState.showIntermediate),
  });
  const pushMutation = usePushFrameMutation(apiBase, frame.id, {
    onSuccess: () => requestImageRef.current(uiState.showIntermediate),
  });
  const nextMutation = useNextFrameMutation(apiBase, frame.id, {
    onSuccess: () => requestImageRef.current(uiState.showIntermediate),
  });

  const uploadMutation = useUploadFrameMutation(apiBase, frame.id);
  const paletteQuery = useFramePaletteQuery(apiBase, frame.id);
  const metadataQuery = useFrameMetadataQuery(apiBase, frame.id);
  const [metadataOpen, setMetadataOpen] = useState(false);

  const cycleDithering = (delta: number) => {
    const DITHER_VALUES = [
      "none",
      "ordered_bayer_2",
      "ordered_bayer_4",
      "ordered_bayer_8",
      "ordered_blue_256",
      "stark_8",
      "yliluoma1_8",
      "yliluoma2_8",
      "floyd_steinberg",
      "jarvis_judice_ninke",
      "stucki",
      "burkes",
      "sierra_3",
      "sierra_2",
      "sierra_lite",
      "atkinson",
      "reduced_atkinson",
    ];
    const idx = Math.max(
      0,
      DITHER_VALUES.findIndex((o) => o === uiState.dithering),
    );
    const len = DITHER_VALUES.length;
    const next = (((idx + delta) % len) + len) % len;
    setUiState({ ...uiState, dithering: DITHER_VALUES[next] });
  };

  function onUpload(file: File) {
    const form = new FormData();
    form.append("file", file);
    uploadMutation.mutate(form, {
      onSuccess: () => {
        setUiState((prev) => ({ ...prev, paused: true }));
        // Persist paused state immediately to avoid cron overwriting the uploaded base
        try {
          pauseMutation.mutate(true);
        } catch {}
        // refresh the visible image for current mode to avoid stale content
        requestImage(uiState.showIntermediate);
      },
    });
  }
  // Removed context menu; actions hoisted into MiscSettingsAccordion

  const lastPreviewParams = useRef<PreviewParams | null>(null);

  const paramsChanged = (a: PreviewParams, b: PreviewParams | null) => {
    if (!b) return true;
    return (
      a.dithering !== b.dithering ||
      a.brightness !== b.brightness ||
      a.contrast !== b.contrast ||
      a.saturation !== b.saturation ||
      a.sharpness !== b.sharpness ||
      a.left !== b.left ||
      a.right !== b.right ||
      a.top !== b.top ||
      a.bottom !== b.bottom ||
      a.timestamp_enabled !== b.timestamp_enabled ||
      a.timestamp_position !== b.timestamp_position ||
      a.timestamp_font_size !== b.timestamp_font_size ||
      a.timestamp_color !== b.timestamp_color ||
      a.timestamp_full_width_banner !== b.timestamp_full_width_banner ||
      a.timestamp_banner_height !== b.timestamp_banner_height ||
      a.timestamp_padding_horizontal !== b.timestamp_padding_horizontal ||
      a.timestamp_padding_vertical !== b.timestamp_padding_vertical ||
      a.timestamp_stroke_enabled !== b.timestamp_stroke_enabled ||
      a.timestamp_stroke_width !== b.timestamp_stroke_width ||
      a.timestamp_stroke_color !== b.timestamp_stroke_color ||
      a.timestamp_format !== b.timestamp_format
    );
  };

  const queuePreview = useCallback(() => {
    const payload: PreviewParams = {
      dithering: uiState.dithering,
      brightness: uiState.brightness,
      contrast: uiState.contrast,
      saturation: uiState.saturation,
      sharpness: uiState.sharpness,
      left: uiState.left,
      right: uiState.right,
      top: uiState.top,
      bottom: uiState.bottom,
      paused: uiState.paused,
      timestamp_enabled: uiState.timestampEnabled,
      timestamp_position: uiState.timestampPosition as TimestampPosition,
      timestamp_font_size: uiState.timestampFontSize,
      timestamp_color: uiState.timestampColor as TimestampColor,
      timestamp_full_width_banner: uiState.timestampFullWidthBanner,
      timestamp_banner_height: uiState.timestampBannerHeight,
      timestamp_padding_horizontal: uiState.timestampPaddingHorizontal,
      timestamp_padding_vertical: uiState.timestampPaddingVertical,
      timestamp_stroke_enabled: uiState.timestampStrokeEnabled,
      timestamp_stroke_width: uiState.timestampStrokeWidth,
      timestamp_stroke_color:
        uiState.timestampStrokeColor as TimestampStrokeColor,
    };
    // When showing the Original (intermediate) image, avoid auto-calling /preview.
    if (uiState.showIntermediate) return;
    if (!paramsChanged(payload, lastPreviewParams.current)) return;
    requestImageRef.current(false);
  }, [uiState]);

  useDebouncedEffect(
    queuePreview,
    [
      uiState.dithering,
      uiState.brightness,
      uiState.contrast,
      uiState.saturation,
      uiState.sharpness,
      uiState.left,
      uiState.right,
      uiState.top,
      uiState.bottom,
      uiState.paused,
      // Timestamp settings should live-preview without saving
      uiState.timestampEnabled,
      uiState.timestampPosition,
      uiState.timestampFontSize,
      uiState.timestampColor,
      uiState.timestampFullWidthBanner,
      uiState.timestampBannerHeight,
      uiState.timestampPaddingHorizontal,
      uiState.timestampPaddingVertical,
      uiState.timestampStrokeEnabled,
      uiState.timestampStrokeWidth,
      uiState.timestampStrokeColor,
      uiState.timestampFormat,
    ],
    500,
    { leading: true, maxWait: 500 },
  );

  function onCancel() {
    setUiState({
      ...uiState,
      dithering: original.current.dithering,
      brightness: original.current.brightness,
      contrast: original.current.contrast,
      saturation: original.current.saturation,
      sharpness: original.current.sharpness,
      left: frame.overscan?.left ?? 0,
      right: frame.overscan?.right ?? 0,
      top: frame.overscan?.top ?? 0,
      bottom: frame.overscan?.bottom ?? 0,
      paused: !!frame.paused,
      flip: !!frame.flip,
      dummy: !!frame.dummy,
      // Reset timestamp settings to the config values
      timestampEnabled: frame.timestamp?.enabled ?? false,
      timestampPosition: frame.timestamp?.position ?? "bottom_right",
      timestampFontSize: frame.timestamp?.font_size ?? 24,
      timestampColor: frame.timestamp?.color ?? "transparent_white_text",
      timestampFullWidthBanner: frame.timestamp?.full_width_banner ?? false,
      timestampBannerHeight: frame.timestamp?.banner_height ?? 40,
      timestampPaddingHorizontal: frame.timestamp?.padding_horizontal ?? 16,
      timestampPaddingVertical: frame.timestamp?.padding_vertical ?? 16,
      timestampStrokeEnabled: frame.timestamp?.stroke_enabled ?? false,
      timestampStrokeWidth: frame.timestamp?.stroke_width ?? 2,
      timestampStrokeColor: frame.timestamp?.stroke_color ?? "auto",
      timestampFormat: frame.timestamp?.format,
    });
    // No automatic preview if values already match last; queuePreview handles comparison.
    queuePreview();
  }

  const unsaved =
    uiState.dithering !== original.current.dithering ||
    uiState.brightness !== original.current.brightness ||
    uiState.contrast !== original.current.contrast ||
    uiState.saturation !== original.current.saturation ||
    uiState.sharpness !== original.current.sharpness ||
    uiState.left !== (frame.overscan?.left ?? 0) ||
    uiState.right !== (frame.overscan?.right ?? 0) ||
    uiState.top !== (frame.overscan?.top ?? 0) ||
    uiState.bottom !== (frame.overscan?.bottom ?? 0) ||
    uiState.paused !== !!frame.paused ||
    uiState.flip !== !!frame.flip ||
    uiState.dummy !== !!frame.dummy ||
    // Timestamp changes count as unsaved until user clicks Save
    uiState.timestampEnabled !== (frame.timestamp?.enabled ?? false) ||
    uiState.timestampPosition !==
      (frame.timestamp?.position ?? "bottom_right") ||
    uiState.timestampFontSize !== (frame.timestamp?.font_size ?? 24) ||
    uiState.timestampColor !==
      (frame.timestamp?.color ?? "transparent_white_text") ||
    uiState.timestampFullWidthBanner !==
      (frame.timestamp?.full_width_banner ?? false) ||
    uiState.timestampBannerHeight !== (frame.timestamp?.banner_height ?? 40) ||
    uiState.timestampPaddingHorizontal !==
      (frame.timestamp?.padding_horizontal ?? 16) ||
    uiState.timestampPaddingVertical !==
      (frame.timestamp?.padding_vertical ?? 16) ||
    uiState.timestampFormat !== frame.timestamp?.format;

  // Auto-refresh preview every minute to reflect external updates.
  useEffect(() => {
    const interval = setInterval(() => {
      requestImageRef.current(uiState.showIntermediate);
    }, 60_000);
    return () => clearInterval(interval);
  }, [uiState.showIntermediate]);

  // Revoke the created object URL when it changes or on unmount.
  useEffect(() => {
    return () => {
      if (previewObjectUrl) URL.revokeObjectURL(previewObjectUrl);
    };
  }, [previewObjectUrl]);

  return (
    <Card className="flex flex-col h-full">
      <FrameHeader
        title={frame.id}
        unsaved={unsaved}
        paused={uiState.paused}
        dummy={uiState.dummy}
        onOpenMetadata={() => setMetadataOpen(true)}
      />
      <CardContent className="flex flex-col gap-3">
        <ImagePreview
          previewObjectUrl={previewObjectUrl}
          loadingMode={loadingMode}
          alt={frame.id}
        />
        <Dialog
          open={metadataOpen}
          onClose={() => setMetadataOpen(false)}
          fullWidth
          maxWidth="md"
        >
          <DialogTitle>Metadata for {frame.id}</DialogTitle>
          <DialogContent dividers>
            {metadataQuery.isLoading && (
              <p className="text-sm text-gray-500">Loading…</p>
            )}
            {metadataQuery.error && (
              <p className="text-sm text-red-600">Failed to load metadata</p>
            )}
            {metadataQuery.data && (
              <pre className="text-xs whitespace-pre-wrap break-words">
                {JSON.stringify(metadataQuery.data, null, 2)}
              </pre>
            )}
          </DialogContent>
          <DialogActions>
            <Button onClick={() => setMetadataOpen(false)}>Close</Button>
          </DialogActions>
        </Dialog>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            patchMutation.mutate({
              dithering: uiState.dithering,
              brightness: uiState.brightness,
              contrast: uiState.contrast,
              saturation: uiState.saturation,
              sharpness: uiState.sharpness,
              left: uiState.left,
              right: uiState.right,
              top: uiState.top,
              bottom: uiState.bottom,
              paused: uiState.paused,
              dummy: uiState.dummy,
              flip: uiState.flip,
              // Persist timestamp settings on save
              timestamp_enabled: uiState.timestampEnabled,
              timestamp_position:
                uiState.timestampPosition as TimestampPosition,
              timestamp_font_size: uiState.timestampFontSize,
              timestamp_color: uiState.timestampColor as TimestampColor,
              timestamp_full_width_banner: uiState.timestampFullWidthBanner,
              timestamp_banner_height: uiState.timestampBannerHeight,
              timestamp_padding_horizontal: uiState.timestampPaddingHorizontal,
              timestamp_padding_vertical: uiState.timestampPaddingVertical,
              timestamp_stroke_enabled: uiState.timestampStrokeEnabled,
              timestamp_stroke_width: uiState.timestampStrokeWidth,
              timestamp_stroke_color:
                uiState.timestampStrokeColor as TimestampStrokeColor,
              timestamp_format: uiState.timestampFormat,
            });
          }}
          className="flex flex-col gap-3"
        >
          <div className="space-y-2 pb-2">
            <AdjustmentsAccordion
              expanded={uiState.tab === 0}
              onToggle={(e) => setUiState({ ...uiState, tab: e ? 0 : -1 })}
              uiState={uiState}
              setUiState={setUiState}
              cycleDithering={cycleDithering}
              requestImage={requestImage}
            />
            <PaddingAccordion
              expanded={uiState.tab === 1}
              onToggle={(e) => setUiState({ ...uiState, tab: e ? 1 : -1 })}
              uiState={uiState}
              setUiState={setUiState}
            />
            <TimestampAccordion
              expanded={uiState.tab === 2}
              onToggle={(e) => setUiState({ ...uiState, tab: e ? 2 : -1 })}
              uiState={uiState}
              setUiState={(
                next: FrameUiState | ((prev: FrameUiState) => FrameUiState),
              ) => {
                const newState =
                  typeof next === "function" ? next(uiState) : next;
                // Update local UI only; preview will post to /preview via debounced effect
                setUiState(newState);
              }}
            />
            <MiscSettingsAccordion
              expanded={uiState.tab === 3}
              onToggle={(e) => setUiState({ ...uiState, tab: e ? 3 : -1 })}
              uiState={uiState}
              setUiState={(
                next: FrameUiState | ((prev: FrameUiState) => FrameUiState),
              ) => {
                const newState =
                  typeof next === "function" ? next(uiState) : next;
                // apply mutations for toggles when these specific fields change
                if (newState.flip !== uiState.flip)
                  flipMutation.mutate(newState.flip);
                if (newState.paused !== uiState.paused)
                  pauseMutation.mutate(newState.paused);
                if (newState.dummy !== uiState.dummy)
                  dummyMutation.mutate(newState.dummy);
                setUiState(newState);
              }}
              flipPending={flipMutation.isPending}
              pausePending={pauseMutation.isPending}
              dummyPending={dummyMutation.isPending}
              onUpload={onUpload}
              uploadPending={uploadMutation.isPending}
              onClear={() =>
                clearMutation.mutate(undefined, {
                  onSuccess: () =>
                    requestImageRef.current(uiState.showIntermediate),
                })
              }
              clearPending={clearMutation.isPending}
              onTrigger={() => triggerMutation.mutate()}
              triggerPending={triggerMutation.isPending}
              palette={paletteQuery.data}
            />
          </div>
          <ActionsBar
            onRevert={onCancel}
            onSave={() =>
              patchMutation.mutate({
                dithering: uiState.dithering,
                brightness: uiState.brightness,
                contrast: uiState.contrast,
                saturation: uiState.saturation,
                sharpness: uiState.sharpness,
                left: uiState.left,
                right: uiState.right,
                top: uiState.top,
                bottom: uiState.bottom,
                paused: uiState.paused,
                dummy: uiState.dummy,
                flip: uiState.flip,
                // Persist timestamp settings on explicit Save
                timestamp_enabled: uiState.timestampEnabled,
                timestamp_position:
                  uiState.timestampPosition as TimestampPosition,
                timestamp_font_size: uiState.timestampFontSize,
                timestamp_color: uiState.timestampColor as TimestampColor,
                timestamp_full_width_banner: uiState.timestampFullWidthBanner,
                timestamp_banner_height: uiState.timestampBannerHeight,
                timestamp_padding_horizontal:
                  uiState.timestampPaddingHorizontal,
                timestamp_padding_vertical: uiState.timestampPaddingVertical,
                timestamp_stroke_enabled: uiState.timestampStrokeEnabled,
                timestamp_stroke_width: uiState.timestampStrokeWidth,
                timestamp_stroke_color:
                  uiState.timestampStrokeColor as TimestampStrokeColor,
                timestamp_format: uiState.timestampFormat,
              })
            }
            onNext={() => nextMutation.mutate()}
            onPush={() => pushMutation.mutate()}
            saving={patchMutation.isPending}
            nextPending={nextMutation.isPending}
            pushPending={pushMutation.isPending}
          />
        </form>
      </CardContent>
    </Card>
  );
}
