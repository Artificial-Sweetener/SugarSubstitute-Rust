// frontend/comfyui/ui/surface/stack/CubeStackMetrics.ts
var CUBE_STACK_ROW_HEIGHT = 46;
var CUBE_STACK_ROW_GAP = 4;
function cubeStackRowGap(packed) {
  return packed ? 0 : CUBE_STACK_ROW_GAP;
}
function cubeStackRowStride(packed) {
  return CUBE_STACK_ROW_HEIGHT + cubeStackRowGap(packed);
}

export {
  CUBE_STACK_ROW_HEIGHT,
  cubeStackRowGap,
  cubeStackRowStride
};
