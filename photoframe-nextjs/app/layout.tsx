"use client";

import "./globals.css";
import { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import CssBaseline from "@mui/material/CssBaseline";
import { ThemeProvider, createTheme } from "@mui/material/styles";
import Box from "@mui/material/Box";
import Drawer from "@mui/material/Drawer";
import Toolbar from "@mui/material/Toolbar";
import List from "@mui/material/List";
import ListItem from "@mui/material/ListItem";
import ListItemButton from "@mui/material/ListItemButton";
import ListItemIcon from "@mui/material/ListItemIcon";
import ListItemText from "@mui/material/ListItemText";
import AppBar from "@mui/material/AppBar";
import Typography from "@mui/material/Typography";
import Link from "next/link";
import { usePathname } from "next/navigation";
import PhotoIcon from "@mui/icons-material/Photo";
import StorageIcon from "@mui/icons-material/Storage";

const client = new QueryClient();
const theme = createTheme({
  palette: {
    mode: "dark",
    background: {
      default: "#121212",
      paper: "#1d1d1f",
    },
  },
});

const drawerWidth = 220;

function Shell({ children }: { children: ReactNode }) {
  const pathname = usePathname();
  return (
    <Box sx={{ display: "flex" }}>
      <AppBar position="fixed" sx={{ zIndex: (t) => t.zIndex.drawer + 1 }}>
        <Toolbar>
          <Typography variant="h6" noWrap component="div">
            photoframe-rs
          </Typography>
        </Toolbar>
      </AppBar>
      <Drawer
        variant="permanent"
        sx={{
          width: drawerWidth,
          flexShrink: 0,
          [`& .MuiDrawer-paper`]: {
            width: drawerWidth,
            boxSizing: "border-box",
          },
        }}
      >
        <Toolbar />
        <Box sx={{ overflow: "auto" }}>
          <List>
            <ListItem disablePadding>
              <ListItemButton
                component={Link}
                href="/frames"
                selected={pathname?.startsWith("/frames") || pathname === "/"}
              >
                <ListItemIcon>
                  <PhotoIcon />
                </ListItemIcon>
                <ListItemText primary="Frames" />
              </ListItemButton>
            </ListItem>
            <ListItem disablePadding>
              <ListItemButton
                component={Link}
                href="/sources"
                selected={pathname?.startsWith("/sources") === true}
              >
                <ListItemIcon>
                  <StorageIcon />
                </ListItemIcon>
                <ListItemText primary="Sources" />
              </ListItemButton>
            </ListItem>
          </List>
        </Box>
      </Drawer>
      <Box component="main" sx={{ flexGrow: 1, p: 3 }}>
        <Toolbar />
        {children}
      </Box>
    </Box>
  );
}

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" className="h-full">
      <body className="h-full bg-neutral-900 text-neutral-100">
        <ThemeProvider theme={theme}>
          <CssBaseline />
          <QueryClientProvider client={client}>
            <Shell>{children}</Shell>
          </QueryClientProvider>
        </ThemeProvider>
      </body>
    </html>
  );
}
