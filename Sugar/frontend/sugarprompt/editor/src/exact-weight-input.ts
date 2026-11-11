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
/** A visibly editable native numeric input shares its committed widget cell's actual layout and typography. */
import type { WeightSubject } from './weight-target.js';
import { weightValue } from './weight-value.js';
/** Native numeric editing permits intermediate signs/dots and at most two decimal digits. */
export function validWeightDraft(value: string): boolean {
  return /^-?\p{Decimal_Number}*(?:\.\p{Decimal_Number}{0,2})?$/u.test(value);
}
/** The native input owns temporary text/caret/history; an inert glyph meter sizes the same widget cell. */
export class ExactWeightInput {
  readonly element: HTMLInputElement;
  private readonly lifetime = new AbortController();
  /** Build one detached widget subtree; CodeMirror alone mounts and removes its parent cell. */
  constructor(
    cell: HTMLElement,
    kind: WeightSubject['kind'],
    value: string,
    finish: (completion: 'keyboard' | 'outside' | 'space', cancel: boolean) => void,
  ) {
    const input = (this.element = document.createElement('input')),
      meter = weightValue(value);
    input.type = 'text';
    input.inputMode = 'decimal';
    input.autocomplete = 'off';
    input.spellcheck = false;
    input.className = 'sp-exact-weight-input';
    input.setAttribute('aria-label', `Exact ${kind === 'lora' ? 'LoRA' : 'emphasis'} weight`);
    input.value = value;
    meter.setAttribute('aria-hidden', 'true');
    Object.assign(meter.style, {
      gridArea: '1 / 1',
      visibility: 'hidden',
      whiteSpace: 'pre',
      pointerEvents: 'none',
    });
    Object.assign(input.style, {
      gridArea: '1 / 1',
      width: '0',
      minWidth: 'calc(100% + 2 * var(--sp-weight-cell-padding, 0px))',
      height: '100%',
      minHeight: '0',
      boxSizing: 'border-box',
      margin: '0 calc(-1 * var(--sp-weight-cell-padding, 0px))',
      padding: '0 var(--sp-weight-cell-padding, 0px)',
      border: '0',
      borderRadius: '2px',
      background: 'inherit',
      outline: '1px solid #4f6df5',
      fontFamily: 'inherit',
      fontSize: 'inherit',
      fontStyle: 'inherit',
      fontWeight: 'inherit',
      fontStretch: 'inherit',
      fontVariant: 'inherit',
      fontVariationSettings: 'inherit',
      fontKerning: 'inherit',
      fontFeatureSettings: 'inherit',
      letterSpacing: 'inherit',
      lineHeight: 'inherit',
      textAlign: 'inherit',
      textShadow: 'inherit',
      color: 'inherit',
    });
    const options = { signal: this.lifetime.signal };
    /** Validate proposed insertion against the authoritative native selection. */
    const permitted = (text: string): boolean =>
      validWeightDraft(
        input.value.slice(0, input.selectionStart ?? 0) +
          text +
          input.value.slice(input.selectionEnd ?? 0),
      );
    input.addEventListener(
      'beforeinput',
      (event) => {
        if (
          event instanceof InputEvent &&
          !event.isComposing &&
          event.data !== null &&
          !permitted(event.data)
        )
          event.preventDefault();
      },
      options,
    );
    input.addEventListener(
      'paste',
      (event) => {
        if (!permitted(event.clipboardData?.getData('text/plain') ?? '')) event.preventDefault();
      },
      options,
    );
    input.addEventListener(
      'drop',
      (event) => {
        if (!validWeightDraft(event.dataTransfer?.getData('text/plain') ?? ''))
          event.preventDefault();
      },
      options,
    );
    input.addEventListener(
      'input',
      () => {
        meter.textContent = input.value;
      },
      options,
    );
    input.addEventListener(
      'keydown',
      (event) => {
        if (!event.isComposing && ['Enter', 'Escape', ' '].includes(event.key)) {
          event.preventDefault();
          finish(event.key === ' ' ? 'space' : 'keyboard', event.key === 'Escape');
        }
      },
      options,
    );
    input.addEventListener('blur', () => finish('outside', false), options);
    cell.append(meter, input);
  }
  /** Select once after the native widget is connected; flow changes never reselect its value. */
  focus(): void {
    this.element.focus({ preventScroll: true });
    this.element.select();
  }
  /** Widget/interaction teardown cancels listeners while CodeMirror owns subtree removal. */
  destroy(): void {
    this.lifetime.abort();
  }
}
