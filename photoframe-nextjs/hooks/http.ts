import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";

// Shared types
export interface FrameAdjustments {
  brightness: number;
  contrast: number;
  saturation: number;
  sharpness: number;
}
export interface Overscan {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

export type TimestampPosition =
  | "bottom_left"
  | "bottom_center"
  | "bottom_right"
  | "top_left"
  | "top_center"
  | "top_right";

export type TimestampColor =
  | "transparent_white_text"
  | "transparent_black_text"
  | "transparent_auto_text"
  | "white_background"
  | "black_background";
export type TimestampStrokeColor = "auto" | "white" | "black";

export interface Timestamp {
  enabled: boolean;
  position?: TimestampPosition;
  font_size?: number;
  color?: TimestampColor;
  full_width_banner: boolean;
  banner_height?: number;
  padding_horizontal?: number;
  padding_vertical?: number;
  stroke_enabled?: boolean;
  stroke_width?: number;
  stroke_color?: TimestampStrokeColor;
  format?: string;
}

export interface FrameConfig {
  dithering?: string;
  adjustments?: FrameAdjustments;
  overscan?: Overscan;
  timestamp?: Timestamp;
  paused?: boolean;
  dummy?: boolean;
  flip?: boolean;
}
export interface ConfigResponse {
  photoframes: Record<string, FrameConfig>;
  sources: Record<string, SourceConfig>;
}

export type SourceKind = "filesystem" | "immich" | string;
export type OrderKind = "random" | "sequential";
export interface FilesystemSourceCfg {
  glob?: string;
  order?: OrderKind;
}
export interface ImmichSourceCfg {
  base_url?: string;
  api_key?: string;
  order?: OrderKind;
  // Arbitrary filters passed to searchAssets endpoint (albumIds, personIds, etc)
  // Can be either a single filter object or an array of filter objects
  filters?: Record<string, unknown> | Record<string, unknown>[];
}
export interface SourceConfig {
  kind: SourceKind;
  filesystem?: FilesystemSourceCfg;
  immich?: ImmichSourceCfg;
}

// Immich onboarding
export function useSetImmichCredentials(apiBase: string, sourceId: string) {
  return useMutation<void, Error, { base_url: string; api_key: string }>({
    mutationFn: async (payload) => {
      const res = await fetch(
        `${apiBase}/sources/${sourceId}/immich/credentials`,
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(payload),
        },
      );
      if (!res.ok) throw new Error("Set Immich creds failed");
    },
  });
}

// (Future) mutation to persist Immich filters could be added here once backend endpoint exists.
export function useSetImmichFilters(apiBase: string, sourceId: string) {
  const qc = useQueryClient();
  return useMutation<
    void,
    Error,
    { filters: Record<string, unknown> | Record<string, unknown>[] }
  >({
    mutationFn: async (payload) => {
      const res = await fetch(`${apiBase}/sources/${sourceId}/immich/filters`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!res.ok) throw new Error("Set Immich filters failed");
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["config", apiBase] });
    },
  });
}

export interface FramePatchPayload extends FrameAdjustments {
  dithering: string;
  left?: number;
  right?: number;
  top?: number;
  bottom?: number;
  paused?: boolean;
  dummy?: boolean;
  flip?: boolean;
  timestamp_enabled?: boolean;
  timestamp_position?: TimestampPosition;
  timestamp_font_size?: number;
  timestamp_color?: TimestampColor;
  timestamp_full_width_banner?: boolean;
  timestamp_banner_height?: number;
  timestamp_padding_horizontal?: number;
  timestamp_padding_vertical?: number;
  timestamp_stroke_enabled?: boolean;
  timestamp_stroke_width?: number;
  timestamp_stroke_color?: TimestampStrokeColor;
  timestamp_format?: string;
}
export interface PreviewParams extends FramePatchPayload {}

const defaultFetch = async <T>(url: string, init?: RequestInit): Promise<T> => {
  const res = await fetch(url, init);
  if (!res.ok)
    throw new Error(`${init?.method || "GET"} ${url} failed (${res.status})`);
  return res.json() as Promise<T>;
};

/** Fetch full server configuration */
export function useConfigQuery(apiBase: string) {
  return useQuery<ConfigResponse>({
    queryKey: ["config", apiBase],
    queryFn: () => defaultFetch<ConfigResponse>(`${apiBase}/config`),
    refetchInterval: 30_000,
  });
}

