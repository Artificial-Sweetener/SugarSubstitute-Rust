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
/** One native grab writer materializes complete read-prepared line or unwrapped-fragment copies with identical finite cleanup. */
import type { EditorView } from '@codemirror/view';
import type { PreparedReorderPaint } from './reorder-paint-preparation.js';
import type { ReorderGrabPaint } from './reorder-grab-contracts.js';
import type { ReorderGrabGeometry } from './reorder-grab-geometry.js';
import type { ReorderRect } from './reorder-geometry.js';
import { copyNativeReorderPaint } from './reorder-native-paint.js';
import type { ReorderPaintMaterialization } from './reorder-paint-materialization.js';
/** CSS-local dimensions and physical origins are captured only in the existing native read phase. */
export interface ReorderGrabCopy {
  readonly node: HTMLElement;
  readonly kind: 'line' | 'fragment';
  readonly rect: Pick<DOMRect, 'left' | 'top'>;
  readonly width: number;
  readonly height: number;
  readonly scaleX: number;
  readonly scaleY: number;
}
/** Exact physical fragment clips retain noncentral press and native outlines independently of copied subtree layout. */
export interface ReorderGrabClip {
  readonly rect: Pick<ReorderRect, 'left' | 'right' | 'top' | 'bottom'>;
  readonly outlineX: number;
  readonly outlineY: number;
}
/** A Scene carrier inherits its existing calibrated parent; ordinary press copies retain viewport coordinates. */
export type ReorderGrabPlacement =
  | { readonly kind: 'viewport' }
  | {
      readonly kind: 'stage';
      readonly left: number;
      readonly top: number;
      readonly scaleX: number;
      readonly scaleY: number;
    };
/** Materialize a previously admitted complete held footprint; no original style/layout read occurs after allocation. */
export function materializeReorderGrab(
  view: EditorView,
  materialization: ReorderPaintMaterialization,
  geometry: ReorderGrabGeometry,
  prepared: PreparedReorderPaint,
  copies: readonly ReorderGrabCopy[],
  clips: readonly ReorderGrabClip[],
  placement: ReorderGrabPlacement = { kind: 'viewport' },
): ReorderGrabPaint | null {
  const localX = placement.kind === 'stage' ? placement.scaleX : 1,
    localY = placement.kind === 'stage' ? placement.scaleY : 1;
  if (
    ![localX, localY].every((value) => Number.isFinite(value) && value > 0) ||
    (placement.kind === 'stage' && ![placement.left, placement.top].every(Number.isFinite))
  )
    return null;
  const proxy = view.dom.ownerDocument.createElement('span');
  try {
    proxy.className = placement.kind === 'stage' ? 'sp-reorder-motion-peer' : 'sp-reorder-proxy';
    proxy.inert = true;
    proxy.setAttribute('aria-hidden', 'true');
    Object.assign(proxy.style, {
      position: placement.kind === 'stage' ? 'absolute' : 'fixed',
      ...(placement.kind === 'stage'
        ? {
            left: `${(geometry.left - placement.left) / localX}px`,
            top: `${(geometry.top - placement.top) / localY}px`,
          }
        : {}),
      display: 'block',
      padding: '0',
      border: '0',
      borderRadius: '0',
      background: 'transparent',
      margin: '0',
      width: `${geometry.width / localX}px`,
      height: `${geometry.height / localY}px`,
      pointerEvents: 'none',
      overflow: 'visible',
    });
    // Nonzero overlapping subpaths form one clip, so nested chrome never repaints antialiased glyphs twice.
    proxy.style.clipPath = `path("${clips
      .map(({ rect, outlineX, outlineY }) => {
        const x = (rect.left - geometry.left - outlineX) / localX,
          y = (rect.top - geometry.top - outlineY) / localY;
        return `M${x} ${y}h${(rect.right - rect.left + outlineX * 2) / localX}v${(rect.bottom - rect.top + outlineY * 2) / localY}h${(rect.left - rect.right - outlineX * 2) / localX}Z`;
      })
      .join(' ')}")`;
    for (const { node, kind, rect, width, height, scaleX, scaleY } of copies) {
      const copy = copyNativeReorderPaint(
        view,
        node,
        prepared,
        materialization,
      ) as HTMLElement | null;
      if (!copy) {
        proxy.remove();
        return null;
      }
      Object.assign(copy.style, {
        position: 'absolute',
        ...(kind === 'line' ? { display: 'block' } : {}),
        boxSizing: 'border-box',
        left: `${(rect.left - geometry.left) / localX}px`,
        top: `${(rect.top - geometry.top) / localY}px`,
        margin: '0',
        width: `${width}px`,
        height: `${height}px`,
        transformOrigin: '0 0',
        transform:
          placement.kind === 'stage' && scaleX === localX && scaleY === localY
            ? 'none'
            : `scale(${scaleX / localX}, ${scaleY / localY})`,
      });
      proxy.append(copy);
    }
    return { node: proxy, geometry };
  } catch (primary) {
    try {
      proxy.remove();
    } catch (cleanup) {
      throw new AggregateError([primary, cleanup], 'Reorder passive paint cleanup failed');
    }
    throw primary;
  }
}
