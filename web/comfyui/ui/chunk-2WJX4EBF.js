// frontend/comfyui/ui/canvas_editing/presentation/CanvasEditText.ts
var CANVAS_TOOL_LABELS = Object.freeze({
  "pan-zoom": "Pan & Zoom",
  move: "Move",
  brush: "Brush",
  eraser: "Eraser",
  "select-rectangle": "Rectangle selection",
  "select-ellipse": "Ellipse selection",
  "select-lasso": "Lasso selection",
  "mask-rectangle": "Rectangle Mask",
  "mask-ellipse": "Ellipse Mask",
  "mask-lasso": "Lasso Mask",
  transform: "Transform",
  "shared-edges": "Resize shared edges"
});
var CANVAS_COVERAGE_OPERATION_LABELS = Object.freeze({ expand: "Expand", contract: "Contract", feather: "Feather" });
var CANVAS_EDIT_TEXT = Object.freeze({
  tools: "Canvas tools",
  moveToolbar: "Move Contextual Toolbar",
  toolGroup: (label) => `${label} tools`,
  brushSettings: "Brush settings",
  size: "Size",
  hardness: "Hardness",
  opacity: "Opacity",
  pixelValue: (value) => `${value} px`,
  percentValue: (value) => `${value}%`,
  close: "Close",
  undo: "Undo",
  redo: "Redo",
  history: "History",
  apply: "Apply",
  cancel: "Cancel",
  modifySelection: "Modify selection",
  selectionActions: "Selection",
  transform: "Transform",
  transformSelection: "Transform selection",
  clear: "Clear",
  deselect: "Deselect",
  rotateLeft: "Rotate left",
  rotateRight: "Rotate right",
  flipHorizontal: "Flip horizontal",
  flipVertical: "Flip vertical",
  operation: "Operation",
  by: "by",
  pixels: "Pixels",
  editLayerCoverage: "Edit layer coverage",
  canvasActions: "Canvas actions"
});

export {
  CANVAS_TOOL_LABELS,
  CANVAS_COVERAGE_OPERATION_LABELS,
  CANVAS_EDIT_TEXT
};
