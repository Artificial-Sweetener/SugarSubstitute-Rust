// frontend/comfyui/ui/model_library/ModelPickerPlacement.ts
var MODEL_PICKER_POPUP_WIDTH = 560;
var MODEL_PICKER_POPUP_HEIGHT = 630;
var MARGIN = 8;
function safeAxis(origin, length) {
  const bounded = Math.max(0, length);
  return bounded > MARGIN * 2 ? [origin + MARGIN, origin + bounded - MARGIN] : [origin, origin + bounded];
}
function modelPickerPlacement(boundary, anchor) {
  const [safeLeft, safeRight] = safeAxis(boundary.left, boundary.width);
  const [safeTop, safeBottom] = safeAxis(boundary.top, boundary.height);
  const width = Math.min(MODEL_PICKER_POPUP_WIDTH, Math.max(1, safeRight - safeLeft));
  const left = Math.max(safeLeft, Math.min(anchor.left, safeRight - width));
  const bottom = anchor.top + anchor.height;
  const below = Math.max(0, safeBottom - bottom);
  const above = Math.max(0, anchor.top - safeTop);
  const mode = below >= MODEL_PICKER_POPUP_HEIGHT ? "below" : above >= MODEL_PICKER_POPUP_HEIGHT || above > below ? "above" : "below";
  const height = Math.min(MODEL_PICKER_POPUP_HEIGHT, Math.max(1, mode === "below" ? below : above));
  return { left, top: mode === "below" ? bottom : anchor.top - height, width, height, mode };
}

export {
  modelPickerPlacement
};
