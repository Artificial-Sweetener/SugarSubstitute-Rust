import {
  filterModelChoices
} from "./chunk-RSAZHLEW.js";
import {
  modelWallNeighbor
} from "./chunk-XP42IRIX.js";
import {
  computed,
  ref
} from "./chunk-OJTDXCRK.js";

// frontend/comfyui/ui/model_library/ModelPickerCompletion.ts
function modelChoiceCompletion(item, query) {
  if (!item) return null;
  const path = (item.relativePath.trim() || item.value.trim()).replaceAll("\\", "/");
  const stripped = path.replace(/\.(?:safetensors|ckpt|pt)$/i, "");
  const filename = stripped.split("/").at(-1) ?? stripped;
  const normalized = query.toLocaleLowerCase();
  const hasSeparator = /[\\/]/.test(query);
  if (hasSeparator && stripped.toLocaleLowerCase().startsWith(normalized.replaceAll("\\", "/"))) {
    const candidate = query.includes("\\") && !query.includes("/") ? stripped.replaceAll("/", "\\") : stripped;
    return completion(candidate.slice(query.length), query);
  }
  if (filename.toLocaleLowerCase().startsWith(normalized))
    return completion(filename.slice(query.length), query);
  if (!hasSeparator && query.trim().length > 0) {
    const start = filename.toLocaleLowerCase().indexOf(normalized);
    if (start > 0) return completion(filename.slice(start + query.length), query);
  }
  const friendly = [item.title.trim(), item.version.trim()].filter(Boolean).join(" - ");
  if (friendly.toLocaleLowerCase().startsWith(normalized))
    return completion(friendly.slice(query.length), query);
  return filename ? { suffix: ` \u2192 ${filename}`, text: filename } : null;
}
function completion(suffix, query) {
  return suffix ? { suffix, text: query + suffix } : null;
}

// frontend/comfyui/ui/model_library/vue/ModelBrowsing.ts
var WALL_KEYS = /* @__PURE__ */ new Set(["ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight", "Home", "End"]);
function useModelBrowsing(items) {
  const query = ref("");
  const folder = ref("");
  const active = ref(0);
  const wallWidth = ref(540);
  const visible = computed(() => filterModelChoices(items(), query.value, folder.value));
  const navigable = computed(() => filterModelChoices(items(), query.value, ""));
  const completion2 = computed(
    () => modelChoiceCompletion(visible.value[active.value], query.value)
  );
  return {
    query,
    folder,
    active,
    wallWidth,
    /** Models matching the search inside the open folder, in wall order. */
    visible,
    /** Models matching the search in every folder, from which the folder strip is built. */
    navigable,
    /** The display-only completion of the search text against the current model. */
    completion: completion2,
    /** Start at the root with no search, on the model with this value when it is shown. */
    restart(current = "") {
      query.value = "";
      folder.value = "";
      active.value = Math.max(
        0,
        visible.value.findIndex((item) => item.value === current)
      );
    },
    /** Replace the search text and return to the first match. */
    search(text) {
      query.value = text;
      active.value = 0;
    },
    /** Open one folder and return to its first model. */
    open(path) {
      folder.value = path;
      active.value = 0;
    },
    /** Extend the search to its completion, keeping the same model current. */
    complete() {
      const accepted = completion2.value;
      if (!accepted) return false;
      const highlighted = visible.value[active.value]?.value;
      query.value = accepted.text;
      active.value = Math.max(
        0,
        visible.value.findIndex((item) => item.value === highlighted)
      );
      return true;
    },
    /** Move the current tile for one wall key; other keys are not handled. */
    move(key) {
      if (!WALL_KEYS.has(key)) return false;
      active.value = modelWallNeighbor(visible.value, wallWidth.value, active.value, key);
      return true;
    }
  };
}

export {
  useModelBrowsing
};
