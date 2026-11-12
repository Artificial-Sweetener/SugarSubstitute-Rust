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
/** Model only Comfy's native widget creation/removal boundary for source controls. */
import type { ComfyNode, ComfyWidget } from '../../../frontend/comfyui/ui/types/graph.js';
import { nativePresentationWidgets } from '../../../frontend/comfyui/ui/surface/NativePresentationWidgets.js';
import { NativeNodeFields } from '../../../frontend/comfyui/ui/surface/NativeNodeFields.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';

/** Retain native widget identity while deterministic tests exercise their owning source state. */
export function installNativeNodeFields(node: ComfyNode): void {
  node.addCustomWidget = (value: unknown): ComfyWidget => {
    if (!isRecord(value) || typeof value.name !== 'string')
      throw new Error('Invalid native widget.');
    const widget: ComfyWidget = { ...value, name: value.name };
    (node.widgets ??= []).push(widget);
    return widget;
  };
  node.removeWidget = (widget: ComfyWidget): void => {
    const index = node.widgets?.indexOf(widget) ?? -1;
    if (index < 0) throw new Error('Unknown native widget.');
    node.widgets?.splice(index, 1);
  };
}
/** Supply the production native node-field owner with a deterministic host widget lifecycle. */
export function nativeInputFields(node: ComfyNode): NativeNodeFields {
  installNativeNodeFields(node);
  return new NativeNodeFields(node, 'image', document);
}
/** Read the actual native widget supplied to the node's ordinary renderer. */
export function nativeSourceField(node: ComfyNode, field = 'kind'): ComfyWidget {
  const key = field === 'kind' ? 'Source' : 'Layer';
  const widget = nativePresentationWidgets(node).find((widget) => widget.name.endsWith(':' + key));
  if (!widget) throw new Error('Missing native source field.');
  return widget;
}
/** Commit through the native widget callback used by Comfy's node field renderer. */
export function chooseNativeSource(node: ComfyNode, value: string, field = 'kind'): void {
  const widget = nativeSourceField(node, field);
  widget.value = value;
  widget.callback?.(value);
}
