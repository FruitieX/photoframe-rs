import React, { useState, useEffect, useRef, useCallback } from "react";
import Card from "@mui/material/Card";
import CardContent from "@mui/material/CardContent";
import CardHeader from "@mui/material/CardHeader";
import Button from "@mui/material/Button";
import Slider from "@mui/material/Slider";
import Stack from "@mui/material/Stack";
import Typography from "@mui/material/Typography";
import Select from "@mui/material/Select";
import MenuItem from "@mui/material/MenuItem";
import LinearProgress from "@mui/material/LinearProgress";
import Chip from "@mui/material/Chip";
import {
  usePatchFrameMutation,
  useTriggerFrameMutation,
  useNextFrameMutation,
  useUploadFrameMutation,
  usePreviewFrameMutation,
  usePauseFrameMutation,
  useDummyFrameMutation,
  useClearFrameMutation,
  FramePatchPayload,
  PreviewParams,
  FrameConfig,
  useFlipFrameMutation,
} from "../hooks/http";
import Tabs from "@mui/material/Tabs";
import Tab from "@mui/material/Tab";
import Checkbox from "@mui/material/Checkbox";
import FormControlLabel from "@mui/material/FormControlLabel";
import IconButton from "@mui/material/IconButton";
import Menu from "@mui/material/Menu";
import MoreVertIcon from "@mui/icons-material/MoreVert";
import ChevronLeftIcon from "@mui/icons-material/ChevronLeft";
import ChevronRightIcon from "@mui/icons-material/ChevronRight";
import { useDebouncedEffect } from "../hooks/useDebouncedEffect";
import { useFramePaletteQuery } from "../hooks/http";

export interface Frame extends FrameConfig {
  id: string;
}

interface Props {
  frame: Frame;
  refresh: () => void;
  apiBase: string;
}

