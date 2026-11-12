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
/** Generated protocol validation; edit the authoritative panel.schema.json. */
import type * as Contract from '../contracts/PanelProtocol.generated.js';
/** Narrow external records without allowing arrays. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}
/** Validate a bounded PanelPhotoProPalette payload before dispatch. */
export function isPanelPhotoProPalette(value: unknown): value is Contract.PanelPhotoProPalette {
  return (
    value === 'black' || value === 'dark-gray' || value === 'medium-gray' || value === 'light-gray'
  );
}
/** Validate a bounded PanelPresentation payload before dispatch. */
export function isPanelPresentation(value: unknown): value is Contract.PanelPresentation {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 + (Object.hasOwn(value, 'photoProPalette') ? 1 : 0) &&
    Object.hasOwn(value, 'pluginMode') &&
    typeof value['pluginMode'] === 'boolean' &&
    Object.hasOwn(value, 'visualStyle') &&
    (value['visualStyle'] === 'comfy' ||
      value['visualStyle'] === 'fluent' ||
      value['visualStyle'] === 'spectrum' ||
      value['visualStyle'] === 'photopro' ||
      value['visualStyle'] === 'gimp') &&
    (!Object.hasOwn(value, 'photoProPalette') ||
      isPanelPhotoProPalette(value['photoProPalette'])) &&
    Object.hasOwn(value, 'accentColor') &&
    ((typeof value['accentColor'] === 'string' &&
      value['accentColor'].length <= 7 &&
      new RegExp('^#[a-fA-F0-9]{6}$').test(value['accentColor'])) ||
      value['accentColor'] === null) &&
    Object.hasOwn(value, 'resizeCornerSize') &&
    typeof value['resizeCornerSize'] === 'number' &&
    Number.isSafeInteger(value['resizeCornerSize']) &&
    value['resizeCornerSize'] >= 0 &&
    value['resizeCornerSize'] <= 64
  );
}
/** Validate a bounded PanelReady payload before dispatch. */
export function isPanelReady(value: unknown): value is Contract.PanelReady {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.panel' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 3 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'ready'
  );
}
/** Validate a bounded PanelConfigure payload before dispatch. */
export function isPanelConfigure(value: unknown): value is Contract.PanelConfigure {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.panel' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 3 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'configure' &&
    Object.hasOwn(value, 'presentation') &&
    isPanelPresentation(value['presentation'])
  );
}
/** Validate a bounded PanelApplied payload before dispatch. */
export function isPanelApplied(value: unknown): value is Contract.PanelApplied {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.panel' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 3 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'applied'
  );
}
/** Validate a bounded PanelPing payload before dispatch. */
export function isPanelPing(value: unknown): value is Contract.PanelPing {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.panel' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 3 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'ping'
  );
}
/** Validate a bounded PanelPong payload before dispatch. */
export function isPanelPong(value: unknown): value is Contract.PanelPong {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.panel' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 3 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'pong' &&
    Object.hasOwn(value, 'connected') &&
    typeof value['connected'] === 'boolean'
  );
}
/** Validate a bounded PanelRejected payload before dispatch. */
export function isPanelRejected(value: unknown): value is Contract.PanelRejected {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.panel' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 3 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'rejected' &&
    Object.hasOwn(value, 'reason') &&
    (value['reason'] === 'unsupported-presentation' || value['reason'] === 'presentation-failed')
  );
}
/** Validate a bounded PanelMessage payload before dispatch. */
export function isPanelMessage(value: unknown): value is Contract.PanelMessage {
  return (
    isPanelReady(value) ||
    isPanelConfigure(value) ||
    isPanelApplied(value) ||
    isPanelPing(value) ||
    isPanelPong(value) ||
    isPanelRejected(value)
  );
}
