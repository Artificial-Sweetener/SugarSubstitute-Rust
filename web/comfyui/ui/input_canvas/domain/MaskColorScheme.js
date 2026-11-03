import {
  REGION_PALETTE_BASE,
  formatRegionHex,
  parseRegionHex,
  regionPaletteColor
} from "../chunk-S7BQTISC.js";

// frontend/comfyui/ui/input_canvas/domain/MaskColorScheme.ts
var DEFAULT_MASK_OPACITY = 0.5;
function maskColor(accent, index, count) {
  const base = parseRegionHex(accent);
  if (!base) throw new Error("A mask colour requires a #rrggbb accent.");
  return formatRegionHex(regionPaletteColor(base, index, count));
}
function regionMaskColor(index, count) {
  return formatRegionHex(regionPaletteColor(REGION_PALETTE_BASE, index, count));
}
export {
  DEFAULT_MASK_OPACITY,
  maskColor,
  regionMaskColor
};
