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
/** Attach an owned command to the value area of an existing native text field. */
import { nativeNodeFieldRows } from './NativeNodeFieldRows.js';
import { CommandButtonMount } from './appearance/CommandButtonMount.js';
import { ensureOwnedControlAppearanceStyles } from './appearance/OwnedControlAppearanceStyles.js';

/** Preserve native labels, inputs and field layout while a trailing command follows renderer replacement. */
export class NativeFieldActionMount {
  readonly #button: CommandButtonMount;
  #value: HTMLElement | null = null;

  /** Use the shared visual provider for the action without creating another field renderer. */
  constructor(
    private readonly root: HTMLElement,
    label: string,
    action: () => void,
  ) {
    ensureOwnedControlAppearanceStyles(root.ownerDocument);
    this.#button = new CommandButtonMount(root.ownerDocument, label, action);
  }

  /** Follow only this field's identified native row during its existing scoped refresh lifecycle. */
  update(name: string, disabled: boolean): void {
    this.#button.setDisabled(disabled);
    const row = nativeNodeFieldRows(this.root).get(name);
    const input = row?.querySelector<HTMLInputElement>('input:not([type="checkbox"])');
    const value = input?.parentElement ?? null;
    if (this.#value !== value) {
      this.#value?.removeAttribute('data-sc-field-action');
      this.#button.element.remove();
      this.#value = value;
    }
    if (value) {
      value.dataset.scFieldAction = '';
      if (this.#button.element.parentElement !== value) value.append(this.#button.element);
    }
  }

  /** Release the command without changing the native input or its surrounding field row. */
  dispose(): void {
    this.#value?.removeAttribute('data-sc-field-action');
    this.#button.dispose();
    this.#value = null;
  }
}
