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
/** Bound host extraction's widget accessor installation while preserving native render data and dynamic widget ownership. */
import type { ComfyNode } from '../types/graph.js';

interface NativeWidgetDescriptor {
  readonly base: PropertyDescriptor | undefined;
  readonly installed: PropertyDescriptor | undefined;
}
const widgetDescriptors = new WeakMap<ComfyNode, NativeWidgetDescriptor>();

/** Restore only a descriptor installed by this boundary; external replacements become the next authoritative base. */
function sameDescriptor(
  left: PropertyDescriptor | undefined,
  right: PropertyDescriptor | undefined,
): boolean {
  return (
    left === right ||
    (!!left &&
      !!right &&
      left.get === right.get &&
      left.set === right.set &&
      left.value === right.value &&
      left.writable === right.writable &&
      left.configurable === right.configurable &&
      left.enumerable === right.enumerable)
  );
}

/** Invoke native extraction through the sole widget descriptor adaptation boundary. */
export function extractNativeVueNodeData(
  node: ComfyNode,
  extract: (node: ComfyNode) => unknown,
): unknown {
  const previous = widgetDescriptors.get(node);
  let base = Object.getOwnPropertyDescriptor(node, 'widgets');
  if (previous && sameDescriptor(base, previous.installed)) {
    base = previous.base;
    if (base) {
      if ('value' in base) {
        const widgets: unknown = Reflect.get(node, 'widgets');
        base = { ...base, value: widgets };
      }
      Object.defineProperty(node, 'widgets', base);
    } else if (!Reflect.deleteProperty(node, 'widgets'))
      throw new TypeError('Cannot restore native widget ownership before extraction.');
  }
  try {
    return extract(node);
  } finally {
    widgetDescriptors.set(node, {
      base,
      installed: Object.getOwnPropertyDescriptor(node, 'widgets'),
    });
  }
}
