import {
  CanvasToolRail
} from "./chunk-3IZU6ONS.js";
import {
  CanvasBrushOptions
} from "./chunk-NC4KX5LX.js";
import {
  CanvasContextualToolbar
} from "./chunk-NOFDFIDV.js";
import {
  canvasEditToolAvailable,
  iconButton
} from "./chunk-OWTPCN3N.js";
import {
  CanvasCoverageControls
} from "./chunk-36SKP3QA.js";
import {
  CANVAS_EDIT_TEXT
} from "./chunk-ECAVEMES.js";
import {
  defineComponent,
  h,
  onMounted,
  ref
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/canvas_editing/domain/CanvasEditSurface.ts
function canvasEditSurface(state, modifying) {
  const session = state.session;
  if (state.mode === "coverage") return { kind: "coverage" };
  if (modifying || session?.kind === "modify") return { kind: "modify" };
  if (session?.kind === "move" || session?.kind === "shared-edges")
    return { kind: "session", tool: session.kind, armed: false };
  if (!session && state.tool === "shared-edges")
    return { kind: "session", tool: "shared-edges", armed: true };
  if (session?.kind === "transform")
    return { kind: "transform", target: state.selection ? "selection" : "layer" };
  if (state.selection) return { kind: "selection" };
  return { kind: "none" };
}
function canvasEditChromeSuppressed(surface) {
  return surface.kind === "coverage" || surface.kind === "modify" || surface.kind === "transform";
}

// frontend/comfyui/ui/canvas_editing/presentation/CanvasEditOverlay.ts
var CanvasEditOverlay = defineComponent({
  name: "CanvasEditOverlay",
  props: {
    snapshot: { type: Object, default: null },
    controller: { type: Object, required: true },
    viewport: { type: Object, default: null },
    maskColor: { type: String, default: null }
  },
  setup(props) {
    return () => {
      const snapshot = props.snapshot;
      if (!snapshot) return null;
      const edit = props.controller;
      const surface = snapshot.surface;
      return [
        canvasEditChromeSuppressed(surface) ? null : dock(snapshot, edit, props.maskColor),
        props.viewport && surface.kind !== "none" && surface.kind !== "coverage" ? h(CanvasContextualToolbar, { snapshot, controller: edit, viewport: props.viewport }) : null,
        surface.kind === "coverage" ? h(CanvasCoveragePanel, { snapshot, controller: edit, key: "coverage" }) : null
      ];
    };
  }
});
function dock(snapshot, edit, maskColor) {
  const state = snapshot.state;
  const tool = state.tool === "brush" || state.tool === "eraser" ? state.tool : null;
  return h("div", { class: "sc-canvas-edit-dock" }, [
    h("div", { class: "sc-canvas-edit-topbar" }, [
      h(
        "div",
        {
          class: "sc-canvas-edit-chrome sc-canvas-edit-row",
          role: "group",
          "aria-label": CANVAS_EDIT_TEXT.history
        },
        [
          iconButton(
            "undo",
            CANVAS_EDIT_TEXT.undo,
            "undo",
            () => edit.execute({ type: "history", action: "undo" }),
            state.history.undo
          ),
          iconButton(
            "redo",
            CANVAS_EDIT_TEXT.redo,
            "redo",
            () => edit.execute({ type: "history", action: "redo" }),
            state.history.redo
          )
        ]
      ),
      tool && canvasEditToolAvailable(state, tool) ? h(CanvasBrushOptions, {
        key: "options",
        tool,
        options: state.options[tool],
        color: maskColor,
        change: (target, options) => edit.setBrushOptions(target, options)
      }) : null
    ]),
    h(CanvasToolRail, {
      slots: snapshot.slots,
      selectSlot: (slot) => edit.selectSlot(slot),
      selectTool: (tool2) => edit.selectTool(tool2)
    })
  ]);
}
var CanvasCoveragePanel = defineComponent({
  name: "CanvasCoveragePanel",
  props: {
    snapshot: { type: Object, required: true },
    controller: { type: Object, required: true }
  },
  setup(props) {
    const element = ref(null);
    const applying = ref(false);
    onMounted(() => element.value?.querySelector("select")?.focus());
    return () => h(
      "div",
      {
        ref: element,
        class: "sc-canvas-edit-chrome sc-canvas-edit-coverage",
        role: "dialog",
        "aria-modal": "true",
        "aria-label": CANVAS_EDIT_TEXT.editLayerCoverage
      },
      [
        h(CanvasCoverageControls, {
          value: props.snapshot.coverage,
          disabled: applying.value,
          change: (value) => props.controller.updateCoverage(value)
        }),
        h(
          "button",
          {
            type: "button",
            class: "sc-canvas-edit-primary",
            "data-canvas-action": "apply",
            disabled: applying.value,
            onClick: () => {
              applying.value = true;
              void props.controller.applyCoverage().finally(() => {
                applying.value = false;
              });
            }
          },
          CANVAS_EDIT_TEXT.apply
        ),
        iconButton(
          "close",
          CANVAS_EDIT_TEXT.cancel,
          "cancel",
          () => props.controller.closeCoverage(),
          !applying.value
        )
      ]
    );
  }
});

export {
  canvasEditSurface,
  CanvasEditOverlay
};
