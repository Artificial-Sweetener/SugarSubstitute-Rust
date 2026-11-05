import {
  CanvasToolRail
} from "./chunk-3IZU6ONS.js";
import {
  CANVAS_EDIT_CHROME_INSET
} from "./chunk-HDVDWAOE.js";
import {
  canvasEditToolAvailable,
  iconButton
} from "./chunk-BPQUSL77.js";
import {
  CanvasBrushOptions
} from "./chunk-NC4KX5LX.js";
import {
  CANVAS_EDIT_TEXT
} from "./chunk-ECAVEMES.js";
import {
  defineComponent,
  h,
  onBeforeUnmount,
  onMounted,
  onUpdated,
  ref
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/canvas_editing/domain/CanvasEditDockLayout.ts
var CANVAS_EDIT_CHROME_GAP = 8;
function canvasEditDockPlacement(obstacles, topBar, rail) {
  const bar = { left: CANVAS_EDIT_CHROME_INSET, top: CANVAS_EDIT_CHROME_INSET, ...topBar };
  for (const obstacle of [...obstacles].sort((a, b) => a.left - b.left))
    if (intersects(bar, obstacle))
      bar.left = obstacle.left + obstacle.width + CANVAS_EDIT_CHROME_GAP;
  const column = { left: CANVAS_EDIT_CHROME_INSET, top: CANVAS_EDIT_CHROME_INSET, ...rail };
  for (const obstacle of [...obstacles, bar].sort((a, b) => a.top - b.top))
    if (obstacle.top <= column.top && intersects(column, obstacle))
      column.top = obstacle.top + obstacle.height + CANVAS_EDIT_CHROME_GAP;
  const limits = obstacles.filter(
    (obstacle) => obstacle.width > 0 && obstacle.height > 0 && obstacle.top > column.top && obstacle.left < column.left + column.width && column.left < obstacle.left + obstacle.width
  ).map((obstacle) => obstacle.top - CANVAS_EDIT_CHROME_GAP - column.top);
  return {
    topBar: { x: bar.left, y: bar.top },
    rail: { x: column.left, y: column.top },
    railHeight: limits.length ? Math.max(0, Math.min(...limits)) : null
  };
}
function intersects(a, b) {
  if (a.width <= 0 || a.height <= 0 || b.width <= 0 || b.height <= 0) return false;
  return a.left < b.left + b.width && b.left < a.left + a.width && a.top < b.top + b.height && b.top < a.top + a.height;
}

// frontend/comfyui/ui/canvas_editing/presentation/CanvasEditDock.ts
var CanvasEditDock = defineComponent({
  name: "CanvasEditDock",
  props: {
    snapshot: { type: Object, required: true },
    controller: { type: Object, required: true },
    maskColor: { type: String, default: null },
    /** Host chrome to keep clear, in the dock's CSS-pixel space. */
    obstacles: {
      type: Function,
      default: () => []
    },
    layoutRevision: { type: Number, default: 0 }
  },
  setup(props) {
    const topBar = ref(null);
    const rail = ref(null);
    let observer = null;
    function place() {
      const bar = topBar.value, column = rail.value;
      if (!bar || !column) return;
      const placement = canvasEditDockPlacement(
        props.obstacles(),
        { width: bar.offsetWidth, height: bar.offsetHeight },
        { width: column.offsetWidth, height: column.offsetHeight }
      );
      move(bar, placement.topBar);
      move(column, placement.rail);
      const fill = `calc(100% - ${String(placement.rail.y + CANVAS_EDIT_CHROME_INSET)}px)`;
      column.style.maxHeight = placement.railHeight === null ? fill : `min(${fill}, ${String(placement.railHeight)}px)`;
    }
    onMounted(() => {
      place();
      if (typeof ResizeObserver === "undefined") return;
      observer = new ResizeObserver(() => place());
      if (topBar.value) observer.observe(topBar.value);
      if (rail.value) observer.observe(rail.value);
    });
    onUpdated(place);
    onBeforeUnmount(() => observer?.disconnect());
    return () => {
      void props.layoutRevision;
      const edit = props.controller;
      const state = props.snapshot.state;
      const tool = state.tool === "brush" || state.tool === "eraser" ? state.tool : null;
      return h("div", { class: "sc-canvas-edit-dock" }, [
        h("div", { ref: topBar, class: "sc-canvas-edit-topbar" }, [
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
            color: props.maskColor,
            change: (target, options) => edit.setBrushOptions(target, options)
          }) : null
        ]),
        h("div", { ref: rail, class: "sc-canvas-edit-rail-dock" }, [
          h(CanvasToolRail, {
            slots: props.snapshot.slots,
            selectSlot: (slot) => edit.selectSlot(slot),
            selectTool: (selected) => edit.selectTool(selected)
          })
        ])
      ]);
    };
  }
});
function move(element, point) {
  element.style.transform = `translate(${point.x}px, ${point.y}px)`;
  element.dataset.canvasPlacement = `${point.x},${point.y}`;
}

export {
  CanvasEditDock
};
