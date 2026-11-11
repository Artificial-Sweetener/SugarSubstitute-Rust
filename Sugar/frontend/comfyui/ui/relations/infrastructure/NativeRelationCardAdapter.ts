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
/** Identify native field rows without using mounted DOM as the candidate inventory. */
import type { ComfyNode } from '../../types/graph.js';
import { isRecord } from '../../types/common.js';
import { nativeNodeFieldRows } from '../../surface/NativeNodeFieldRows.js';
import { readNativeNodeCard } from '../../surface/NativeNodeCardBoundary.js';
import { CUBE_FACE_PROJECTION_SYMBOL } from '../../surface/ComfyLiteGraphCubeFaceProjectionHost.js';
import { cubeFacePresentedWidgets } from '../../surface/CubeFaceWidgetPolicy.js';
import type {
  RelationCardSurface,
  RelationCanvasField,
} from '../presentation/RelationCardSurface.js';

/** Resolve already-mounted views of one eligible face node in either native renderer. */
export function nativeRelationCards(
  document: Document,
  node: ComfyNode,
  cubeId: string | null,
): RelationCardSurface[] {
  const cells = [
    ...document.querySelectorAll<HTMLElement>('.sugarcubes-cube-face__node-card'),
  ].filter((cell) => {
    const boundary = readNativeNodeCard(cell);
    return boundary?.node === node && boundary.cubeId === cubeId;
  });
  const roots =
    cubeId !== null
      ? cells
      : [
          ...cells,
          ...[...document.querySelectorAll<HTMLElement>('.lg-node[data-node-id]')].filter(
            (root) => root.dataset.nodeId === String(node.id),
          ),
        ];
  const projection: unknown = Reflect.get(node, CUBE_FACE_PROJECTION_SYMBOL);
  if (
    isRecord(projection) &&
    projection.container instanceof HTMLElement &&
    typeof projection.projectRect === 'function'
  ) {
    const projectRect = projection.projectRect;
    const root = projection.container;
    const width = Number.parseFloat(root.style.width);
    const bounds = root.getBoundingClientRect();
    const scale = bounds.width / width;
    const fields = canvasFields(node, width, (y) => {
      const rect: unknown = projectRect.call(projection, [0, y, 1, 1]);
      return isRecord(rect) && typeof rect.top === 'number' && scale > 0
        ? (rect.top - bounds.top) / scale
        : y;
    });
    return [
      {
        root,
        header: null,
        rows: new Map(),
        canvas: null,
        canvasFields: fields,
        canvasTitle: true,
      },
    ];
  }
  return roots.map((root) => {
    const card = root.matches('.lg-node') ? root : root.querySelector<HTMLElement>('.lg-node');
    const canvas = root.querySelector<HTMLCanvasElement>('canvas[data-cube-face-native="nodes-1"]');
    const header = card?.querySelector<HTMLElement>('.lg-node-header') ?? null;
    const rows = nativeNodeFieldRows(root);
    const aspect = canvas?.style.aspectRatio.split('/').map((value) => Number.parseFloat(value));
    const width = aspect?.[0] ?? (canvas ? root.clientWidth : 0);
    const height = aspect?.[1] ?? (canvas ? root.clientHeight : 0);
    const ratio = canvas && width > 0 ? canvas.getBoundingClientRect().width / width : 1;
    const canvasTop = canvas
      ? canvas.getBoundingClientRect().top - root.getBoundingClientRect().top
      : 0;
    const fields = canvas
      ? canvasFields(node, width, (y) => canvasTop + (30 + y) * ratio, ratio)
      : new Map<string, RelationCanvasField>();
    return {
      nativeWidgetVisibility: readNativeNodeCard(root) !== null,
      root,
      header,
      rows,
      canvas,
      canvasFields: fields,
      canvasTitle: canvas !== null && height > 0,
    };
  });
}

/** Read actual arranged LiteGraph widget positions for native DOM selectors on canvas faces. */
function canvasFields(
  node: ComfyNode,
  width: number,
  top: (y: number) => number,
  scale = 1,
): ReadonlyMap<string, RelationCanvasField> {
  const fields = new Map<string, RelationCanvasField>();
  for (const widget of cubeFacePresentedWidgets(node)) {
    const y = typeof widget.y === 'number' ? widget.y : widget.last_y;
    if (typeof y !== 'number' || !Number.isFinite(y)) continue;
    fields.set(widget.name, {
      top: top(y),
      left: 12 * scale,
      width: Math.max(1, width - 24) * scale,
      height: 24 * scale,
    });
  }
  return fields;
}
