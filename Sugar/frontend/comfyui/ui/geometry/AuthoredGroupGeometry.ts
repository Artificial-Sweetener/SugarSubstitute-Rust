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
/** Own authored cube-group margins and renderer-facing group storage geometry. */

import { isRecord } from '../types/common.js';
import {
  AUTHORED_LAYOUT_KEY,
  readAuthoredLayoutBaseline,
  type AuthoredLayoutBaseline,
} from './AuthoredLayoutBaseline.js';
import { unionLayoutRects, type LayoutRect } from './AuthoredLayoutSolver.js';
import { authoredNodePresentationRect } from './ComfyNodeGeometry.js';
import type { NodeRenderer } from './RendererGeometryPolicy.js';

const NODE_TITLE_HEIGHT = 30;

/** Report whether managed metadata carries authoritative authored group geometry. */
export function hasAuthoredGroupGeometry(metadata: unknown): boolean {
  if (!isRecord(metadata)) return false;
  return readAuthoredLayoutBaseline(metadata[AUTHORED_LAYOUT_KEY])?.group != null;
}

/** Resize authored cube chrome around solved node cards while preserving every margin. */
export function solveAuthoredGroupPresentationRect(
  baseline: AuthoredLayoutBaseline,
  solvedRects: readonly LayoutRect[],
): LayoutRect | null {
  const authoredGroup = baseline.group;
  const sourceBounds = unionLayoutRects(
    Object.values(baseline.entries).map((entry) =>
      authoredNodePresentationRect(
        {
          x: baseline.origin[0] + entry.x,
          y: baseline.origin[1] + entry.y,
          w: entry.w,
          h: entry.h,
        },
        entry.collapsed,
        entry.title,
      ),
    ),
  );
  const targetBounds = unionLayoutRects(solvedRects);
  if (!authoredGroup || !sourceBounds || !targetBounds) return null;
  const groupSource = {
    x: baseline.origin[0] + authoredGroup.x,
    y: baseline.origin[1] + authoredGroup.y,
    w: authoredGroup.w,
    h: authoredGroup.h,
  };
  const left = Math.max(0, sourceBounds.x - groupSource.x);
  const top = Math.max(0, sourceBounds.y - groupSource.y);
  const right = Math.max(0, groupSource.x + groupSource.w - sourceBounds.x - sourceBounds.w);
  const bottom = Math.max(0, groupSource.y + groupSource.h - sourceBounds.y - sourceBounds.h);
  return {
    x: targetBounds.x - left,
    y: targetBounds.y - top,
    w: targetBounds.w + left + right,
    h: targetBounds.h + top + bottom,
  };
}

/** Convert visual cube chrome into the group rectangle expected by the active host renderer. */
export function toComfyGroupStorageRect(rect: LayoutRect, renderer: NodeRenderer): LayoutRect {
  return {
    ...rect,
    h: renderer === 'vue' ? rect.h + NODE_TITLE_HEIGHT : rect.h,
  };
}
