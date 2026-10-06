import {
  MODEL_PICKER_WALL_PROFILE,
  layoutModelGallery
} from "./chunk-JHL2BQUZ.js";

// frontend/comfyui/ui/model_library/ModelPickerNavigation.ts
function modelWallNeighbor(items, width, active, key, profile = MODEL_PICKER_WALL_PROFILE) {
  if (!items.length) return 0;
  if (key === "Home") return 0;
  if (key === "End") return items.length - 1;
  if (key === "ArrowLeft" || key === "ArrowRight")
    return Math.max(0, Math.min(items.length - 1, active + (key === "ArrowRight" ? 1 : -1)));
  const rows = layoutModelGallery(items, width, profile);
  const rowIndex = rows.findIndex((row) => row.tiles.some((tile2) => tile2.index === active));
  const tile = rows[rowIndex]?.tiles.find((candidate) => candidate.index === active);
  const next = rows[(rowIndex + (key === "ArrowDown" ? 1 : -1) + rows.length) % rows.length];
  if (!tile || !next) return active;
  const center = tile.left + tile.width / 2;
  return next.tiles.reduce(
    (closest, candidate) => Math.abs(candidate.left + candidate.width / 2 - center) < Math.abs(closest.left + closest.width / 2 - center) ? candidate : closest
  ).index;
}

export {
  modelWallNeighbor
};
