import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  // vue-i18n: interpret messages from an AST instead of compiling them with
  // `new Function` (the webview CSP has no 'unsafe-eval'), and drop the
  // legacy Options API surface we do not use
  define: {
    __INTLIFY_JIT_COMPILATION__: true,
    __VUE_I18N_LEGACY_API__: false,
    __VUE_I18N_FULL_INSTALL__: false,
  },
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "es2021" },
  // `npx vitest run`: unit tests under tests/, DOM from happy-dom; the
  // Tauri bridge is mocked per test (tests/setup.js stubs invoke)
  test: {
    environment: "happy-dom",
    include: ["tests/**/*.test.js"],
    setupFiles: ["tests/setup.js"],
    restoreMocks: true,
  },
});
