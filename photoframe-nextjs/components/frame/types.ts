export interface FrameUiState {
  dithering: string;
  brightness: number;
  contrast: number;
  saturation: number;
  sharpness: number;
  left: number;
  right: number;
  top: number;
  bottom: number;
  paused: boolean;
  flip: boolean;
  dummy: boolean;
  showIntermediate: boolean;
  tab: number; // -1 closed, 0 adjustments, 1 padding, 2 misc
}

export type SetFrameUiState = (
  next: FrameUiState | ((prev: FrameUiState) => FrameUiState),
) => void;
