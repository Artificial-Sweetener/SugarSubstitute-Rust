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
/** Carry Comfy's completed native-choice refresh to still-mounted projected cards. */
import type { ComfyNode } from '../types/graph.js';
import { isRecord } from '../types/common.js';

const eventName = 'sugarcubes-native-node-card-change';

export type NativeNodeCardChange =
  | { readonly kind: 'nodes'; readonly nodes: readonly ComfyNode[] }
  | { readonly kind: 'reset'; readonly reason: 'native-choices' };

/** Refresh copied Vue field data without replacing a card's node or interaction identity. */
export function refreshNativeNodeCards(document: Document, change: NativeNodeCardChange): void {
  document.dispatchEvent(new CustomEvent<NativeNodeCardChange>(eventName, { detail: change }));
}

/** Scope native-data invalidation to the projected card's exact mount lifetime. */
export function subscribeNativeNodeCardRefresh(
  document: Document,
  nodes: readonly ComfyNode[],
  refresh: () => void,
): () => void {
  const listener = (event: Event): void => {
    if (!(event instanceof CustomEvent)) return;
    const change: unknown = event.detail;
    if (!isRecord(change)) return;
    if (change.kind === 'reset' && change.reason === 'native-choices') refresh();
    else if (
      change.kind === 'nodes' &&
      Array.isArray(change.nodes) &&
      change.nodes.some((node: unknown) => nodes.some((owner) => owner === node))
    )
      refresh();
  };
  document.addEventListener(eventName, listener);
  return () => document.removeEventListener(eventName, listener);
}
