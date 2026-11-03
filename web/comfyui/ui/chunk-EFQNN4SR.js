// frontend/comfyui/ui/canvas_editing/domain/CanvasCoverageModification.ts
var CANVAS_COVERAGE_OPERATIONS = Object.freeze([
  "expand",
  "contract",
  "feather"
]);
var DEFAULT_COVERAGE_MODIFICATION = Object.freeze({
  operation: "expand",
  pixels: 4
});
var COVERAGE_PIXELS_MIN = 1;
var COVERAGE_PIXELS_MAX = 999;
function coveragePixels(text, previous) {
  const value = Number.parseInt(text, 10);
  if (!Number.isFinite(value)) return previous;
  return Math.min(COVERAGE_PIXELS_MAX, Math.max(COVERAGE_PIXELS_MIN, value));
}
function coverageOperation(value) {
  return CANVAS_COVERAGE_OPERATIONS.find((operation) => operation === value) ?? null;
}
var BRUSH_LIMITS = Object.freeze({
  size: { min: 1, max: 1e3 },
  hardness: { min: 0, max: 100 },
  opacity: { min: 0, max: 100 }
});
function withBrushOption(options, field, value) {
  const { min, max } = BRUSH_LIMITS[field];
  const bounded = Number.isFinite(value) ? Math.min(max, Math.max(min, Math.round(value))) : min;
  return { ...options, [field]: bounded };
}

export {
  CANVAS_COVERAGE_OPERATIONS,
  DEFAULT_COVERAGE_MODIFICATION,
  COVERAGE_PIXELS_MIN,
  COVERAGE_PIXELS_MAX,
  coveragePixels,
  coverageOperation,
  BRUSH_LIMITS,
  withBrushOption
};
