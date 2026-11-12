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
/** Resolve comparable final images within the current Run and batch context. */

import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
export interface CanvasImagePair {
  readonly primary: WorkflowOutputItem | null;
  readonly secondary: WorkflowOutputItem | null;
}

/** Determine availability from real images rather than declared batch positions. */
export function canvasComparisonAvailable(catalog: CanvasCatalogSnapshot): boolean {
  return (
    new Set(
      catalog.entries.flatMap((entry) =>
        entry.items.filter((item) => item.mediaKind === 'image').map((item) => item.id),
      ),
    ).size > 1
  );
}

/** Prefer first/last stages in the selected batch, falling back to the available final images. */
export function canvasComparisonPair(
  catalog: CanvasCatalogSnapshot,
  batchIndex: number,
): CanvasImagePair | null {
  const images = catalog.entries.flatMap((entry) =>
    entry.items.filter((item) => item.mediaKind === 'image'),
  );
  const scoped = images.filter((item) => item.batchIndex === batchIndex);
  const candidates = new Set(scoped.map((item) => item.id)).size > 1 ? scoped : images;
  const primary = candidates[0] ?? null;
  const secondary = [...candidates].reverse().find((item) => item.id !== primary?.id) ?? null;
  return primary && secondary ? { primary, secondary } : null;
}
