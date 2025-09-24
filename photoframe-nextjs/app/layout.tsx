"use client";

import "./globals.css";
import { ReactNode, useEffect, useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import CssBaseline from "@mui/material/CssBaseline";
import { ThemeProvider } from "@mui/material/styles";
import { theme } from "../theme";
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
import IconButton from "@mui/material/IconButton";
import MenuIcon from "@mui/icons-material/Menu";
import useMediaQuery from "@mui/material/useMediaQuery";
import Link from "next/link";
import Image from "next/image";
import { usePathname } from "next/navigation";
import PhotoIcon from "@mui/icons-material/Photo";
import StorageIcon from "@mui/icons-material/Storage";

const client = new QueryClient();

const drawerWidth = 220;

function Shell({ children }: { children: ReactNode }) {
  const pathname = usePathname();
  const [mobileOpen, setMobileOpen] = useState(false);
  const isMdDn = useMediaQuery((theme: any) => theme.breakpoints.down("md"));

  const handleDrawerToggle = () => {
    setMobileOpen((o) => !o);
  };
  return (
    <Box sx={{ display: "flex" }}>
      <AppBar position="fixed" sx={{ zIndex: (t) => t.zIndex.drawer + 1 }}>
        <Toolbar>
          {isMdDn && (
            <IconButton
              color="inherit"
              edge="start"
              onClick={handleDrawerToggle}
              sx={{ mr: 2 }}
              aria-label="open drawer"
            >
              <MenuIcon />
            </IconButton>
          )}
          <Link
            href="/"
            className="flex items-center justify-center gap-6 -ml-3"
            style={{ textDecoration: "none", color: "inherit" }}
          >
            <Image
              src="/logo.png"
              alt="photoframe-rs"
              width={36}
              height={36}
              className="pt-1"
            />
            <Typography variant="h6" noWrap component="div">
              photoframe-rs
            </Typography>
          </Link>
        </Toolbar>
      </AppBar>
      {/* Permanent drawer on larger screens, temporary on mobile */}
      {!isMdDn ? (
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
      ) : (
        <Drawer
          variant="temporary"
          open={mobileOpen}
          onClose={handleDrawerToggle}
          ModalProps={{ keepMounted: true }}
          sx={{
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
                  onClick={() => setMobileOpen(false)}
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
                  onClick={() => setMobileOpen(false)}
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
      )}

      <Box component="main" sx={{ flexGrow: 1, width: "100%" }}>
        <Toolbar />
        {children}
      </Box>
    </Box>
  );
}

export default function RootLayout({ children }: { children: ReactNode }) {
  useEffect(() => {
    if (typeof window !== "undefined" && "serviceWorker" in navigator) {
      navigator.serviceWorker.register("/sw.js").catch(() => {});
    }
  }, []);
  return (
    <html lang="en" className="h-full">
      <body className="h-full">
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
