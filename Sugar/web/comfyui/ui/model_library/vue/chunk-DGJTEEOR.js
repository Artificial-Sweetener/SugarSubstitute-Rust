import {
  watch
} from "./chunk-OJTDXCRK.js";

// frontend/comfyui/ui/model_library/vue/ModelSelectionUpdates.ts
function useModelSelectionUpdates(selected, updates, feedback) {
  watch(
    () => {
      const item = selected();
      return item ? `${item.kind}
${item.id}
${item.providerVersionId}` : "";
    },
    () => {
      const item = selected();
      if (!item || !["checkpoints", "diffusion_models", "loras"].includes(item.kind) || !/^[a-f0-9]{64}$/.test(item.id))
        return;
      void updates?.checkSelection(item.id).catch((error) => {
        feedback?.(
          "error",
          error instanceof Error ? error.message : "Model updates could not be checked."
        );
      });
    },
    { immediate: true }
  );
}

export {
  useModelSelectionUpdates
};
