// frontend/comfyui/ui/canvas_editing/presentation/CanvasEditArtwork.ts
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
var CANVAS_EDIT_ICON_NAMES = Object.freeze([
  ...FLUENT,
  ...SUBSTITUTE
]);
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
function canvasEditIconUrl(name) {
  return URLS.get(name) ?? "";
}
export {
  CANVAS_EDIT_ICON_NAMES,
  canvasEditIconUrl
};
