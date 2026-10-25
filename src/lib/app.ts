export const isTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const APP_NAME = "OnionHoster";
export const APP_VERSION = "1.0.0";
export const APP_TAGLINE = "Local services, published through Tor.";
