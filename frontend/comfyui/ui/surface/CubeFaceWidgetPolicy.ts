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
/** Own effective widget visibility for projected Cube-face node cards. */

import type { ComfyInput, ComfyNode, ComfyWidget } from '../types/graph.js';

/** Return widgets whose values remain owned by the node and visible through Comfy. */
export function cubeFaceVisibleWidgets(node: ComfyNode): ComfyWidget[] {
  const isWidgetVisible = node.isWidgetVisible;
  return (node.widgets ?? []).filter(
    (widget) =>
      !isCubeFaceWidgetConsumed(node, widget) &&
      (typeof isWidgetVisible !== 'function' || isWidgetVisible.call(node, widget) !== false),
  );
}

/** Return widgets whose values remain owned by the node regardless of host visibility. */
export function cubeFaceUnconsumedWidgets(node: ComfyNode): ComfyWidget[] {
  return (node.widgets ?? []).filter((widget) => !isCubeFaceWidgetConsumed(node, widget));
}

/** Return whether an incoming graph connection owns one widget's effective value. */
export function isCubeFaceWidgetConsumed(node: ComfyNode, widget: ComfyWidget): boolean {
  const widgetName = widget.name.trim();
  if (!widgetName) return false;
  return (node.inputs ?? []).some(
    (input) => inputWidgetName(input) === widgetName && inputHasConnection(input),
  );
}

/** Read the stable widget identity carried beside a converted input. */
function inputWidgetName(input: ComfyInput): string {
  return typeof input.widget?.name === 'string' ? input.widget.name.trim() : '';
}

/** Recognize both singular and multi-link host input representations. */
function inputHasConnection(input: ComfyInput): boolean {
  return input.link !== null && input.link !== undefined ? true : (input.links?.length ?? 0) > 0;
}
