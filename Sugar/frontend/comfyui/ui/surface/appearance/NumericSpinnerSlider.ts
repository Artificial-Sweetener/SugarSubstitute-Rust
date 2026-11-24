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
/**
 * The node cards' number control for surfaces SugarCubes owns: a spinbox and, for bounded
 * values, the visual style's slider beside it. It carries the same `data-sc-number-*` markers
 * as a node card's adapted ComfyUI spinbox, so every visual style draws it exactly as it draws
 * the node cards' pair, and follows a change of visual style by replacing its slider.
 */
import type { NumericSliderSpec } from './NumericSliderPresentation.js';
import { ownStyledNumericSurface, StyledNumericSlider } from './OwnedNumericSlider.js';

/** Quiet period after the last slider value before a settled drag is committed. */
const SLIDER_COMMIT_DELAY_MS = 200;

/** One mounted spinbox, with the style's slider when the value is bounded. */
export class NumericSpinnerSlider {
  readonly root: HTMLElement;
  readonly #input: HTMLInputElement;
  readonly #increment: HTMLButtonElement;
  readonly #decrement: HTMLButtonElement;
  readonly #slider: StyledNumericSlider | null;
  #value = 0;
  #disabled = false;
  #timer: ReturnType<typeof setTimeout> | null = null;

  /**
   * @param withSlider Whether a slider accompanies the spinbox; it needs finite bounds.
   * @param change Receives each committed value, already clamped and stepped.
   * @param drag `settled` commits a slider drag once it rests, so one drag is one write;
   *   `live` commits every value a drag passes, for previews that follow the drag.
   */
  constructor(
    private readonly document: Document,
    private readonly spec: NumericSliderSpec,
    withSlider: boolean,
    private readonly change: (value: number) => void,
    private readonly drag: 'settled' | 'live' = 'settled',
  ) {
    this.root = document.createElement('div');
    this.root.className = 'sc-numeric-spinner-slider';
    this.root.setAttribute('data-sc-number-pair', '');
    ownStyledNumericSurface(this.root);
    const spinner = document.createElement('div');
    spinner.setAttribute('data-sc-style-edit-frame', '');
    spinner.setAttribute('data-sc-number-spinner', '');
    spinner.setAttribute('aria-label', spec.label);
    this.#decrement = this.#stepper('Decrement', 'minus', -1);
    this.#increment = this.#stepper('Increment', 'plus', 1);
    const field = document.createElement('div');
    this.#input = document.createElement('input');
    this.#input.setAttribute('data-sc-number-input', '');
    this.#input.setAttribute('role', 'spinbutton');
    this.#input.inputMode = spec.precision === 0 ? 'numeric' : 'decimal';
    this.#input.autocomplete = 'off';
    this.#input.spellcheck = false;
    if (Number.isFinite(spec.minimum))
      this.#input.setAttribute('aria-valuemin', String(spec.minimum));
    if (Number.isFinite(spec.maximum))
      this.#input.setAttribute('aria-valuemax', String(spec.maximum));
    field.append(this.#input);
    spinner.append(this.#decrement, field, this.#increment);
    this.root.append(spinner);
    // A typed value is committed when editing finishes: Enter or leaving the field.
    this.#input.addEventListener('change', () => this.#commitText());
    this.#input.addEventListener('keydown', (event) => {
      if (event.key === 'Enter') this.#input.blur();
      else if (event.key === 'Escape') {
        this.#showText();
        this.#input.blur();
      } else if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
        event.preventDefault();
        this.#commit(this.#value + (event.key === 'ArrowUp' ? spec.step : -spec.step));
      }
    });
    this.#slider = withSlider
      ? new StyledNumericSlider(document, spec, (value) => this.#follow(value))
      : null;
    if (this.#slider) this.root.append(this.#slider.root);
  }

  /** Show a stored value; a field being edited keeps the person's text. */
  show(value: number, disabled = false): void {
    this.#value = value;
    this.#disabled = disabled;
    this.#input.disabled = disabled;
    this.#input.setAttribute('aria-valuenow', String(value));
    if (this.document.activeElement !== this.#input) this.#showText();
    this.#synchronizeSlider();
    this.#decrement.disabled = disabled || value <= this.spec.minimum;
    this.#increment.disabled = disabled || value >= this.spec.maximum;
  }

  /** Commit a pending slider value and stop following the visual style. */
  dispose(): void {
    this.#flush();
    this.#slider?.dispose();
  }

  #stepper(label: string, icon: 'minus' | 'plus', direction: 1 | -1): HTMLButtonElement {
    const button = this.document.createElement('button');
    button.type = 'button';
    button.tabIndex = -1;
    button.setAttribute('aria-label', label);
    button.setAttribute(`data-sc-number-${label.toLowerCase()}`, '');
    const glyph = this.document.createElement('i');
    // ComfyUI's own stepper glyphs, as its number widget draws them on the node cards.
    glyph.className = `icon-[lucide--${icon}]`;
    button.append(glyph);
    button.addEventListener('click', () => this.#commit(this.#value + direction * this.spec.step));
    return button;
  }

  /** A slider drag shows its value at once and commits as the drag mode says. */
  #follow(value: number): void {
    if (this.#disabled || value === this.#value) return;
    this.#value = value;
    this.#showText();
    if (this.drag === 'live') {
      this.change(value);
      return;
    }
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = setTimeout(() => this.#flush(), SLIDER_COMMIT_DELAY_MS);
  }

  #synchronizeSlider(): void {
    this.#slider?.synchronize(
      this.spec.minimum,
      this.spec.maximum,
      this.spec.step,
      this.#disabled,
      this.#value,
    );
  }

  #flush(): void {
    if (this.#timer === null) return;
    clearTimeout(this.#timer);
    this.#timer = null;
    this.change(this.#value);
  }

  #commitText(): void {
    const typed = Number(this.#input.value.trim());
    if (this.#input.value.trim() === '' || !Number.isFinite(typed)) {
      this.#showText();
      return;
    }
    this.#commit(typed);
  }

  /** Clamp to the bounds and the step grid, show it, and report it. */
  #commit(value: number): void {
    if (this.#disabled) return;
    const { minimum, maximum, step } = this.spec;
    let next = value;
    if (Number.isFinite(minimum) && step > 0)
      next = minimum + Math.round((next - minimum) / step) * step;
    if (Number.isFinite(minimum)) next = Math.max(minimum, next);
    if (Number.isFinite(maximum)) next = Math.min(maximum, next);
    next = Number(next.toFixed(this.#decimals()));
    this.show(next, this.#disabled);
    this.#showText();
    this.change(next);
  }

  #showText(): void {
    this.#input.value = this.#value.toFixed(this.#decimals());
  }

  /** The precision the field declares, or the step's own decimals. */
  #decimals(): number {
    if (this.spec.precision !== null) return this.spec.precision;
    const text = String(this.spec.step);
    const dot = text.indexOf('.');
    return dot < 0 ? 0 : Math.min(6, text.length - dot - 1);
  }
}
