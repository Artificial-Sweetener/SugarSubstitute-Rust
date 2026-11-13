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
import { readDocumentVisualStyle } from './VisualStyleBoundary.js';

interface ComboToggleLease {
  users: number;
  onClick: (event: MouseEvent) => void;
  onPointerDown: (event: PointerEvent) => void;
}

const leases = new WeakMap<Document, ComboToggleLease>();

/** Install one delegated listener while any combobox presentation needs it. */
export function acquireComboToggle(documentRef: Document): () => void {
  let lease = leases.get(documentRef);
  if (!lease) {
    lease = {
      users: 0,
      onClick: (event) => routeComboClick(documentRef, event),
      onPointerDown: (event) => retainCaptionToggle(documentRef, event),
    };
    leases.set(documentRef, lease);
    documentRef.addEventListener('click', lease.onClick, true);
    documentRef.addEventListener('pointerdown', lease.onPointerDown, true);
  }
  lease.users += 1;
  let released = false;
  return () => {
    if (!lease || released) return;
    released = true;
    if (--lease.users !== 0) return;
    documentRef.removeEventListener('click', lease.onClick, true);
    documentRef.removeEventListener('pointerdown', lease.onPointerDown, true);
    leases.delete(documentRef);
  };
}

/** Resolve only enabled GTK captions and field padding, leaving existing interactive children alone. */
function captionTrigger(document: Document, target: EventTarget | null): HTMLElement | null {
  if (
    readDocumentVisualStyle(document) !== 'gimp' ||
    !(target instanceof Element) ||
    target.closest('[data-sc-style-combo-frame],button,input,a,[role="button"]')
  )
    return null;
  const trigger = target
    .closest('[data-sc-style-combo-layout]')
    ?.querySelector<HTMLElement>(
      '[data-sc-style-combo-frame]:not([data-sc-model-native]) > [role="combobox"]',
    );
  return trigger && !trigger.matches(':disabled,[aria-disabled="true"]') ? trigger : null;
}

/** Keep a caption press inside its owning toggle before a popup's outside-press listener dismisses it. */
function retainCaptionToggle(document: Document, event: PointerEvent): void {
  const trigger = event.button === 0 ? captionTrigger(document, event.target) : null;
  if (!trigger) return;
  event.preventDefault();
  event.stopImmediatePropagation();
  trigger.focus({ preventScroll: true });
}

/** Activate a GIMP field's caption or dismiss its open popup through the existing native trigger. */
function routeComboClick(documentRef: Document, event: MouseEvent): void {
  if (!(event.target instanceof Element)) return;
  const trigger = captionTrigger(documentRef, event.target);
  if (trigger) {
    event.preventDefault();
    event.stopImmediatePropagation();
    trigger.focus({ preventScroll: true });
    trigger.click();
    return;
  }
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
