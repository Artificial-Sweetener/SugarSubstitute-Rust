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
/** Model form-associated custom elements at JSDOM's missing ElementInternals boundary. */

/** Delegate validation to a native input while tests exercise real Fluent components. */
class BrowserFormInternals {
  readonly #host: HTMLElement;
  readonly #fallback = document.createElement('input');
  ariaReadOnly = 'false';
  ariaRequired = 'false';
  /** Bind native validation to the custom element's eventual shadow control. */
  constructor(host: HTMLElement) {
    this.#host = host;
  }
  /** Read the form associated with the mounted custom element. */
  get form(): HTMLFormElement | null {
    return this.#host.closest('form');
  }
  /** Read the browser's native validity implementation instead of inventing validation policy. */
  get validity(): ValidityState {
    return this.#control.validity;
  }
  /** Read current native validation feedback. */
  get validationMessage(): string {
    return this.#control.validationMessage;
  }
  /** Report whether the actual native input accepts validation. */
  get willValidate(): boolean {
    return this.#control.willValidate;
  }
  /** Set custom error feedback through the native form control. */
  setValidity(flags: ValidityStateFlags = {}, message = '', _anchor?: HTMLElement): void {
    this.#control.setCustomValidity(flags.customError ? message : '');
  }
  /** Keep form submission storage outside rendering and interaction tests. */
  setFormValue(
    _value: string | File | FormData | null,
    _state?: string | File | FormData | null,
  ): void {}
  /** Use the browser's validation event behavior. */
  checkValidity(): boolean {
    return this.#control.checkValidity();
  }
  /** Use the browser's validation result without platform popup rendering. */
  reportValidity(): boolean {
    return this.#control.reportValidity();
  }
  /** Resolve the official component's native input after its template mounts. */
  get #control(): HTMLInputElement {
    return this.#host.shadowRoot?.querySelector('input') ?? this.#fallback;
  }
}

if (
  typeof HTMLElement !== 'undefined' &&
  typeof HTMLElement.prototype.attachInternals !== 'function'
)
  Object.defineProperty(HTMLElement.prototype, 'attachInternals', {
    configurable: true,
    /** Provide only the unavailable browser capability; production owners remain real. */
    value(this: HTMLElement): BrowserFormInternals {
      return new BrowserFormInternals(this);
    },
  });
