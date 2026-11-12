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
/** Publish the accepted Plugin Mode presentation to independent browser surface owners. */
const ATTRIBUTE = 'data-sc-plugin-mode';
const CHANGE = 'sugarcubes:plugin-mode-change';

/** Read the single accepted presentation marker without observing unrelated DOM. */
export function readPluginMode(document: Document): boolean {
  return document.documentElement.hasAttribute(ATTRIBUTE);
}

/** Publish only validated preferences from the Plugin Mode setting owner. */
export function publishPluginMode(document: Document, enabled: boolean): void {
  if (readPluginMode(document) === enabled) return;
  document.documentElement.toggleAttribute(ATTRIBUTE, enabled);
  const EventConstructor = document.defaultView?.Event;
  if (EventConstructor) document.dispatchEvent(new EventConstructor(CHANGE));
}

/** Follow named mode transitions without graph discovery or mutation observation. */
export function subscribePluginMode(document: Document, changed: () => void): () => void {
  document.addEventListener(CHANGE, changed);
  return () => document.removeEventListener(CHANGE, changed);
}