/** Patch frame adjustments & dithering */
export function usePatchFrameMutation(
  apiBase: string,
  frameId: string,
  opts?: { onSuccess?: (payload: FramePatchPayload) => void },
) {
  const qc = useQueryClient();
  return useMutation<void, Error, FramePatchPayload>({
    mutationFn: async (payload) => {
      const res = await fetch(`${apiBase}/frames/${frameId}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!res.ok) throw new Error("Patch failed");
    },
    onSuccess: (_data, variables) => {
      qc.invalidateQueries({ queryKey: ["config", apiBase] });
      opts?.onSuccess?.(variables);
    },
  });
}

/** Immediate pause/resume toggle (only sends paused flag) */
export function usePauseFrameMutation(apiBase: string, frameId: string) {
  const qc = useQueryClient();
  return useMutation<void, Error, boolean>({
    mutationFn: async (next) => {
      const res = await fetch(`${apiBase}/frames/${frameId}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ paused: next }),
      });
      if (!res.ok) throw new Error("Pause toggle failed");
    },
    onSuccess: () => {
      qc.invalidateQueries();
    },
  });
}

/** Immediate dummy toggle (only sends dummy flag) */
export function useDummyFrameMutation(apiBase: string, frameId: string) {
  const qc = useQueryClient();
  return useMutation<void, Error, boolean>({
    mutationFn: async (next) => {
      const res = await fetch(`${apiBase}/frames/${frameId}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ dummy: next }),
      });
      if (!res.ok) throw new Error("Dummy toggle failed");
    },
    onSuccess: () => {
      qc.invalidateQueries();
    },
  });
}

/** Immediate 180° flip toggle (only sends flip flag) */
export function useFlipFrameMutation(apiBase: string, frameId: string) {
  const qc = useQueryClient();
  return useMutation<void, Error, boolean>({
    mutationFn: async (next) => {
      const res = await fetch(`${apiBase}/frames/${frameId}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ flip: next }),
      });
      if (!res.ok) throw new Error("Flip toggle failed");
    },
    onSuccess: () => {
      qc.invalidateQueries();
    },
  });
}

/** Trigger frame to advance to next image */
export function useTriggerFrameMutation(
  apiBase: string,
  frameId: string,
  opts?: { onSuccess?: () => void },
) {
  return useMutation<void, Error, void>({
    mutationFn: async () => {
      const res = await fetch(`${apiBase}/frames/${frameId}/trigger`, {
        method: "POST",
      });
      if (!res.ok) throw new Error("Trigger failed");
    },
    onSuccess: () => opts?.onSuccess?.(),
  });
}

/** Select next image for the frame without pushing to device */
export function useNextFrameMutation(
  apiBase: string,
  frameId: string,
  opts?: { onSuccess?: () => void },
) {
  return useMutation<void, Error, void>({
    mutationFn: async () => {
      const res = await fetch(`${apiBase}/frames/${frameId}/next`, {
        method: "POST",
      });
      if (!res.ok) throw new Error("Next failed");
    },
    onSuccess: () => opts?.onSuccess?.(),
  });
}

/** Upload a custom image */
export function useUploadFrameMutation(apiBase: string, frameId: string) {
  const previewMutation = usePreviewFrameMutation(apiBase, frameId);

  return useMutation<void, Error, FormData>({
    mutationFn: async (form) => {
      const res = await fetch(`${apiBase}/frames/${frameId}/upload`, {
        method: "POST",
        body: form,
      });
      if (!res.ok) throw new Error("Upload failed");
    },
    onSuccess: () => {
      // Force a refetch of preview immediately after upload
      previewMutation.mutate(undefined);
    },
  });
}

/** Generate live preview of frame with current adjustments */
export function usePreviewFrameMutation(apiBase: string, frameId: string) {
  return useMutation<Blob, Error, PreviewParams | undefined>({
    mutationFn: async (payload) => {
      const res = await fetch(`${apiBase}/frames/${frameId}/preview`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!res.ok) throw new Error("Preview failed");
      return res.blob();
    },
  });
}

export interface FramePaletteEntry {
  input: string;
  hex: string;
  rgb: [number, number, number];
}
export interface FramePaletteResponse {
  frame_id: string;
  palette: FramePaletteEntry[];
}

export function useFramePaletteQuery(apiBase: string, frameId: string) {
  return useQuery<FramePaletteResponse>({
    queryKey: ["palette", apiBase, frameId],
    queryFn: () =>
      defaultFetch<FramePaletteResponse>(
        `${apiBase}/frames/${encodeURIComponent(frameId)}/palette`,
      ),
    refetchInterval: 30_000,
  });
}

/** Clear the device screen to white */
export function useClearFrameMutation(apiBase: string, frameId: string) {
  return useMutation<void, Error, void>({
    mutationFn: async () => {
      const res = await fetch(`${apiBase}/frames/${frameId}/clear`, {
        method: "POST",
      });
      if (!res.ok) throw new Error("Clear failed");
    },
  });
}
