import { alpha, createTheme } from "@mui/material/styles";

// Dark-only custom theme with a distinctive cyan/violet accent palette
const base = createTheme({
  palette: {
    mode: "dark",
    // Orange-forward accents
    primary: { main: "#f97316" }, // orange-500
    secondary: { main: "#fb923c" }, // orange-400
    success: { main: "#22c55e" }, // green-500
    warning: { main: "#f59e0b" }, // amber-500
    error: { main: "#f43f5e" }, // rose-500
    info: { main: "#fdba74" }, // orange-300
    background: {
      // Slightly lighter dark backgrounds for improved contrast
      default: "#1c2024", // a touch lighter than #0b0f14
      paper: "#1a1a1a", // lighter neutral paper for cards
    },
    divider: "rgba(148,163,184,0.24)", // slate-400 @ ~24%
  },
  shape: {
    borderRadius: 10,
  },
  typography: {
    // Keep it simple; match system font stack for performance
    fontFamily:
      'ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial, "Apple Color Emoji", "Segoe UI Emoji"',
  },
});

export const theme = createTheme(base, {
  components: {
    MuiCssBaseline: {
      styleOverrides: {
        body: {
          backgroundColor: base.palette.background.default,
          color: base.palette.text.primary,
        },
      },
    },
    MuiAppBar: {
      styleOverrides: {
        colorPrimary: {
          backgroundColor: base.palette.background.default,
          borderBottom: `1px solid ${alpha(base.palette.common.white, 0.08)}`,
          color: base.palette.text.primary,
        },
      },
    },
    MuiDrawer: {
      styleOverrides: {
        paper: {
          backgroundColor: base.palette.background.default,
          borderRight: `1px solid ${alpha(base.palette.common.white, 0.08)}`,
        },
      },
    },
    MuiPaper: {
      defaultProps: {
        elevation: 0,
        variant: "outlined",
      },
      styleOverrides: {
        root: {
          backgroundImage: "none",
          borderColor: alpha(base.palette.common.white, 0.3),
        },
      },
    },
    MuiCard: {
      defaultProps: {
        variant: "outlined",
      },
      styleOverrides: {
        root: {
          borderRadius: base.shape.borderRadius,
          backgroundColor: base.palette.background.paper,
          borderColor: alpha(base.palette.common.white, 0.18),
        },
      },
    },
    MuiListItemButton: {
      styleOverrides: {
        root: {
          borderRadius: 8,
          "&.Mui-selected": {
            backgroundColor: alpha(base.palette.primary.main, 0.16),
            "&:hover": {
              backgroundColor: alpha(base.palette.primary.main, 0.24),
            },
          },
          "&:hover": {
            backgroundColor: alpha(base.palette.common.white, 0.04),
          },
        },
      },
    },
    MuiButton: {
      defaultProps: {
        disableElevation: true,
      },
      styleOverrides: {
        root: {
          textTransform: "none",
          borderRadius: 8,
        },
        containedPrimary: {
          color: base.palette.getContrastText(base.palette.primary.main),
        },
        outlined: {
          borderColor: alpha(base.palette.primary.main, 0.4),
        },
      },
    },
    MuiTabs: {
      styleOverrides: {
        root: {
          minHeight: 40,
        },
        indicator: {
          height: 3,
          borderRadius: 1,
          backgroundColor: base.palette.primary.main,
        },
      },
    },
    MuiTab: {
      styleOverrides: {
        root: {
          textTransform: "none",
          minHeight: 40,
        },
      },
    },
    MuiDivider: {
      styleOverrides: {
        root: {
          borderColor: alpha(base.palette.common.white, 0.08),
        },
      },
    },
  },
});

export type AppTheme = typeof theme;
