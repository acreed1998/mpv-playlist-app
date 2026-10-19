import { useMemo, type ReactNode } from "react";
import {
  CssBaseline,
  ThemeProvider,
  createTheme,
  useMediaQuery,
} from "@mui/material";

export function AppThemeProvider({ children }: { children: ReactNode }) {
  // Follow the operating system's light/dark setting.
  const prefersDark = useMediaQuery("(prefers-color-scheme: dark)");

  // createTheme builds a new object, so cache it: it only needs rebuilding
  // when the mode changes.
  const theme = useMemo(
    () => createTheme({ palette: { mode: prefersDark ? "dark" : "light" } }),
    [prefersDark],
  );

  return (
    <ThemeProvider theme={theme}>
      <CssBaseline />
      {children}
    </ThemeProvider>
  );
}
