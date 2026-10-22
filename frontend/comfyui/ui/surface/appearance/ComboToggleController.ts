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
/** Give ordinary and enhanced comboboxes one click-to-close policy. */

interface ComboToggleLease {
  users: number;
  onClick: (event: MouseEvent) => void;
}

const leases = new WeakMap<Document, ComboToggleLease>();

/** Install one delegated listener while any combobox presentation needs it. */
export function acquireComboToggle(documentRef: Document): () => void {
  let lease = leases.get(documentRef);
  if (!lease) {
    lease = { users: 0, onClick: (event) => closeOpenCombo(documentRef, event) };
    leases.set(documentRef, lease);
    documentRef.addEventListener('click', lease.onClick, true);
  }
  lease.users += 1;
  let released = false;
  return () => {
    if (!lease || released) return;
    released = true;
    if (--lease.users !== 0) return;
    documentRef.removeEventListener('click', lease.onClick, true);
    leases.delete(documentRef);
  };
}

/** Dismiss the active host or enhanced popup before its click handler reopens it. */
function closeOpenCombo(documentRef: Document, event: MouseEvent): void {
  if (!(event.target instanceof Element)) return;
  const inlineSearch = event.target.closest<HTMLInputElement>('[data-sc-inline-search]');
  const frame = event.target.closest<HTMLElement>(
    '[data-sc-style-combo-frame], [data-capture-wheel]',
  );
  const activation = event.target.closest<HTMLElement>(
    '[role="combobox"], .sc-model-field-arrow, button[aria-hidden="true"]',
  );
  if (!inlineSearch && (!frame || !activation || !frame.contains(activation))) return;
  const combo = inlineSearch ?? frame?.querySelector<HTMLElement>(':scope > [role="combobox"]');
  if (combo?.getAttribute('aria-expanded') !== 'true') return;
  event.preventDefault();
  event.stopImmediatePropagation();
  const popupId = combo.getAttribute('aria-controls');
  const popup = popupId ? documentRef.getElementById(popupId) : null;
  const target =
    popup?.querySelector<HTMLInputElement>('[data-sc-inline-search]') ?? popup ?? combo;
  target.dispatchEvent(
    new KeyboardEvent('keydown', {
      key: 'Escape',
      code: 'Escape',
      bubbles: true,
      cancelable: true,
    }),
  );
}
