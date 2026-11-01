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
/** Measure a conservative rectangular web clip for an independently presented surface. */

import type { CanvasSurfaceGeometry } from '../domain/CanvasViewPort.js';

const MAX_ANCESTORS = 128;
const CLIPPED_OVERFLOW = new Set(['auto', 'clip', 'hidden', 'scroll']);

type Clip = CanvasSurfaceGeometry['clip'];

/** Follow composed ancestry so a shadow host cannot conceal an unmeasured clipping ancestor. */
export function canvasGeometryAncestry(
  element: Element,
): { readonly elements: readonly Element[]; readonly roots: readonly ShadowRoot[] } | null {
  const ancestors: Element[] = [];
  const roots: ShadowRoot[] = [];
  let current: Element | null = element;
  while (current) {
    if (ancestors.length === MAX_ANCESTORS) return null;
    ancestors.push(current);
    if (current.assignedSlot) {
      current = current.assignedSlot;
      continue;
    }
    if (current.parentElement) {
      current = current.parentElement;
      continue;
    }
    const root = current.getRootNode();
    const Shadow: typeof ShadowRoot | undefined = current.ownerDocument.defaultView?.ShadowRoot;
    if (Shadow && root instanceof Shadow) {
      roots.push(root);
      current = root.host;
    } else current = null;
  }
  return { elements: ancestors, roots };
}

/** Keep only geometry that can be represented by the shared rectangular surface contract. */
export function canvasVisibleClip(
  element: HTMLElement,
  bounds: DOMRect,
): { readonly clip: Clip; readonly visible: boolean } {
  const empty = Object.freeze({ x: bounds.left, y: bounds.top, width: 0, height: 0 });
  const document = element.ownerDocument;
  const window = document.defaultView;
  const ancestry = canvasGeometryAncestry(element);
  if (!window || !element.isConnected || document.visibilityState === 'hidden' || !ancestry)
    return { clip: empty, visible: false };
  let left = Math.max(bounds.left, 0);
  let top = Math.max(bounds.top, 0);
  let right = Math.min(bounds.right, window.innerWidth);
  let bottom = Math.min(bounds.bottom, window.innerHeight);
  let representable = true;
  // Overflow clips only through the containing-block chain: a fixed or absolutely positioned
  // box escapes the overflow of ancestors until one of them forms its containing block.
  let escape: 'none' | 'absolute' | 'fixed' = 'none';
  for (const ancestor of ancestry.elements) {
    const style = window.getComputedStyle(ancestor);
    const clipsOverflow =
      escape === 'none' ||
      (escape === 'absolute' &&
        ((style.position !== '' && style.position !== 'static') || containsFixed(style))) ||
      (escape === 'fixed' && containsFixed(style));
    if (clipsOverflow) escape = 'none';
    if (style.position === 'fixed') escape = 'fixed';
    else if (style.position === 'absolute' && escape === 'none') escape = 'absolute';
    if (
      style.display === 'none' ||
      style.visibility === 'hidden' ||
      style.visibility === 'collapse' ||
      style.getPropertyValue('content-visibility') === 'hidden' ||
      Number(style.opacity || '1') === 0
    )
      return { clip: empty, visible: false };
    if (
      Number(style.opacity || '1') !== 1 ||
      active(style.getPropertyValue('mask-image')) ||
      active(style.getPropertyValue('-webkit-mask-image')) ||
      active(style.getPropertyValue('mask-border-source')) ||
      (style.clip !== '' && style.clip !== 'auto') ||
      active(style.getPropertyValue('mix-blend-mode')) ||
      active(style.getPropertyValue('filter')) ||
      active(style.getPropertyValue('backdrop-filter')) ||
      active(style.getPropertyValue('perspective')) ||
      !axisAligned(window, style)
    )
      representable = false;
    const path = style.getPropertyValue('clip-path');
    if (active(path)) {
      const clipped = ancestor instanceof window.HTMLElement ? insetClip(ancestor, path) : null;
      if (!clipped) representable = false;
      else {
        left = Math.max(left, clipped[0]);
        top = Math.max(top, clipped[1]);
        right = Math.min(right, clipped[2]);
        bottom = Math.min(bottom, clipped[3]);
      }
    }
    if (style.display === 'contents' || !clipsOverflow) continue;
    const paintContainment = style
      .getPropertyValue('contain')
      .split(/\s+/)
      .some((value) => value === 'paint' || value === 'strict' || value === 'content');
    const clipX = paintContainment || CLIPPED_OVERFLOW.has(style.overflowX || style.overflow);
    const clipY = paintContainment || CLIPPED_OVERFLOW.has(style.overflowY || style.overflow);
    if (!clipX && !clipY) continue;
    if (rounded(style)) representable = false;
    if (!(ancestor instanceof window.HTMLElement)) {
      representable = false;
      continue;
    }
    const rect = ancestor.getBoundingClientRect();
    if (clipX) {
      const edge = innerEdges(
        rect.left,
        rect.width,
        ancestor.offsetWidth,
        ancestor.clientLeft,
        ancestor.clientWidth,
      );
      if (!edge) representable = false;
      else {
        left = Math.max(left, edge[0]);
        right = Math.min(right, edge[1]);
      }
    }
    if (clipY) {
      const edge = innerEdges(
        rect.top,
        rect.height,
        ancestor.offsetHeight,
        ancestor.clientTop,
        ancestor.clientHeight,
      );
      if (!edge) representable = false;
      else {
        top = Math.max(top, edge[0]);
        bottom = Math.min(bottom, edge[1]);
      }
    }
  }
  const visible = bounds.width > 0 && bounds.height > 0;
  if (
    !representable ||
    ![left, top, right, bottom].every(Number.isFinite) ||
    right <= left ||
    bottom <= top
  )
    return { clip: empty, visible };
  return {
    clip: Object.freeze({
      x: left,
      y: top,
      width: Math.max(0, right - left),
      height: Math.max(0, bottom - top),
    }),
    visible,
  };
}

