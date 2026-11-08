import { createApp } from "vue";
// solid-only icons: every icon in the editor renders with `fas fa-*`
// (brand glyphs never rendered here - the classes are hardcoded `fas`),
// and skipping all.min.css keeps the brands/regular webfonts out of the
// bundle. fontawesome.min.css carries the per-icon content rules
// (.fa-*:before); the 900-weight @font-face lives in fonts.css, WOFF2
// only (solid.min.css would also pull in ~1.4 MB of EOT/TTF/WOFF/SVG
// fallbacks no webview reads). Tablets style their own icons and do not
// use this CSS.
import "@fortawesome/fontawesome-free/css/fontawesome.min.css";
import "./style.css";
import "./season.css";
import App from "./App.vue";
import { i18n, initUiLang } from "./i18n.js";
import { initSeason } from "./season.js";

// the stored language and the Christmas-mode skin are applied before the
// first paint (initSeason never rejects)
Promise.allSettled([initUiLang(), initSeason()]).finally(() =>
  createApp(App).use(i18n).mount("#app")
);
