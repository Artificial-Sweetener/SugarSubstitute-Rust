//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Keep image-wall keyboard movement aligned with its actual justified rows. */
import type { ModelCatalogItem } from './ModelCatalog.js';
import { MODEL_PICKER_WALL_PROFILE } from './ModelPickerLayout.js';
import type { ModelWallProfile } from './ModelPickerLayout.js';
import { layoutModelGallery } from './ModelGalleryLayout.js';

/** Move vertically to the nearest tile center rather than stepping through a linear list. */
export function modelWallNeighbor(
  items: readonly ModelCatalogItem[],
  width: number,
  active: number,
  key: string,
  profile: ModelWallProfile = MODEL_PICKER_WALL_PROFILE,
): number {
  if (!items.length) return 0;
  if (key === 'Home') return 0;
  if (key === 'End') return items.length - 1;
  if (key === 'ArrowLeft' || key === 'ArrowRight')
    return Math.max(0, Math.min(items.length - 1, active + (key === 'ArrowRight' ? 1 : -1)));
  const rows = layoutModelGallery(items, width, profile);
  const rowIndex = rows.findIndex((row) => row.tiles.some((tile) => tile.index === active));
  const tile = rows[rowIndex]?.tiles.find((candidate) => candidate.index === active);
  const next = rows[(rowIndex + (key === 'ArrowDown' ? 1 : -1) + rows.length) % rows.length];
  if (!tile || !next) return active;
  const center = tile.left + tile.width / 2;
  return next.tiles.reduce((closest, candidate) =>
    Math.abs(candidate.left + candidate.width / 2 - center) <
    Math.abs(closest.left + closest.width / 2 - center)
      ? candidate
      : closest,
  ).index;
}
