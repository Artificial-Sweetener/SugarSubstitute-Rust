import {
  h
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/canvas_editing/presentation/CanvasEditIcons.ts
var FLUENT = [
  "pan-zoom",
  "move",
  "brush",
  "eraser",
  "select-rectangle",
  "select-lasso",
  "transform",
  "shared-edges",
  "undo",
  "redo",
  "rotate-left",
  "rotate-right",
  "flip-horizontal",
  "flip-vertical"
];
var SUBSTITUTE = [
  "select-ellipse",
  "mask-rectangle",
  "mask-ellipse",
  "mask-lasso",
  "deselect",
  "apply",
  "close",
  "modify"
];
var URLS = new Map([
  ...FLUENT.map(
    (name) => [
      name,
      new URL(`../../../../icons/canvas-editing/${name}.svg`, import.meta.url).href
    ]
  ),
  ...SUBSTITUTE.map(
    (name) => [name, new URL(`../../../../canvas-editing/${name}.svg`, import.meta.url).href]
  )
]);
var CANVAS_TOOL_ICONS = Object.freeze({
  "pan-zoom": "pan-zoom",
  move: "move",
  brush: "brush",
  eraser: "eraser",
  "select-rectangle": "select-rectangle",
  "select-ellipse": "select-ellipse",
  "select-lasso": "select-lasso",
  "mask-rectangle": "mask-rectangle",
  "mask-ellipse": "mask-ellipse",
  "mask-lasso": "mask-lasso",
  transform: "transform",
  "shared-edges": "shared-edges"
});
function maskImage(name) {
  return `url("${URLS.get(name) ?? ""}")`;
}
function canvasEditIcon(name) {
  return h("span", {
    class: "sc-canvas-edit-icon",
    "data-icon": name,
    "aria-hidden": "true",
    style: { "--sc-canvas-edit-icon": maskImage(name) }
  });
}
function createCanvasEditIcon(document, name) {
  const icon = document.createElement("span");
  icon.className = "sc-canvas-edit-icon";
  icon.setAttribute("data-icon", name);
  icon.setAttribute("aria-hidden", "true");
  icon.style.setProperty("--sc-canvas-edit-icon", maskImage(name));
  return icon;
}

export {
  CANVAS_TOOL_ICONS,
  canvasEditIcon,
  createCanvasEditIcon
};
