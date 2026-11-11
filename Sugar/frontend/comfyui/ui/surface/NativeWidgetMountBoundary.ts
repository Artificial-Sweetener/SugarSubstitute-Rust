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
/** Expose a mounted native widget to the shared appearance owner without graph identity. */
import type { UnknownRecord } from '../types/common.js';
import type { NativeSeedControl } from './appearance/ComfyNativeSeedControl.js';

/** Describe native renderer capabilities supplied by a standalone widget mount. */
export interface NativeWidgetMountBoundary {
  vnode: UnknownRecord;
  seed: NativeSeedControl | null;
}

const mounts = new WeakMap<HTMLElement, NativeWidgetMountBoundary>();

/** Register the actual Vue mount and optional seed value owner for shared appearance. */
export function registerNativeWidgetMount(
  root: HTMLElement,
  boundary: NativeWidgetMountBoundary,
): void {
  mounts.set(root, boundary);
}

/** Resolve only positively registered native widget mounts. */
export function readNativeWidgetMount(root: HTMLElement): NativeWidgetMountBoundary | null {
  return mounts.get(root) ?? null;
}

/** Release renderer ownership before removing its DOM. */
export function releaseNativeWidgetMount(root: HTMLElement): void {
  mounts.delete(root);
}
