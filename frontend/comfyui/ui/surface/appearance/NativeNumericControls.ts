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
/** Own reversible numeric presentation while Comfy retains widget values and callbacks. */
import type { NativeNumericField } from './ComfyNativeNumericField.js';
import { usesSubstituteSlider } from './NumericSliderPresentation.js';

export interface FluentNumericSlider extends HTMLElement {
  value: string;
  min: string;
  max: string;
  step: string;
  disabled: boolean;
}

interface NumericBinding {
  field: NativeNumericField;
  slider: FluentNumericSlider | null;
  synchronize(): void;
  release(): void;
}

/** Attach Microsoft's slider beside the existing spinbox only at recognized native boundaries. */
export class NativeNumericControls {
  readonly #cards = new Map<HTMLElement, Map<HTMLInputElement, NumericBinding>>();
  readonly #createSlider: (documentRef: Document) => FluentNumericSlider;

  /** Bind the official component factory independently of native widget adaptation. */
  constructor(createSlider: (documentRef: Document) => FluentNumericSlider) {
    this.#createSlider = createSlider;
  }

  /** Follow replaced native inputs and reapply current presentation without duplicating controls. */
  reconcile(root: HTMLElement, fields: readonly NativeNumericField[]): void {
    for (const [card, bindings] of this.#cards) {
      if (!card.isConnected) {
        for (const binding of bindings.values()) binding.release();
        this.#cards.delete(card);
      }
    }
    const bindings = this.#cards.get(root) ?? new Map<HTMLInputElement, NumericBinding>();
    this.#cards.set(root, bindings);
    for (const [input, binding] of bindings) {
      const replacement = fields.find((field) => field.input === input);
      if (
        !replacement ||
        replacement.container !== binding.field.container ||
        replacement.spinner !== binding.field.spinner ||
        (usesSubstituteSlider(replacement) && !replacement.hasNativeSlider) !==
          (binding.slider !== null)
      ) {
        binding.release();
        bindings.delete(input);
      }
    }
    for (const field of fields) {
      const existing = bindings.get(field.input);
      if (existing) {
        existing.field = field;
        existing.synchronize();
      } else bindings.set(field.input, this.#bind(field));
    }
  }

  /** Release controls, subscriptions, and annotations when the theme or renderer ends. */
  clear(): void {
    for (const bindings of this.#cards.values())
      for (const binding of bindings.values()) binding.release();
    this.#cards.clear();
  }

  /** Synchronize through native model events so all edits use Comfy's existing value path. */
  #bind(field: NativeNumericField): NumericBinding {
    const slider =
      usesSubstituteSlider(field) && !field.hasNativeSlider
        ? this.#createSlider(field.row.ownerDocument)
        : null;
    const annotations = new Map<string, HTMLElement>();
    const gestureTargets = slider ? [field.spinner, slider] : [field.spinner];
    for (const target of gestureTargets)
      target.addEventListener('pointerdown', retainNumericGesture);
    let synchronizing = false;
    let unsubscribe: () => void = () => undefined;
    const change = (): void => {
      if (synchronizing || !slider || binding.field.disabled()) return;
      const value = Number(slider.value);
      if (Number.isFinite(value) && value !== binding.field.readValue())
        binding.field.writeValue(value);
    };
    const binding: NumericBinding = {
      field,
      slider,
      synchronize: () => {
        synchronizeNumericAnnotations(binding.field, annotations);
        if (!slider) return;
        synchronizing = true;
        try {
          slider.min = String(binding.field.minimum);
          slider.max = String(binding.field.maximum);
          slider.step = String(binding.field.step);
          slider.disabled = binding.field.disabled();
          const value = binding.field.readValue();
          if (value !== null) slider.value = String(value);
        } finally {
          synchronizing = false;
        }
      },
      release: () => {
        unsubscribe();
        for (const target of gestureTargets)
          target.removeEventListener('pointerdown', retainNumericGesture);
        for (const [attribute, element] of annotations) element.removeAttribute(attribute);
        annotations.clear();
        field.container.removeAttribute('data-sc-number-pair');
        slider?.removeEventListener('change', change);
        slider?.remove();
      },
    };
    if (slider) {
      slider.setAttribute('aria-label', `${field.label} slider`);
      slider.dataset.scNumberSlider = '';
      field.container.setAttribute('data-sc-number-pair', '');
      slider.addEventListener('change', change);
      field.container.append(slider);
    }
    try {
      binding.synchronize();
      if (slider) unsubscribe = field.subscribe(binding.synchronize);
    } catch (error: unknown) {
      binding.release();
      throw error;
    }
    return binding;
  }
}

/** Follow late-mounted native buttons while retaining the numeric input and slider identities. */
function synchronizeNumericAnnotations(
  field: NativeNumericField,
  annotations: Map<string, HTMLElement>,
): void {
  const targets = [
    ['data-sc-number-spinner', field.spinner],
    ['data-sc-number-input', field.input],
    ['data-sc-number-increment', field.increment],
    ['data-sc-number-decrement', field.decrement],
    ['data-sc-native-number-fill', field.rangeFill],
  ] as const;
  for (const [attribute, element] of targets) {
    const previous = annotations.get(attribute);
    if (previous === element) continue;
    previous?.removeAttribute(attribute);
    annotations.delete(attribute);
    if (element) {
      element.setAttribute(attribute, '');
      annotations.set(attribute, element);
    }
  }
}

/** Keep primary gesture starts away from Comfy's node drag owner; Fluent retains window move/up. */
function retainNumericGesture(event: Event): void {
  if (event instanceof MouseEvent && event.button === 0) event.stopPropagation();
}
