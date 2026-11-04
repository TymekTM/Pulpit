<script setup>
import { computed, reactive, ref } from "vue";
import { ask } from "../confirm";
import { api } from "../api";
import { MAX_BOARD_DIM, boardDim } from "../catalog";
import { t } from "../i18n";

const props = defineProps({
  mode: { type: String, required: true }, // 'create' | 'edit'
  board: { type: Object, default: null },
});
const emit = defineEmits(["close", "saved"]);

const form = reactive({
  name: props.board?.name ?? "",
  width: props.board?.width ?? 6,
  height: props.board?.height ?? 4,
  background: props.board?.background ?? "#437072",
});

// quick picks for the board ground: the default teal-slate plus muted
// darks that keep any tile color readable on top
const PRESETS = ["#437072", "#34495e", "#2c3e50", "#22313c", "#3b3355", "#4a2f3a", "#2f4a3a", "#3d3d3d"];

// preview stays readable for huge boards: the miniature caps at 16x10
const previewW = computed(() => Math.min(16, Math.max(1, boardDim(form.width, 6))));
const previewH = computed(() => Math.min(10, Math.max(1, boardDim(form.height, 4))));

// Overlay click only closes when nothing was edited (012 A9); the same
// ask()-confirmation the board context menu uses guards Clear/Delete.
const initialSnapshot = JSON.stringify({ ...form });
const dirty = computed(() => JSON.stringify({ ...form }) !== initialSnapshot);

function overlayClose() {
  if (!dirty.value) emit("close");
}

// the backend reasons are shown as-is (duplicate name, refused dimensions,
// locked db); the dialog stays open so the entered values survive
const error = ref("");

async function save() {
  // clamp dimensions to the backend's integer bounds (012 C4): the
  // number input's min/max only steer the spinner, they do not validate
  // pasted or typed values
  const width = boardDim(form.width, props.board?.width ?? 6);
  const height = boardDim(form.height, props.board?.height ?? 4);
  error.value = "";
  try {
    if (props.mode === "create") {
      const name = form.name || t("New board");
      const id = await api.createBoard(name, form.background, width, height);
      emit("saved", {
        mode: "create",
        id,
        name,
        background: form.background,
        width,
        height,
      });
    } else {
      const next = {
        ...props.board,
        name: form.name,
        width,
        height,
        background: form.background,
      };
      await api.updateBoard(next);
      emit("saved", { mode: "edit", prev: { ...props.board }, next });
    }
  } catch (e) {
    error.value = e ? String(e) : t("Saving the board failed.");
  }
}

async function clearBoard() {
  const ok = await ask(t('Clear every tile from "{name}"?', { name: props.board.name }), {
    title: t("Clear board"),
    okLabel: t("Clear"),
    kind: "warning",
  });
  if (!ok) return;
  error.value = "";
  const buttonsSnapshot = (props.board.buttons || []).map((b) => ({ ...b }));
  try {
    await api.clearBoard(props.board.id);
  } catch (e) {
    error.value = e ? String(e) : t("Clearing the board failed.");
    return;
  }
  emit("saved", { mode: "clear", boardId: props.board.id, buttons: buttonsSnapshot });
}

async function deleteBoard() {
  const ok = await ask(t('Delete board "{name}"?', { name: props.board.name }), {
    title: t("Delete board"),
    okLabel: t("Delete"),
    kind: "warning",
  });
  if (!ok) return;
  error.value = "";
  const boardSnapshot = {
    ...props.board,
    buttons: (props.board.buttons || []).map((b) => ({ ...b })),
  };
  try {
    await api.deleteBoard(props.board.id);
  } catch (e) {
    error.value = e ? String(e) : t("Deleting the board failed.");
    return;
  }
  emit("saved", { mode: "delete", board: boardSnapshot });
}
</script>

<template>
  <div class="overlay" @click.self="overlayClose">
    <div class="modal">
      <div class="modal-head">
        {{ mode === "create" ? t("New board") : t("Edit board") }}
      </div>
      <div class="modal-body">
        <!-- live miniature of the grid the form describes -->
        <div class="mini-board" :style="{ background: form.background }">
          <div
            class="mini-grid"
            :style="{
              gridTemplateColumns: `repeat(${previewW}, 1fr)`,
              aspectRatio: `${previewW * 1.15} / ${previewH}`,
              width: `min(100%, ${Math.round((130 * previewW * 1.15) / previewH)}px)`,
            }"
          >
            <span v-for="n in previewW * previewH" :key="n"></span>
          </div>
        </div>
        <label class="field">
          {{ t("Name") }}
          <input v-model="form.name" :placeholder="t('Board name')" @keyup.enter="save" />
        </label>
        <div class="row">
          <label class="field">
            {{ t("Columns (width)") }}
            <input v-model.number="form.width" type="number" min="1" :max="MAX_BOARD_DIM" step="1" />
          </label>
          <label class="field">
            {{ t("Rows (height)") }}
            <input v-model.number="form.height" type="number" min="1" :max="MAX_BOARD_DIM" step="1" />
          </label>
        </div>
        <div class="field bg-field">
          <span>{{ t("Background") }}</span>
          <div class="bg-row">
            <input
              v-model="form.background"
              type="color"
              class="color-input"
              :aria-label="t('Background')"
            />
            <button
              v-for="c in PRESETS"
              :key="c"
              type="button"
              class="preset"
              :class="{ on: form.background.toLowerCase() === c }"
              :style="{ background: c }"
              :title="c"
              @click="form.background = c"
            ></button>
          </div>
        </div>
        <p v-if="error" class="modal-error" role="alert">{{ error }}</p>
      </div>
      <div class="modal-actions">
        <template v-if="mode === 'edit'">
          <button class="btn-text quiet-danger left" @click="clearBoard">{{ t("Clear tiles") }}</button>
          <button class="btn-text quiet-danger" @click="deleteBoard">{{ t("Delete board") }}</button>
        </template>
        <button class="btn-text" @click="emit('close')">{{ t("Cancel") }}</button>
        <button class="btn-text accent" @click="save">{{ t("Save") }}</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mini-board {
  border-radius: 12px;
  padding: 14px;
  margin-bottom: 16px;
  display: flex;
  justify-content: center;
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.06);
  transition: background 160ms ease-out;
}
.mini-grid {
  display: grid;
  gap: 4px;
}
.mini-grid span {
  border-radius: 3px;
  background: rgba(0, 0, 0, 0.16);
}
.row { display: flex; gap: 10px; }
.row > * { flex: 1; }
.bg-field { display: block; margin-bottom: 14px; font-size: 12.5px; font-weight: 500; color: var(--ink-2); }
.bg-row { display: flex; align-items: center; gap: 8px; margin-top: 6px; flex-wrap: wrap; }
.color-input { width: 44px; height: 36px; padding: 3px; cursor: pointer; flex: none; }
.preset {
  width: 26px;
  height: 26px;
  border-radius: 7px;
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
  transition: transform 120ms ease-out, box-shadow 120ms ease-out;
}
.preset:hover { transform: scale(1.08); }
.preset.on { box-shadow: 0 0 0 2px var(--tile), 0 0 0 4px var(--accent); }
/* destructive actions stay available but quieter than Save */
.quiet-danger { background: transparent; color: var(--danger-ink); padding-left: 10px; padding-right: 10px; }
.quiet-danger:hover { background: var(--danger-soft); }
.quiet-danger.left { margin-right: 0; }
.quiet-danger.left + .quiet-danger { margin-right: auto; }
.modal-error {
  font-size: 12px;
  color: var(--danger);
  margin: 10px 0 0;
  overflow-wrap: anywhere;
}
</style>
