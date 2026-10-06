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
const eventName = 'sugarcubes-native-node-cards-refresh';

/** Refresh copied Vue field data without replacing a card's node or interaction identity. */
export function refreshNativeNodeCards(document: Document): void {
  document.dispatchEvent(new Event(eventName));
}

/** Scope native-data invalidation to the projected card's exact mount lifetime. */
export function subscribeNativeNodeCardRefresh(
  document: Document,
  refresh: () => void,
): () => void {
  document.addEventListener(eventName, refresh);
  return () => document.removeEventListener(eventName, refresh);
}
