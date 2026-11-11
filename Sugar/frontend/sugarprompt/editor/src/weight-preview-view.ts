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
/** A passive owned portal measures and paints short-lived numeric feedback outside native content. */
import type { WeightRect } from './weight-control-geometry.js';
import type { WheelPoint } from './wheel-intent.js';

/** Plain typography copied during the native read phase, with viewport scale already captured. */
export interface WeightPreviewTypography {
  readonly fontFamily: string;
  readonly fontSize: string;
  readonly color: string;
  readonly colorScheme: string;
  readonly scale: number;
}

/** Clamp measured viewport-pixel feedback above the pointer with donor margins; oversized labels use the prepared width cap. */
export function weightPreviewPlacement(
  size: { readonly width: number; readonly height: number },
  point: WheelPoint,
  viewport: WeightRect,
): WeightRect | null {
  if (
    ![
      size.width,
      size.height,
      point.x,
      point.y,
      viewport.left,
      viewport.top,
      viewport.width,
      viewport.height,
    ].every(Number.isFinite) ||
    size.width <= 0 ||
    size.height <= 0 ||
    viewport.width <= 12 ||
    viewport.height <= 12
  )
    return null;
  const width = Math.min(size.width, viewport.width - 12),
    height = Math.min(size.height, viewport.height - 12);
  return {
    left: Math.max(
      viewport.left + 6,
      Math.min(point.x - width / 2, viewport.left + viewport.width - width - 6),
    ),
    top: Math.max(
      viewport.top + 6,
      Math.min(point.y - 4 - height, viewport.top + viewport.height - height - 6),
    ),
    width,
    height,
  };
}

/** The native preview owner retains this finite DOM subtree and never writes CodeMirror-managed content. */
export class WeightPreviewView {
  readonly root: HTMLDivElement;
  /** Mount one inert literal value portal on the actual editor document. */
  constructor(document: Document) {
    this.root = document.createElement('div');
    this.root.className = 'sp-weight-preview';
    this.root.setAttribute('aria-hidden', 'true');
    Object.assign(this.root.style, {
      position: 'fixed',
      pointerEvents: 'none',
      zIndex: '41',
      padding: '1px 3.5px',
      boxSizing: 'border-box',
      whiteSpace: 'nowrap',
      overflow: 'hidden',
      textOverflow: 'ellipsis',
      textAlign: 'center',
      lineHeight: 'normal',
      textShadow: [
        '0 1px 0 var(--sugarprompt-preview-halo, light-dark(#ffffffe6, #000000d8))',
        '1px 1px 0 var(--sugarprompt-preview-halo, light-dark(#ffffffe6, #000000d8))',
        '1px 0 0 var(--sugarprompt-preview-halo, light-dark(#ffffffe6, #000000d8))',
        '-1px 1px 0 var(--sugarprompt-preview-halo, light-dark(#ffffffe6, #000000d8))',
      ].join(', '),
      left: '0',
      top: '0',
    });
    this.root.hidden = true;
    document.body.append(this.root);
  }
  /** Native write phase prepares literal text and typography before the next measured read. */
  prepare(text: string, font: WeightPreviewTypography, viewport: WeightRect): void {
    this.root.textContent = text;
    Object.assign(this.root.style, {
      fontFamily: font.fontFamily,
      fontWeight: '400',
      fontSize: `${Math.max(12, parseFloat(font.fontSize) * font.scale - 1)}px`,
      color: font.color,
      colorScheme: font.colorScheme,
      maxWidth: `${Math.max(0, viewport.width - 12)}px`,
      maxHeight: `${Math.max(0, viewport.height - 12)}px`,
      visibility: 'hidden',
    });
    this.root.hidden = false;
  }
  /** Read only owned native DOM after preparation, during CodeMirror's read phase. */
  measure(): DOMRect {
    return this.root.getBoundingClientRect();
  }
  /** Native write phase publishes prepared geometry without affecting prompt layout. */
  show(rect: WeightRect): void {
    Object.assign(this.root.style, {
      left: `${rect.left}px`,
      top: `${rect.top}px`,
      maxWidth: `${rect.width}px`,
      maxHeight: `${rect.height}px`,
      visibility: 'visible',
    });
  }
  /** Retirement releases displayed source-derived spelling and makes the portal inert. */
  hide(): void {
    this.root.hidden = true;
    this.root.textContent = '';
  }
  /** Teardown releases all retained native DOM. */
  destroy(): void {
    this.root.remove();
  }
}
