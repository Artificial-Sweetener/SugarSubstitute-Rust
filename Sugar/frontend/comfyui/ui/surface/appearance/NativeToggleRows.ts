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
/** Present native booleans as captioned themed checkboxes with reversible row interaction. */

interface ToggleRowBinding {
  refresh(): void;
  release(): void;
}

export interface BooleanCheckboxControl extends HTMLElement {
  checked: boolean;
  disabled?: boolean;
}

/** Delegate row gestures to native switches without replacing their value ownership. */
export class NativeToggleRows {
  readonly #bindings = new Map<HTMLElement, ToggleRowBinding>();
  readonly #createCheckbox: (documentRef: Document) => BooleanCheckboxControl;

  /** Bind the official checkbox independently of the native boolean adapter. */
  constructor(createCheckbox: (documentRef: Document) => BooleanCheckboxControl) {
    this.#createCheckbox = createCheckbox;
  }

  /** Reconcile only positively identified native toggle fields. */
  reconcile(root: HTMLElement): void {
    for (const [row, binding] of this.#bindings) {
      if (
        !row.isConnected ||
        (root.contains(row) && !row.matches('[data-sc-style-widget="WidgetToggleSwitch"]'))
      ) {
        binding.release();
        this.#bindings.delete(row);
      }
    }
    for (const row of root.querySelectorAll<HTMLElement>(
      '[data-sc-style-row][data-sc-style-widget="WidgetToggleSwitch"]',
    )) {
      const existing = this.#bindings.get(row);
      if (existing) {
        existing.refresh();
        continue;
      }
      if (!nativeSwitch(row)) continue;
      const tabindex = row.getAttribute('tabindex');
      const caption = row.ownerDocument.createElement('span');
      caption.dataset.scBooleanCaption = '';
      caption.textContent = 'Enable';
      const checkbox = this.#createCheckbox(row.ownerDocument);
      checkbox.dataset.scBooleanCheckbox = '';
      const control = row.ownerDocument.createElement('label');
      control.dataset.scBooleanControl = '';
      control.append(checkbox, caption);
      let nativeRoot: Element | null = null;
      const refresh = (): void => {
        const input = nativeSwitch(row);
        const switchRoot = input?.closest('[data-pc-name="toggleswitch"]') ?? input;
        if (!input || !switchRoot?.parentElement) {
          nativeRoot?.removeAttribute('data-sc-boolean-native');
          nativeRoot = null;
          control.remove();
          row.removeAttribute('data-sc-toggle-row');
          if (tabindex === null) row.removeAttribute('tabindex');
          else row.setAttribute('tabindex', tabindex);
          return;
        }
        row.tabIndex = 0;
        row.dataset.scToggleRow = '';
        if (nativeRoot !== switchRoot) {
          nativeRoot?.removeAttribute('data-sc-boolean-native');
          nativeRoot = switchRoot;
          nativeRoot.setAttribute('data-sc-boolean-native', '');
        }
        if (control.nextSibling !== switchRoot)
          switchRoot.parentElement.insertBefore(control, switchRoot);
        if (checkbox.checked !== input.checked) checkbox.checked = input.checked;
        if (checkbox.disabled !== input.disabled) checkbox.disabled = input.disabled;
        const label =
          input.getAttribute('aria-label') ?? row.querySelector('label')?.textContent ?? '';
        if (checkbox.getAttribute('aria-label') !== label)
          checkbox.setAttribute('aria-label', label);
      };
      const activate = (): void => {
        const input = nativeSwitch(row);
        if (input && !input.disabled) input.click();
      };
      const change = (event: Event): void => {
        if (event.target !== checkbox) return;
        event.stopPropagation();
        const input = nativeSwitch(row);
        if (input && !input.disabled && input.checked !== checkbox.checked) input.click();
        refresh();
      };
      const retainGesture = (event: Event): void => event.stopPropagation();
      const nativeChange = (event: Event): void => {
        if (event.target === nativeSwitch(row)) refresh();
      };
      checkbox.addEventListener('change', change);
      checkbox.addEventListener('pointerdown', retainGesture);
      checkbox.addEventListener('keydown', retainGesture);
      const click = (event: Event): void => {
        if (
          !(event.target instanceof Element) ||
          event.target.closest(
            'input, button, a, label, [role="button"], [data-testid="slot-connection-dot"], [role="switch"], [data-sc-boolean-checkbox], [contenteditable="true"]',
          )
        )
          return;
        activate();
        event.stopPropagation();
      };
      const key = (event: KeyboardEvent): void => {
        if (event.target !== row || !['Enter', ' '].includes(event.key)) return;
        event.preventDefault();
        event.stopPropagation();
        activate();
      };
      row.addEventListener('click', click);
      row.addEventListener('keydown', key);
      row.addEventListener('change', nativeChange, true);
      const stateObserver = new MutationObserver(refresh);
      stateObserver.observe(row, {
        attributes: true,
        subtree: true,
        attributeFilter: ['aria-checked', 'checked', 'disabled'],
      });
      refresh();
      this.#bindings.set(row, {
        refresh,
        release: () => {
          row.removeEventListener('click', click);
          row.removeEventListener('keydown', key);
          row.removeEventListener('change', nativeChange, true);
          stateObserver.disconnect();
          checkbox.removeEventListener('change', change);
          checkbox.removeEventListener('pointerdown', retainGesture);
          checkbox.removeEventListener('keydown', retainGesture);
          control.remove();
          nativeRoot?.removeAttribute('data-sc-boolean-native');
          row.removeAttribute('data-sc-toggle-row');
          if (tabindex === null) row.removeAttribute('tabindex');
          else row.setAttribute('tabindex', tabindex);
        },
      });
    }
  }

  /** Restore host row behavior when the theme ends. */
  clear(): void {
    for (const binding of this.#bindings.values()) binding.release();
    this.#bindings.clear();
  }
}

/** Read the host-owned boolean value without claiming custom component inputs. */
function nativeSwitch(row: HTMLElement): HTMLInputElement | null {
  return row.querySelector<HTMLInputElement>(
    'input[type="checkbox"]:not([data-sc-boolean-checkbox]), input[role="switch"]',
  );
}