export function FrameCard({ frame, refresh, apiBase }: Props) {
  const original = useRef({
    dithering: frame.dithering || "none",
    brightness: frame.adjustments?.brightness ?? 0,
    contrast: frame.adjustments?.contrast ?? 0,
    saturation: frame.adjustments?.saturation ?? 0,
    sharpness: frame.adjustments?.sharpness ?? 0,
  });
  const [dithering, setDithering] = useState(original.current.dithering);
  const [brightness, setBrightness] = useState(original.current.brightness);
  const [contrast, setContrast] = useState(original.current.contrast);
  const [saturation, setSaturation] = useState(original.current.saturation);
  const [sharpness, setSharpness] = useState(original.current.sharpness);
  const [previewObjectUrl, setPreviewObjectUrl] = useState<string | null>(null);
  const [tab, setTab] = useState(0);
  const [paused, setPaused] = useState<boolean>(!!frame.paused);
  const [flip, setFlip] = useState<boolean>(!!(frame as any).flip);
  const [showIntermediate, setShowIntermediate] = useState<boolean>(false);
  const [dummy, setDummy] = useState<boolean>(!!(frame as any).dummy);
  const requestIdRef = useRef(0);
  const [loadingMode, setLoadingMode] = useState<"preview" | null>(null);
  // Overscan (padding)
  const [left, setLeft] = useState(frame.overscan?.left ?? 0);
  const [right, setRight] = useState(frame.overscan?.right ?? 0);
  const [top, setTop] = useState(frame.overscan?.top ?? 0);
  const [bottom, setBottom] = useState(frame.overscan?.bottom ?? 0);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const [menuAnchor, setMenuAnchor] = useState<null | HTMLElement>(null);
  const menuOpen = !!menuAnchor;
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

  const requestImage = useCallback((useIntermediate: boolean) => {
    const payload: PreviewParams = {
      dithering,
      brightness,
      contrast,
      saturation,
      sharpness,
      left,
      right,
      top,
      bottom,
  paused,
    };
    const id = ++requestIdRef.current;
    setLoadingMode("preview");
    if (useIntermediate) {
      // fetch intermediate PNG directly
      fetch(`${apiBase}/frames/${encodeURIComponent(frame.id)}/intermediate`)
        .then((res) => {
          if (!res.ok) throw new Error("Intermediate missing");
          return res.blob();
        })
        .then((blob) => {
          if (id !== requestIdRef.current) return;
          if (previewObjectUrl) URL.revokeObjectURL(previewObjectUrl);
          const url = URL.createObjectURL(blob);
          setPreviewObjectUrl(url);
          setLoadingMode(null);
        })
        .catch(() => {
          // fallback to computed preview
          previewMutation.mutate(payload, {
            onSuccess: (blob: Blob) => {
              if (id !== requestIdRef.current) return;
              if (previewObjectUrl) URL.revokeObjectURL(previewObjectUrl);
              const url = URL.createObjectURL(blob);
              setPreviewObjectUrl(url);
              lastPreviewParams.current = payload;
              setLoadingMode(null);
            },
          } as any);
        });
    } else {
      previewMutation.mutate(payload, {
        onSuccess: (blob: Blob) => {
          if (id !== requestIdRef.current) return; // stale
          if (previewObjectUrl) URL.revokeObjectURL(previewObjectUrl);
          const url = URL.createObjectURL(blob);
          setPreviewObjectUrl(url);
          lastPreviewParams.current = payload;
          setLoadingMode(null);
        },
      } as any);
    }
  }, [
    dithering,
    brightness,
    contrast,
    saturation,
    sharpness,
    left,
    right,
    top,
    bottom,
    paused,
    apiBase,
    frame.id,
    previewMutation,
    previewObjectUrl,
  ]);

  const triggerMutation = useTriggerFrameMutation(apiBase, frame.id, {
    onSuccess: () => requestImage(showIntermediate),
  });
  const nextMutation = useNextFrameMutation(apiBase, frame.id, {
    onSuccess: () => requestImage(showIntermediate),
  });

  const uploadMutation = useUploadFrameMutation(apiBase, frame.id);
  const paletteQuery = useFramePaletteQuery(apiBase, frame.id);

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

  const cycleDithering = (delta: number) => {
    const idx = Math.max(
      0,
      DITHER_OPTIONS.findIndex((o) => o.value === dithering),
    );
    const len = DITHER_OPTIONS.length;
    const next = ((idx + delta) % len + len) % len; // wrap-around
    setDithering(DITHER_OPTIONS[next].value);
  };

  function onUpload(e: React.ChangeEvent<HTMLInputElement>) {
    if (!e.target.files?.length) return;
    const file = e.target.files[0];
    const form = new FormData();
    form.append("file", file);
    uploadMutation.mutate(form, {
      onSuccess: () => {
        setPaused(true);
        // Persist paused state immediately to avoid cron overwriting the uploaded base
        try { pauseMutation.mutate(true); } catch {}
        // refresh the visible image for current mode to avoid stale content
        requestImage(showIntermediate);
      },
    });
  }
  function openMenu(e: React.MouseEvent<HTMLElement>) {
    setMenuAnchor(e.currentTarget);
  }
  function closeMenu() {
    setMenuAnchor(null);
  }
  function triggerUploadDialog() {
    fileInputRef.current?.click();
  }

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
      a.bottom !== b.bottom
    );
  };

  const queuePreview = useCallback(() => {
    const payload: PreviewParams = {
      dithering,
      brightness,
      contrast,
      saturation,
      sharpness,
      left,
      right,
      top,
      bottom,
      paused,
    };
  // When showing the Original (intermediate) image, avoid auto-calling /preview.
  if (showIntermediate) return;
    if (!paramsChanged(payload, lastPreviewParams.current)) return;
  requestImage(false);
  }, [
    dithering,
    brightness,
    contrast,
    saturation,
    sharpness,
    left,
    right,
    top,
    bottom,
    paused,
  showIntermediate,
    requestImage,
  ]);

  useDebouncedEffect(
    queuePreview,
    [
      dithering,
      brightness,
      contrast,
      saturation,
      sharpness,
      left,
      right,
      top,
      bottom,
  paused,
    ],
  500,
  { leading: true, maxWait: 500 },
  );

  function onCancel() {
    setDithering(original.current.dithering);
    setBrightness(original.current.brightness);
    setContrast(original.current.contrast);
    setSaturation(original.current.saturation);
    setSharpness(original.current.sharpness);
    setLeft(frame.overscan?.left ?? 0);
    setRight(frame.overscan?.right ?? 0);
    setTop(frame.overscan?.top ?? 0);
    setBottom(frame.overscan?.bottom ?? 0);
  setPaused(!!frame.paused);
  setFlip(!!(frame as any).flip);
  setDummy(!!(frame as any).dummy);
    // No automatic preview if values already match last; queuePreview handles comparison.
    queuePreview();
  }

  const unsaved =
    dithering !== original.current.dithering ||
    brightness !== original.current.brightness ||
    contrast !== original.current.contrast ||
    saturation !== original.current.saturation ||
    sharpness !== original.current.sharpness ||
    left !== (frame.overscan?.left ?? 0) ||
    right !== (frame.overscan?.right ?? 0) ||
    top !== (frame.overscan?.top ?? 0) ||
  bottom !== (frame.overscan?.bottom ?? 0) ||
  paused !== !!frame.paused ||
  flip !== !!(frame as any).flip ||
  dummy !== !!(frame as any).dummy;

  return (
    <Card className="flex flex-col h-full">
      <CardHeader
        title={frame.id}
        action={
          <Stack direction="row" spacing={1} alignItems="center">
            {unsaved && <Chip size="small" color="warning" label="Unsaved" />}
            <IconButton
              size="small"
              onClick={openMenu}
              aria-label="frame actions"
            >
              <MoreVertIcon fontSize="small" />
            </IconButton>
            <Menu
              anchorEl={menuAnchor}
              open={menuOpen}
              onClose={closeMenu}
              keepMounted
            >
              <MenuItem
                onClick={(e) => {
                  e.stopPropagation();
                  const next = !paused;
                  setPaused(next);
                  pauseMutation.mutate(next);
                }}
              >
                <FormControlLabel
                  onClick={(e) => e.stopPropagation()}
                  control={
                    <Checkbox
                      size="small"
                      checked={paused}
                      onChange={(_, c) => {
                        setPaused(c);
                        pauseMutation.mutate(c);
                      }}
                    />
                  }
                  label={
                    pauseMutation.isPending ? "Updating…" : "Pause schedule"
                  }
                />
              </MenuItem>
              <MenuItem
                onClick={(e) => {
                  e.stopPropagation();
                  const next = !dummy;
                  setDummy(next);
                  dummyMutation.mutate(next);
                }}
              >
                <FormControlLabel
                  onClick={(e) => e.stopPropagation()}
                  control={
                    <Checkbox
                      size="small"
                      checked={dummy}
                      onChange={(_, c) => {
                        setDummy(c);
                        dummyMutation.mutate(c);
                      }}
                    />
                  }
                  label={dummyMutation.isPending ? "Updating…" : "Dummy mode"}
                />
              </MenuItem>
              <MenuItem
                onClick={() => {
                  triggerUploadDialog();
                }}
                disabled={uploadMutation.isPending}
              >
                Upload image
              </MenuItem>
              <MenuItem
                onClick={(e) => {
                  e.stopPropagation();
                  const next = !flip;
                  setFlip(next);
                  flipMutation.mutate(next);
                }}
              >
                <FormControlLabel
                  onClick={(e) => e.stopPropagation()}
                  control={
                    <Checkbox
                      size="small"
                      checked={flip}
                      onChange={(_, c) => {
                        setFlip(c);
                        flipMutation.mutate(c);
                      }}
                    />
                  }
                  label={flipMutation.isPending ? "Updating…" : "Flip 180°"}
                />
              </MenuItem>
              <MenuItem
                onClick={() => {
                  nextMutation.mutate();
                  closeMenu();
                }}
                disabled={nextMutation.isPending}
              >
                Next image
              </MenuItem>
              <MenuItem
                onClick={() => {
                  clearMutation.mutate(undefined, { onSuccess: () => requestImage(showIntermediate) });
                  closeMenu();
                }}
                disabled={clearMutation.isPending}
              >
                Clear screen
              </MenuItem>
            </Menu>
            <input
              ref={fileInputRef}
              hidden
              type="file"
              accept="image/*"
              onChange={onUpload}
            />
            {uploadMutation.isPending && <LinearProgress sx={{ width: 60 }} />}
          </Stack>
        }
      />
      <CardContent className="flex flex-col gap-3">
        <div className="overflow-auto p-2 flex items-start justify-start">
          {previewObjectUrl ? (
            <img
              src={previewObjectUrl}
              alt={frame.id}
              className="max-w-none w-auto h-auto select-none"
              draggable={false}
              onLoad={() => {
                // revoke old URL after image uses it to free memory
              }}
            />
          ) : (
            <Typography variant="body2" color="text.secondary">
              {loadingMode !== null ? "Rendering preview…" : "No preview yet"}
            </Typography>
          )}
        </div>
        {paletteQuery.data && (
          <div className="p-2">
            <Typography variant="caption" gutterBottom>
              Resolved palette
            </Typography>
            <div className="flex flex-wrap gap-6 mt-1">
              {paletteQuery.data.palette.map((p, idx) => (
                <div key={idx} className="flex items-center gap-2">
                  <div
                    className="w-6 h-6 rounded border"
                    style={{ backgroundColor: p.hex !== "invalid" ? p.hex : "#000000" }}
                    title={p.hex}
                  />
                  <Typography variant="caption">
                    {p.input} → {p.hex} ({p.rgb[0]},{p.rgb[1]},{p.rgb[2]})
                  </Typography>
                </div>
              ))}
            </div>
          </div>
        )}
        <div className="flex items-center gap-2 px-2">
          <FormControlLabel
            control={
              <Checkbox
                size="small"
                checked={showIntermediate}
                onChange={(_, c) => {
                  setShowIntermediate(c);
                  // fetch immediately for the chosen mode
                  requestImage(c);
                }}
              />
            }
            label="Original image"
          />
        </div>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            patchMutation.mutate({
              dithering,
              brightness,
              contrast,
              saturation,
              sharpness,
              left,
              right,
              top,
              bottom,
              paused,
              dummy,
              flip,
            });
          }}
          className="flex flex-col gap-3"
        >
          <Tabs value={tab} onChange={(_, v) => setTab(v)} variant="fullWidth">
            <Tab label="Adjustments" />
            <Tab label="Padding" />
          </Tabs>
          {tab === 0 && (
            <>
              <Stack direction="row" spacing={1} alignItems="flex-end">
                <div className="flex flex-col">
                  <Typography variant="caption" gutterBottom>
                    Dithering
                  </Typography>
                  <Stack direction="row" spacing={1} alignItems="center">
                    <Select
                      size="small"
                      value={dithering}
                      onChange={(e: any) => setDithering(e.target.value as string)}
                      sx={{ minWidth: 220 }}
                    >
                      {DITHER_OPTIONS.map((opt) => (
                        <MenuItem key={opt.value} value={opt.value}>
                          {opt.label}
                        </MenuItem>
                      ))}
                    </Select>
                    <IconButton size="small" aria-label="previous dithering" onClick={() => cycleDithering(-1)}>
                      <ChevronLeftIcon fontSize="small" />
                    </IconButton>
                    <IconButton size="small" aria-label="next dithering" onClick={() => cycleDithering(1)}>
                      <ChevronRightIcon fontSize="small" />
                    </IconButton>
                  </Stack>
                </div>
              </Stack>
              <Stack spacing={2} sx={{ px: 1 }}>
                <div>
                  <Typography variant="caption" gutterBottom>
                    Brightness ({brightness})
                  </Typography>
                  <Slider
                    size="small"
                    value={brightness}
                    onChange={(_, v) => setBrightness(v as number)}
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
                    onChange={(_, v) => setContrast(v as number)}
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
                    onChange={(_, v) => setSaturation(v as number)}
                    min={-0.25}
                    max={0.25}
                    step={0.025}
                    marks
                  />
                </div>
                <div>
                  <Typography variant="caption" gutterBottom>
                    Sharpness {sharpness.toFixed(2)} (
                    {sharpness < 0
                      ? "soften"
                      : sharpness > 0
                        ? "sharpen"
                        : "neutral"}
                    )
                  </Typography>
                  <Slider
                    size="small"
                    value={sharpness}
                    onChange={(_, v) => setSharpness(v as number)}
                    min={-5}
                    max={5}
                    step={0.5}
                    marks
                  />
                </div>
              </Stack>
            </>
          )}
          {tab === 1 && (
            <Stack spacing={2} sx={{ px: 1 }}>
              <div>
                <Typography variant="caption" gutterBottom>
                  Left ({left})
                </Typography>
                <Slider
                  size="small"
                  value={left}
                  onChange={(_, v) => setLeft(v as number)}
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
                  onChange={(_, v) => setRight(v as number)}
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
                  onChange={(_, v) => setTop(v as number)}
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
                  onChange={(_, v) => setBottom(v as number)}
                  min={0}
                  max={200}
                  step={1}
                />
              </div>
            </Stack>
          )}
          {/* Actions moved into More menu */}
          <Stack direction="row" spacing={2}>
            <Button
              size="small"
              type="submit"
              variant="outlined"
              disabled={patchMutation.isPending}
            >
              Save
            </Button>
            <Button size="small" type="button" onClick={onCancel}>
              Cancel
            </Button>
            <Button
              size="small"
              type="button"
              variant="contained"
              onClick={() => triggerMutation.mutate()}
              disabled={triggerMutation.isPending}
            >
              Update
            </Button>
          </Stack>
        </form>
      </CardContent>
    </Card>
  );
}