/** Accept only rectangular CSS inset clips without rounded corners or unknown reference boxes. */
function insetClip(
  element: HTMLElement,
  value: string,
): readonly [number, number, number, number] | null {
  const match = /^inset\(([^()]*)\)(?:\s+border-box)?$/.exec(value.trim());
  if (!match) return null;
  const tokens = match[1]?.trim().split(/\s+/) ?? [];
  if (tokens.length < 1 || tokens.length > 4) return null;
  const rect = element.getBoundingClientRect();
  if (element.offsetWidth <= 0 || element.offsetHeight <= 0 || rect.width <= 0 || rect.height <= 0)
    return null;
  const xScale = rect.width / element.offsetWidth;
  const yScale = rect.height / element.offsetHeight;
  const length = (token: string, extent: number, scale: number): number | null => {
    const parsed = /^(-?(?:\d+(?:\.\d*)?|\.\d+))(px|%)?$/.exec(token);
    if (!parsed) return null;
    const number = Number(parsed[1]);
    if (!Number.isFinite(number) || (!parsed[2] && number !== 0)) return null;
    return parsed[2] === '%' ? (number * extent) / 100 : number * scale;
  };
  const [a, b = a, c = a, d = b] = tokens;
  if (!a || !b || !c || !d) return null;
  const top = length(a, rect.height, yScale);
  const right = length(b, rect.width, xScale);
  const bottom = length(c, rect.height, yScale);
  const left = length(d, rect.width, xScale);
  if (top === null || right === null || bottom === null || left === null) return null;
  return [
    rect.left + left + xScale,
    rect.top + top + yScale,
    rect.right - right - xScale,
    rect.bottom - bottom - yScale,
  ];
}
/** Client sizes are integer-rounded, so inset their transformed clip by one CSS pixel. */
function innerEdges(
  start: number,
  extent: number,
  outer: number,
  border: number,
  inner: number,
): readonly [number, number] | null {
  if (
    ![start, extent, outer, border, inner].every(Number.isFinite) ||
    extent <= 0 ||
    outer <= 0 ||
    inner <= 0 ||
    border < 0 ||
    border + inner > outer + 1
  )
    return null;
  const scale = extent / outer;
  return [start + (border + 1) * scale, start + (border + inner - 1) * scale];
}

function active(value: string): boolean {
  return value !== '' && value !== 'none' && value !== 'normal';
}

/** Whether this box forms the containing block of fixed (and absolute) descendants. */
function containsFixed(style: CSSStyleDeclaration): boolean {
  const contain = style.getPropertyValue('contain').split(/\s+/);
  const willChange = style.getPropertyValue('will-change').split(/\s*,\s*/);
  return (
    active(style.transform) ||
    active(style.getPropertyValue('translate')) ||
    active(style.getPropertyValue('rotate')) ||
    active(style.getPropertyValue('scale')) ||
    active(style.getPropertyValue('perspective')) ||
    active(style.getPropertyValue('filter')) ||
    active(style.getPropertyValue('backdrop-filter')) ||
    contain.some((value) => ['paint', 'layout', 'strict', 'content'].includes(value)) ||
    willChange.some((value) => ['transform', 'perspective', 'filter'].includes(value))
  );
}

function rounded(style: CSSStyleDeclaration): boolean {
  return [
    style.borderRadius,
    style.borderTopLeftRadius,
    style.borderTopRightRadius,
    style.borderBottomLeftRadius,
    style.borderBottomRightRadius,
  ].some((value) =>
    value
      .split(/[\s/]+/)
      .some(
        (part) =>
          part !== '' &&
          (!Number.isFinite(Number.parseFloat(part)) || Number.parseFloat(part) !== 0),
      ),
  );
}

/** Reject rotations, skew, 3D transforms, and individual CSS rotations for this rectangular port. */
function axisAligned(window: Window & typeof globalThis, style: CSSStyleDeclaration): boolean {
  const rotate = style.getPropertyValue('rotate');
  if (active(rotate) && rotate !== '0deg') return false;
  const scale = style.getPropertyValue('scale');
  if (active(scale)) {
    const factors = scale.split(/\s+/).map(Number);
    if (factors.length > 2 || factors.some((factor) => !Number.isFinite(factor) || factor <= 0))
      return false;
  }
  const transform = style.transform;
  if (!active(transform)) return true;
  if (!window.DOMMatrixReadOnly) return false;
  try {
    const matrix = new window.DOMMatrixReadOnly(transform);
    return matrix.is2D && matrix.a > 0 && matrix.d > 0 && matrix.b === 0 && matrix.c === 0;
  } catch {
    return false;
  }
}
