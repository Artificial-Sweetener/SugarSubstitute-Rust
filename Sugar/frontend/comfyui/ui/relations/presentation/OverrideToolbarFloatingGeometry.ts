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
/** Match native Run edge anchoring without coupling placement intent to browser geometry APIs. */
import type {
  OverrideToolbarFloatingAnchor,
  OverrideToolbarPosition,
} from './OverrideToolbarPlacementState.js';

export interface OverrideToolbarFloatingBounds {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

export interface OverrideToolbarSize {
  readonly width: number;
  readonly height: number;
}

/** Capture the user's settled position relative to the nearest available workflow edge. */
export function captureOverrideToolbarAnchor(
  position: OverrideToolbarPosition,
  bounds: OverrideToolbarFloatingBounds,
  size: OverrideToolbarSize,
): OverrideToolbarFloatingAnchor {
  const edges: readonly { edge: OverrideToolbarFloatingAnchor['edge']; distance: number }[] = [
    { edge: 'left', distance: position.x - bounds.left },
    { edge: 'right', distance: bounds.left + bounds.width - position.x - size.width },
    { edge: 'top', distance: position.y - bounds.top },
    { edge: 'bottom', distance: bounds.top + bounds.height - position.y - size.height },
  ];
  const nearest = edges.reduce((best, candidate) =>
    candidate.distance < best.distance ? candidate : best,
  );
  return {
    edge: nearest.edge,
    distance: Math.max(0, nearest.distance),
    horizontalRatio: Math.max(
      0,
      Math.min(1, (position.x - bounds.left) / Math.max(1, bounds.width)),
    ),
    verticalRatio: Math.max(0, Math.min(1, (position.y - bounds.top) / Math.max(1, bounds.height))),
  };
}

/** Resolve remembered placement against current bounds while retaining its edge distance. */
export function resolveOverrideToolbarAnchor(
  anchor: OverrideToolbarFloatingAnchor,
  bounds: OverrideToolbarFloatingBounds,
  size: OverrideToolbarSize,
): OverrideToolbarPosition {
  let x = bounds.left + anchor.horizontalRatio * bounds.width;
  let y = bounds.top + anchor.verticalRatio * bounds.height;
  if (anchor.edge === 'left') x = bounds.left + anchor.distance;
  else if (anchor.edge === 'right') x = bounds.left + bounds.width - size.width - anchor.distance;
  else if (anchor.edge === 'top') y = bounds.top + anchor.distance;
  else y = bounds.top + bounds.height - size.height - anchor.distance;
  return { x, y };
}
