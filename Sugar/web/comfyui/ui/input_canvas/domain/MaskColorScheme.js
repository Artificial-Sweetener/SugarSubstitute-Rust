import {
  formatRegionHex,
  parseRegionHex,
  regionPaletteColor
} from "../chunk-6OBOTKVC.js";

// frontend/comfyui/ui/input_canvas/domain/MaskColorScheme.ts
var DEFAULT_MASK_OPACITY = 0.5;
function maskColor(accent, index, count) {
  const base = parseRegionHex(accent);
  if (!base) throw new Error("A mask colour requires a #rrggbb accent.");
  return formatRegionHex(regionPaletteColor(base, index, count));
}
export {
  DEFAULT_MASK_OPACITY,
  maskColor
};
