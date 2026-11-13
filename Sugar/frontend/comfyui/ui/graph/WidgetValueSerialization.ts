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
/** Define JSON-safe Comfy widget serialization behavior. */

import type { ComfyWidget } from '../types/graph.js';

/** Return whether Comfy persists a widget value in workflow data. */
export function isSerializedWidget(widget: ComfyWidget | null | undefined): boolean {
  if (!widget || typeof widget.name !== 'string' || !widget.name.trim()) {
    return false;
  }
  if (widget.serialize === false || widget.options?.serialize === false) {
    return false;
  }
  return widget.type !== 'button';
}

/** Clone a dynamic widget value into JSON-compatible workflow data. */
export function cloneWidgetValue(value: unknown): unknown | undefined {
  if (value === undefined || typeof value === 'function' || typeof value === 'symbol') {
    return undefined;
  }
  try {
    const cloned: unknown = JSON.parse(JSON.stringify(value));
    return cloned;
  } catch (_error) {
    return undefined;
  }
}

/** Read and clone the current host value used when no portable value applies. */
export function readCurrentWidgetValue(widget: ComfyWidget | null | undefined): unknown {
  return cloneWidgetValue(widget?.value ?? widget?.last_value ?? widget?.options?.value) ?? null;
}
