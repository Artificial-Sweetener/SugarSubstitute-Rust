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
/** Keep native field enhancements inside the visibility lifetime of their source control. */
const enhancements = new WeakMap<HTMLElement, HTMLElement>();

/** Register exactly one enhancement and inherit any existing relation suppression. */
export function registerNativeFieldEnhancement(
  anchor: HTMLElement,
  replacement: HTMLElement,
): () => void {
  if (enhancements.has(anchor)) throw new Error('Native field already owns an enhancement.');
  enhancements.set(anchor, replacement);
  anchor.setAttribute('data-sc-model-native', '');
  refreshEnhancement(anchor);
  let registered = true;
  return () => {
    if (!registered) return;
    registered = false;
    if (enhancements.get(anchor) !== replacement) return;
    enhancements.delete(anchor);
    anchor.removeAttribute('data-sc-model-native');
    replacement.hidden = false;
  };
}

/** Suppress a field and all of its enhancements when another control owns its value presentation. */
export function setNativeFieldSuppressed(control: HTMLElement, suppressed: boolean): void {
  if (suppressed) control.setAttribute('data-sc-relation-hidden', 'true');
  else control.removeAttribute('data-sc-relation-hidden');
  refreshEnhancement(control);
  for (const anchor of control.querySelectorAll<HTMLElement>('[data-sc-model-native]'))
    refreshEnhancement(anchor);
}

/** Resolve nested suppression so releasing a row cannot reveal a still-replaced control. */
function refreshEnhancement(anchor: HTMLElement): void {
  const replacement = enhancements.get(anchor);
  if (replacement) replacement.hidden = anchor.closest('[data-sc-relation-hidden="true"]') !== null;
}
