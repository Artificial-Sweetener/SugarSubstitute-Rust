import {
  onBeforeUpdate,
  onUpdated
} from "./chunk-MNPWJMUZ.js";

// frontend/comfyui/ui/surface/stack/vue/CubeStackFocus.ts
function useCubeStackFocus(root) {
  let focused = null;
  onBeforeUpdate(() => {
    const active = root.value?.ownerDocument.activeElement;
    focused = active instanceof HTMLElement && root.value?.contains(active) ? active : null;
  });
  onUpdated(() => {
    if (focused?.isConnected && focused.ownerDocument.activeElement === focused.ownerDocument.body)
      focused.focus({ preventScroll: true });
    focused = null;
  });
}

export {
  useCubeStackFocus
};
