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
/** Align the existing override menu with the native title bar in either graph renderer. */
import { isRecord } from '../../types/common.js';
import type { ComfyApplication, ComfyNode } from '../../types/graph.js';

/** Translate settled host title geometry into the DOM widget's local coordinates. */
export function placeNativeOverridePin(
  root: HTMLElement,
  node: ComfyNode,
  app: ComfyApplication | null,
  runtime: unknown,
): void {
  if (!root.hasAttribute('data-sc-overrides-graph')) return;
  const pin = root.querySelector<HTMLElement>('.sc-overrides-menu');
  const bounds = root.getBoundingClientRect();
  if (!pin || root.offsetWidth <= 0 || bounds.width <= 0) return;
  const scale = bounds.width / root.offsetWidth;
  const header = root.closest('.lg-node')?.querySelector<HTMLElement>('.lg-node-header');
  let center: number;
  let right: number;
  if (header) {
    const title = header.getBoundingClientRect();
    center = title.top + title.height / 2;
    right = title.right;
  } else {
    const canvas = app?.canvas?.canvas;
    const transform = app?.canvas?.ds;
    if (
      !canvas ||
      !isRecord(transform) ||
      typeof transform.scale !== 'number' ||
      !Number.isFinite(transform.scale) ||
      transform.scale <= 0
    )
      return;
    const offset = isRecord(transform.offset) ? transform.offset : {};
    const x = node.pos?.[0];
    const y = node.pos?.[1];
    const width = node.size?.[0];
    if (x === undefined || y === undefined || width === undefined) return;
    const nativeHeight = isRecord(runtime) ? runtime.NODE_TITLE_HEIGHT : null;
    const titleHeight = typeof nativeHeight === 'number' ? nativeHeight : 30;
    const origin = canvas.getBoundingClientRect();
    const offsetX = typeof offset[0] === 'number' ? offset[0] : 0;
    const offsetY = typeof offset[1] === 'number' ? offset[1] : 0;
    center = origin.top + (y + offsetY - titleHeight / 2) * transform.scale;
    right = origin.left + (x + offsetX + width) * transform.scale;
  }
  root.style.setProperty(
    '--sc-overrides-pin-top',
    `${(center - bounds.top) / scale - pin.offsetHeight / 2}px`,
  );
  root.style.setProperty('--sc-overrides-pin-right', `${(bounds.right - right) / scale + 10}px`);
}
