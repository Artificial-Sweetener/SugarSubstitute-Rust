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
/** LoRA action assembly combines prepared trigger and host leaves outside native input and geometry callbacks. */
import type { EditorState } from '@codemirror/state';
import type { ContextMenuItem } from './context-menu-contracts.js';
import { editorFreshness } from './freshness.js';
import { loraPageHost, loraMetadataHost } from './lora-action-hosts.js';
import { prepareLoraTriggerMenu } from './lora-trigger-menu.js';
import type { LoraModelTarget, PreparedLoraMenu } from './lora-menu-targets.js';

/** Capture a logical-document UTF-16 position and exact rendered target; no external requests begin until acceptance. */
export function prepareLoraMenu(
  state: EditorState,
  position: number,
  target: LoraModelTarget | null,
): PreparedLoraMenu | null {
  const pageHost = state.facet(loraPageHost),
    metadataHost = state.facet(loraMetadataHost),
    triggers = prepareLoraTriggerMenu(state, position, target);
  const pageItem: Extract<ContextMenuItem, { kind: 'page' }> | null =
    pageHost && target?.page
      ? { kind: 'page', label: target.page.label, request: target.page }
      : null;
  const external: readonly ContextMenuItem[] = [
    ...(pageItem ? [pageItem] : []),
    ...(target?.metadata ?? []),
  ];
  const items: readonly ContextMenuItem[] = Object.freeze([
    ...triggers.items,
    ...(triggers.items.length && external.length ? [{ kind: 'separator' as const }] : []),
    ...external,
  ]);
  if (!items.length) return null;
  const context = {
    freshness: state.field(editorFreshness),
    pageHost,
    metadataHost,
    items,
    pageItem,
    schedule: triggers.schedule,
    editLeaves: triggers.leaves,
  };
  return target
    ? { ...context, ...target, kind: 'inline' }
    : { ...context, kind: 'general', from: position, to: position };
}
