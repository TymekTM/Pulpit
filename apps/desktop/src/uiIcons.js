// Chrome icons (menus, toolbars) come from Lucide; tile faces keep Font
// Awesome because the Android client renders the same FA glyphs. Context
// menu items name their icon by the old FA name - this maps them over.
import {
  Check,
  ClipboardPaste,
  Copy,
  CopyPlus,
  Eraser,
  Move,
  Palette,
  Pencil,
  Play,
  Plus,
  Trash2,
  Circle,
} from "lucide-vue-next";

const MAP = {
  "arrows-alt": Move,
  check: Check,
  clone: CopyPlus,
  copy: Copy,
  eraser: Eraser,
  palette: Palette,
  paste: ClipboardPaste,
  pen: Pencil,
  play: Play,
  plus: Plus,
  trash: Trash2,
};

export function uiIcon(name) {
  return MAP[name] || Circle;
}
