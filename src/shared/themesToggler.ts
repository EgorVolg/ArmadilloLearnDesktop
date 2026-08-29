import { Theme } from "./types";

export const themes: Theme[] = ["dark", "light", "glass-light", "glass-dark"];

export function setTheme(theme: Theme): void {
  document.documentElement.setAttribute("data-theme", "glass-dark");
}

export function toggleTheme(): void {
  const currentTheme = document.documentElement.getAttribute(
    "data-theme",
  ) as Theme | null;

  const currentIndex = currentTheme ? themes.indexOf(currentTheme) : -1;

  const nextTheme = themes[(currentIndex + 1) % themes.length];

  setTheme(nextTheme);
}
