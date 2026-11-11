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
/** Own node sizing and rendered-presentation measurement at the ComfyUI boundary. */

import { readVector2 } from '../graph/VectorUtils.js';
import { isRecord } from '../types/common.js';
import type { ComfyNode } from '../types/graph.js';
import type { Vec2 } from '../types/common.js';
import type { LayoutRect } from './AuthoredLayoutSolver.js';
import type { RendererGeometryPolicy } from './RendererGeometryPolicy.js';

export interface NodeGeometryLogger {
  warn(message: string, context: unknown): void;
}

export interface NodePresentationMeasurementContext {
  renderer: RendererGeometryPolicy['renderer'];
  document: Document | null;
}

const DEFAULT_TITLE_HEIGHT = 30;
const DEFAULT_COLLAPSED_WIDTH = 80;
const DEFAULT_TEXT_SIZE = 14;

/** Apply the authored expanded size floor expected by the active renderer. */
export function applyComfyNodeSize(
  node: ComfyNode,
  authoredSize: unknown,
  policy: RendererGeometryPolicy,
  _logger: NodeGeometryLogger | null = null,
): Vec2 {
  const current = readVector2(node.size, 0, 0);
  const authored = readVector2(authoredSize, current[0], current[1]);
  const target: Vec2 = [Math.max(authored[0], policy.minimumExpandedNodeWidth), authored[1]];
  writeNodeSize(node, target);
  return target;
}

/** Estimate the renderer-neutral collapsed title rectangle from authored text. */
export function estimateCollapsedPresentationWidth(title: string, expandedWidth: number): number {
  const measuredTitle = title.length * DEFAULT_TEXT_SIZE * 0.6 + DEFAULT_TITLE_HEIGHT * 2;
  return Math.min(
    expandedWidth,
    Math.max(DEFAULT_COLLAPSED_WIDTH, Number.isFinite(measuredTitle) ? measuredTitle : 0),
  );
}

/** Convert a persisted node rect into its authored presentation rectangle. */
export function authoredNodePresentationRect(
  rect: LayoutRect,
  collapsed: boolean,
  title = '',
): LayoutRect {
  if (!collapsed) return { ...rect };
  return {
    x: rect.x,
    y: rect.y - DEFAULT_TITLE_HEIGHT,
    w: estimateCollapsedPresentationWidth(title, rect.w),
    h: DEFAULT_TITLE_HEIGHT,
  };
}

/** Read the host-measured presentation rectangle without mutating node geometry. */
export function measureNodePresentationRect(
  node: ComfyNode,
  context: NodePresentationMeasurementContext = { renderer: 'litegraph', document: null },
): LayoutRect | null {
  const pos = readVector2(node.pos, Number.NaN, Number.NaN);
  const size = readVector2(node.size, Number.NaN, Number.NaN);
  if (![...pos, ...size].every(Number.isFinite)) return null;
  const collapsed = isRecord(node.flags) && node.flags.collapsed === true;
  const renderedSize =
    context.renderer === 'vue' ? readMountedNodeSize(node, context.document) : null;
  if (!collapsed) {
    return {
      x: pos[0],
      y: context.renderer === 'vue' ? pos[1] - DEFAULT_TITLE_HEIGHT : pos[1],
      w: renderedSize?.[0] ?? size[0],
      h: renderedSize?.[1] ?? size[1],
    };
  }
  const renderingSize = readVector2(node.renderingSize, size[0], 0);
  return {
    x: pos[0],
    y: pos[1] - DEFAULT_TITLE_HEIGHT,
    w: renderedSize?.[0] ?? (renderingSize[0] > 0 ? renderingSize[0] : size[0]),
    h: renderedSize?.[1] ?? DEFAULT_TITLE_HEIGHT,
  };
}

/** Report whether Comfy has mounted a measurable Vue card for the node. */
export function hasMountedNodePresentation(node: ComfyNode, documentRef: Document | null): boolean {
  return readMountedNodeSize(node, documentRef) !== null;
}

/** Write a solved presentation origin back to Comfy's node-position convention. */
export function writeNodePresentationPosition(
  node: ComfyNode,
  rect: LayoutRect,
  renderer: RendererGeometryPolicy['renderer'] = 'litegraph',
): void {
  const collapsed = isRecord(node.flags) && node.flags.collapsed === true;
  const usesTitleOffset = renderer === 'vue' || collapsed;
  const target: Vec2 = [rect.x, usesTitleOffset ? rect.y + DEFAULT_TITLE_HEIGHT : rect.y];
  if (Array.isArray(node.pos)) {
    node.pos[0] = target[0];
    node.pos[1] = target[1];
  } else {
    node.pos = target;
  }
}

/** Write one negotiated size through the host API while preserving vector identity. */
function writeNodeSize(node: ComfyNode, target: Vec2): void {
  if (typeof node.setSize === 'function') {
    node.setSize(target);
  } else if (Array.isArray(node.size)) {
    node.size[0] = target[0];
    node.size[1] = target[1];
    node.onResize?.(target);
  } else {
    node.size = target;
    node.onResize?.(target);
  }
}

/** Read the mounted Vue card's layout size before canvas zoom is applied. */
function readMountedNodeSize(node: ComfyNode, documentRef: Document | null): Vec2 | null {
  if (!documentRef || node.id == null) return null;
  const identity = String(node.id);
  const elements = documentRef.querySelectorAll<HTMLElement>('[data-node-id]');
  for (const element of elements) {
    if (element.getAttribute('data-node-id') !== identity) continue;
    const width = Number(element.offsetWidth);
    const height = Number(element.offsetHeight);
    return width > 0 && height > 0 ? [width, height] : null;
  }
  return null;
}
