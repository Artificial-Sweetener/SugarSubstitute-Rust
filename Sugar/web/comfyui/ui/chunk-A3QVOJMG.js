import {
  CanvasToolbarPlacement,
  canvasEditSafeArea
} from "./chunk-3HDL7XRD.js";
import {
  modifyPage,
  selectionPage,
  sessionPage,
  transformPage
} from "./chunk-KA4VBPJE.js";
import {
  CANVAS_EDIT_TEXT,
  CANVAS_TOOL_LABELS
} from "./chunk-2WJX4EBF.js";
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
    let shown = null;
    let drag = null;
    watch(
      () => props.snapshot.surface.kind,
      () => placement.reset()
    );
    function show(toolbar, point) {
      shown = point;
      toolbar.style.transform = `translate(${point.x}px, ${point.y}px)`;
      toolbar.dataset.canvasPlacement = `${point.x},${point.y}`;
    }
    function grab(event) {
      if (event.button !== 0 || !(event.currentTarget instanceof HTMLElement)) return;
      event.preventDefault();
      event.stopPropagation();
      event.currentTarget.setPointerCapture(event.pointerId);
      drag = { pointer: event.pointerId, x: event.clientX, y: event.clientY };
    }
    function follow(event) {
      const toolbar = element.value;
      if (!drag || drag.pointer !== event.pointerId || !toolbar || !shown) return;
      event.preventDefault();
      const delta = { x: event.clientX - drag.x, y: event.clientY - drag.y };
      drag.x = event.clientX;
      drag.y = event.clientY;
      show(
        toolbar,
        placement.moveBy(
          shown,
          delta,
          { width: toolbar.offsetWidth, height: toolbar.offsetHeight },
          canvasEditSafeArea(props.viewport)
        )
      );
    }
    function release(event) {
      if (drag?.pointer === event.pointerId) drag = null;
    }
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
      show(
        toolbar,
        placement.position(
          context,
          { width: toolbar.offsetWidth, height: toolbar.offsetHeight },
          canvasEditSafeArea(viewport),
          viewport.density
        )
      );
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
        [
          h("div", {
            class: "sc-canvas-edit-toolbar-grip",
            title: CANVAS_EDIT_TEXT.moveToolbar,
            "aria-label": CANVAS_EDIT_TEXT.moveToolbar,
            "data-canvas-toolbar-grip": "",
            onPointerdown: grab,
            onPointermove: follow,
            onPointerup: release,
            onPointercancel: release,
            onLostpointercapture: release
          }),
          ...children
        ]
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
