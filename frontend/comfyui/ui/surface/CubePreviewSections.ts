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
/** Define renderer-neutral horizontal sections for Cube output previews. */

import type {
  CubeOutputPreview,
  CubePreviewItem,
  CubePreviewSnapshot,
} from './CubePreviewModel.js';
import { CUBE_PREVIEW_EDGE_INSET } from './CubePreviewRailGeometry.js';

export interface CubePreviewRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface CubeCanvasPreviewSection {
  canonicalName: string;
  item: CubePreviewItem | null;
}

export interface CubeCanvasPreviewSectionLayout extends CubeCanvasPreviewSection {
  rect: CubePreviewRect;
  downloadAction: CubePreviewRect;
}

/** Separate adjacent Cube output sections in either native renderer. */
export const CUBE_PREVIEW_SECTION_GAP = 8;
/** Inset output titles and media from the preview rail boundary. */
export const CUBE_PREVIEW_SECTION_INSET = 6;
/** Reserve the shared title-row height used to anchor native output ports. */
export const CUBE_PREVIEW_TITLE_LINE_HEIGHT = 20;
/** Match Comfy's compact hover action size on the canvas surface. */
export const CUBE_PREVIEW_ACTION_SIZE = 28;

/** Inset every canvas preview section consistently from its rail boundary. */
export function resolveCubePreviewContentRect(area: CubePreviewRect): CubePreviewRect {
  return {
    x: area.x + CUBE_PREVIEW_SECTION_INSET,
    y: area.y + CUBE_PREVIEW_SECTION_INSET,
    width: Math.max(1, area.width - CUBE_PREVIEW_SECTION_INSET * 2),
    height: Math.max(1, area.height - CUBE_PREVIEW_SECTION_INSET * 2),
  };
}

/** Preserve the authored Nodes 1 media corridor around movable output ports. */
export function resolveCubeCanvasPreviewContentRect(area: CubePreviewRect): CubePreviewRect {
  return {
    x: area.x + CUBE_PREVIEW_EDGE_INSET,
    y: area.y + CUBE_PREVIEW_SECTION_INSET,
    width: Math.max(1, area.width - CUBE_PREVIEW_EDGE_INSET * 2),
    height: Math.max(1, area.height - CUBE_PREVIEW_SECTION_INSET * 2),
  };
}

/** Return every boundary output in stable graph order. */
export function resolveCubeOutputSections(
  snapshot: CubePreviewSnapshot,
): readonly CubeOutputPreview[] {
  return snapshot.outputs;
}

/**
 * Resolve one canvas media item per boundary output.
 */
export function resolveCubeCanvasPreviewSections(
  snapshot: CubePreviewSnapshot | null,
): readonly CubeCanvasPreviewSection[] {
  if (!snapshot) return [];
  return snapshot.outputs.map((output) => ({
    canonicalName: output.id,
    item: output.items[0] ?? null,
  }));
}

/** Lay out output sections and their native-style hover download targets. */
export function layoutCubeCanvasPreviewSections(
  area: CubePreviewRect | null,
  snapshot: CubePreviewSnapshot | null,
): readonly CubeCanvasPreviewSectionLayout[] {
  if (!area) return [];
  const outputs = resolveCubeCanvasPreviewSections(snapshot);
  const rects = dividePreviewIntoHorizontalSegments(
    resolveCubeCanvasPreviewContentRect(area),
    outputs.length,
    CUBE_PREVIEW_SECTION_GAP,
  );
  return outputs.flatMap((output, index) => {
    const rect = rects[index];
    if (!rect) return [];
    return [
      {
        ...output,
        rect,
        downloadAction: {
          x: rect.x + Math.max(0, rect.width - CUBE_PREVIEW_ACTION_SIZE),
          y: rect.y + CUBE_PREVIEW_TITLE_LINE_HEIGHT + 6,
          width: Math.min(CUBE_PREVIEW_ACTION_SIZE, rect.width),
          height: Math.min(CUBE_PREVIEW_ACTION_SIZE, Math.max(1, rect.height - 6)),
        },
      },
    ];
  });
}

/** Divide one preview area into ordered, equal-height horizontal segments. */
export function dividePreviewIntoHorizontalSegments(
  area: CubePreviewRect,
  sectionCount: number,
  gap: number,
): readonly CubePreviewRect[] {
  const count = Math.max(0, Math.floor(sectionCount));
  if (count === 0) return [];
  const safeGap = Math.max(0, gap);
  const totalGap = safeGap * Math.max(0, count - 1);
  const height = Math.max(1, (area.height - totalGap) / count);
  return Array.from({ length: count }, (_, index) => ({
    x: area.x,
    y: area.y + index * (height + safeGap),
    width: area.width,
    height,
  }));
}

/** Return the vertical center of each preview section's visible title row. */
export function resolveCubePreviewTitleAnchors(
  area: CubePreviewRect,
  sectionCount: number,
): readonly number[] {
  const content = resolveCubePreviewContentRect(area);
  return dividePreviewIntoHorizontalSegments(content, sectionCount, CUBE_PREVIEW_SECTION_GAP).map(
    (section) => section.y + CUBE_PREVIEW_TITLE_LINE_HEIGHT / 2,
  );
}
