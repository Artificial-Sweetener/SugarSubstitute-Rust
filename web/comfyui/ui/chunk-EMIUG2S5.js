import {
  CanvasToolbarPlacement,
  canvasEditSafeArea
} from "./chunk-HDVDWAOE.js";
import {
  modifyPage,
  selectionPage,
  sessionPage,
  transformPage
} from "./chunk-BPQUSL77.js";
import {
  CANVAS_EDIT_TEXT,
  CANVAS_TOOL_LABELS
} from "./chunk-ECAVEMES.js";
import {
  defineComponent,
  h,
  onMounted,
  onUpdated,
  ref,
  watch
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/canvas_editing/presentation/CanvasContextualToolbar.ts
var CanvasContextualToolbar = defineComponent({
  name: "CanvasContextualToolbar",
  props: {
    snapshot: { type: Object, required: true },
    controller: { type: Object, required: true },
    viewport: { type: Object, required: true }
  },
  setup(props) {
    const element = ref(null);
    const placement = new CanvasToolbarPlacement();
    watch(
      () => props.snapshot.surface.kind,
      () => placement.reset()
    );
    function place() {
      const toolbar = element.value;
      if (!toolbar) return;
      const viewport = props.viewport;
      const bounds = props.snapshot.state.selection?.bounds;
      const context = bounds ? {
        left: viewport.left + bounds.left,
        top: viewport.top + bounds.top,
        width: bounds.width,
        height: bounds.height
      } : null;
      const point = placement.position(
        context,
        { width: toolbar.offsetWidth, height: toolbar.offsetHeight },
        canvasEditSafeArea(viewport),
        viewport.density
      );
      toolbar.style.transform = `translate(${point.x}px, ${point.y}px)`;
      toolbar.dataset.canvasPlacement = `${point.x},${point.y}`;
    }
    onMounted(place);
    onUpdated(place);
    return () => {
      const children = toolbarPage(props.snapshot, props.controller);
      if (!children) return null;
      return h(
        "div",
        {
          ref: element,
          class: "sc-canvas-edit-chrome sc-canvas-edit-row sc-canvas-edit-toolbar",
          role: "toolbar",
          "aria-label": toolbarLabel(props.snapshot),
          "data-canvas-surface": props.snapshot.surface.kind
        },
        children
      );
    };
  }
});
function toolbarLabel(snapshot) {
  const surface = snapshot.surface;
  if (surface.kind === "modify") return CANVAS_EDIT_TEXT.modifySelection;
  if (surface.kind === "transform") return CANVAS_EDIT_TEXT.transform;
  if (surface.kind === "session") return CANVAS_TOOL_LABELS[surface.tool];
  return CANVAS_EDIT_TEXT.selectionActions;
}
function toolbarPage(snapshot, edit) {
  const surface = snapshot.surface;
  switch (surface.kind) {
    case "selection":
      return selectionPage(snapshot, edit);
    case "modify":
      return modifyPage(snapshot, edit);
    case "transform":
      return transformPage(snapshot, edit, surface.target);
    case "session":
      return sessionPage(snapshot, edit, surface.tool);
    default:
      return null;
  }
}

export {
  CanvasContextualToolbar
};
