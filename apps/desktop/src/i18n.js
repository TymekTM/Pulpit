// UI language. English text is the message KEY (gettext style): every
// user-visible literal is written once, in English, and wrapped in `t()`.
// `locales/en.json` stays empty (a missing key renders as itself),
// `locales/pl.json` maps the English source to Polish, and a key without a
// Polish entry falls back to its English text. Polish lives only in pl.json.
import { createI18n } from "vue-i18n";
import { ref } from "vue";
import { api } from "./api.js";
import en from "./locales/en.json";
import pl from "./locales/pl.json";

export const LANG_CHOICES = ["auto", "pl", "en"];

/// The stored choice ("auto" | "pl" | "en"), else the system locale.
export function resolveLocale(stored, systemLang) {
  if (stored === "pl" || stored === "en") return stored;
  const sys = String(systemLang || "").toLowerCase();
  return sys.startsWith("pl") ? "pl" : "en";
}

function systemLang() {
  return typeof navigator !== "undefined" ? navigator.language : "";
}

// Polish has three plural forms (1 / 2-4 / 5+ and the teens); the pipe
// separated message lists them in that order.
function plPlural(choice, choicesLength) {
  if (choicesLength < 3) return choice === 1 ? 0 : 1;
  if (choice === 1) return 0;
  const mod10 = choice % 10;
  const mod100 = choice % 100;
  return mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14) ? 1 : 2;
}

export const i18n = createI18n({
  legacy: false,
  globalInjection: false,
  locale: resolveLocale("auto", systemLang()),
  fallbackLocale: "en",
  messages: { en, pl },
  pluralRules: { pl: plPlural },
  // English-as-key: format (interpolate / pluralize) the key itself when a
  // locale has no entry for it, and do not log every untranslated string
  fallbackFormat: true,
  missingWarn: false,
  fallbackWarn: false,
  warnHtmlMessage: false,
});

/// The translate function. Imported directly (not via `useI18n`) so
/// components and plain modules share one call form, and tests mount
/// components without installing the plugin.
export const t = i18n.global.t;

/// The stored choice, for the language select.
export const uiLang = ref("auto");

function applyLang(lang) {
  uiLang.value = lang;
  const loc = resolveLocale(lang, systemLang());
  i18n.global.locale.value = loc;
  if (typeof document !== "undefined") document.documentElement.lang = loc;
}

/// Read the persisted choice (editor settings "lang") and switch to it.
/// A failed read keeps the system locale - never blocks the launch.
export async function initUiLang() {
  let choice = "auto";
  try {
    const settings = await api.getSettings();
    if (LANG_CHOICES.includes(settings?.lang)) choice = settings.lang;
  } catch {
    // keep the system locale
  }
  applyLang(choice);
  reportLocale(choice);
}

/// Tell the host the RESOLVED locale: only the webview can resolve
/// `auto` (system locale), and the host stays dumb - it mirrors the
/// answer into the payload-text layer (media/Spotify idle strings).
/// Best effort: a failed report leaves English for this session.
function reportLocale(choice) {
  api.setLang(choice, resolveLocale(choice, systemLang())).catch(() => {});
}

/// Switch the UI language and persist the choice. The switch is optimistic;
/// a failed save is rethrown after reverting so the caller can show it.
export async function setUiLang(lang) {
  if (!LANG_CHOICES.includes(lang)) throw new Error(`unknown language: ${lang}`);
  const prev = uiLang.value;
  applyLang(lang);
  try {
    await api.setLang(lang, resolveLocale(lang, systemLang()));
  } catch (e) {
    applyLang(prev);
    throw e;
  }
}

/// Test helper: switch the rendered locale without touching persistence.
export function setLocaleForTest(locale) {
  i18n.global.locale.value = locale;
}
