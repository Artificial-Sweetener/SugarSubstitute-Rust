// frontend/comfyui/ui/surface/stack/CubeStackMetrics.ts
var CUBE_STACK_ROW_HEIGHT = 46;
var CUBE_STACK_ROW_GAP = 4;
function cubeStackRowGap(compact) {
  return compact ? 0 : CUBE_STACK_ROW_GAP;
}
function cubeStackRowStride(compact) {
  return CUBE_STACK_ROW_HEIGHT + cubeStackRowGap(compact);
}

export {
  CUBE_STACK_ROW_HEIGHT,
  cubeStackRowGap,
  cubeStackRowStride
};
