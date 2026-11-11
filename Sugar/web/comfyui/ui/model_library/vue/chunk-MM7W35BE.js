import {
  onBeforeUnmount,
  ref
} from "./chunk-3W5PGMRW.js";

// frontend/comfyui/ui/model_library/vue/ModelActionState.ts
function useModelAction(disabled = () => false) {
  const busy = ref(false);
  const status = ref("");
  const error = ref("");
  let controller = null;
  onBeforeUnmount(() => controller?.abort());
  const run = async (operation) => {
    if (busy.value || disabled()) return;
    const current = new AbortController();
    controller = current;
    busy.value = true;
    status.value = "";
    error.value = "";
    try {
      await operation(current.signal);
    } catch (failure) {
      if (!current.signal.aborted) {
        status.value = "";
        error.value = failure instanceof Error ? failure.message : "The model action failed.";
        console.error("SugarCubes model action failed.", failure);
      }
    } finally {
      busy.value = false;
      controller = null;
    }
  };
  const cancel = () => {
    controller?.abort();
    status.value = "Cancelled.";
  };
  return { busy, status, error, run, cancel };
}

export {
  useModelAction
};
