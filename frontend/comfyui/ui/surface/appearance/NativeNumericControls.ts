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
import { usesNumericSlider, type NumericSliderSpec } from './NumericSliderPresentation.js';
import { readOwnedVisualStyle } from './VisualStyleBoundary.js';
import { formatDisplayLabel } from './DisplayLabels.js';

export interface NumericSliderControl {
  readonly root: HTMLElement;
  readValue(): number;
  synchronize(
    minimum: number,
    maximum: number,
    step: number,
    disabled: boolean,
    value: number | null,
  ): void;
}

interface NumericBinding {
  field: NativeNumericField;
  slider: NumericSliderControl | null;
  synchronize(): void;
  release(): void;
}

/** Attach the selected slider beside the existing spinbox only at recognized native boundaries. */
export class NativeNumericControls {
  readonly #cards = new Map<HTMLElement, Map<HTMLInputElement, NumericBinding>>();
  readonly #createSlider: (documentRef: Document, field: NumericSliderSpec) => NumericSliderControl;

  /** Bind the official component factory independently of native widget adaptation. */
  constructor(
    createSlider: (documentRef: Document, field: NumericSliderSpec) => NumericSliderControl,
  ) {
    this.#createSlider = createSlider;
  }

  /** Reconcile native inputs while keeping adapter-identified seeds out of slider presentation. */
  reconcile(
    root: HTMLElement,
    fields: readonly NativeNumericField[],
    seedInputs: ReadonlySet<HTMLInputElement>,
  ): void {
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
        (supportsSlider(replacement) && !seedInputs.has(input)) !== (binding.slider !== null)
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
      } else
        bindings.set(
          field.input,
          this.#bind(field, supportsSlider(field) && !seedInputs.has(field.input)),
        );
    }
  }

  /** Release controls, subscriptions, and annotations when the theme or renderer ends. */
  clear(): void {
    for (const bindings of this.#cards.values())
      for (const binding of bindings.values()) binding.release();
    this.#cards.clear();
  }

  /** Synchronize through native model events so all edits use Comfy's existing value path. */
  #bind(field: NativeNumericField, withSlider: boolean): NumericBinding {
    const slider = withSlider ? this.#createSlider(field.row.ownerDocument, field) : null;
    const annotations = new Map<string, HTMLElement>();
    const gestureTargets = slider ? [field.spinner, slider.root] : [field.spinner];
    for (const target of gestureTargets)
      target.addEventListener('pointerdown', retainNumericGesture);
    let synchronizing = false;
    let unsubscribe: () => void = () => undefined;
    const publishSliderValue = (): void => {
      if (synchronizing || !slider || binding.field.disabled()) return;
      const value = slider.readValue();
      if (Number.isFinite(value) && value !== binding.field.readValue())
        binding.field.writeValue(value);
    };
    const binding: NumericBinding = {
      field,
      slider,
      synchronize: () => {
        synchronizeNumericAnnotations(binding.field, annotations);
        if (!slider) return;
        const label = formatDisplayLabel(binding.field.label);
        slider.root.setAttribute('aria-label', `${binding.field.label} slider`);
        slider.root.dataset.scNumberLabel = label;
        synchronizing = true;
        try {
          slider.synchronize(
            binding.field.minimum,
            binding.field.maximum,
            binding.field.step,
            binding.field.disabled(),
            binding.field.readValue(),
          );
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
        slider?.root.removeEventListener('input', publishSliderValue);
        slider?.root.removeEventListener('change', publishSliderValue);
        slider?.root.removeEventListener('keydown', retainSliderNavigation);
        slider?.root.remove();
      },
    };
    if (slider) {
      slider.root.dataset.scNumberSlider = '';
      field.container.setAttribute('data-sc-number-pair', '');
      slider.root.addEventListener('input', publishSliderValue);
      slider.root.addEventListener('change', publishSliderValue);
      slider.root.addEventListener('keydown', retainSliderNavigation);
      field.container.append(slider.root);
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

/** Select presentation at the browser boundary while the native field retains its input and value. */
function supportsSlider(field: NativeNumericField): boolean {
  return (
    usesNumericSlider(field, readOwnedVisualStyle(field.row.ownerDocument)) &&
    !field.hasNativeSlider
  );
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

/** Keep primary gesture starts away from Comfy's node drag owner; themed retains window move/up. */
function retainNumericGesture(event: Event): void {
  if (event instanceof MouseEvent && event.button === 0) event.stopPropagation();
}

/** Preserve native range navigation before Comfy's graph shortcuts can cancel its default value change. */
function retainSliderNavigation(event: KeyboardEvent): void {
  if (
    [
      'ArrowLeft',
      'ArrowRight',
      'ArrowUp',
      'ArrowDown',
      'Home',
      'End',
      'PageUp',
      'PageDown',
    ].includes(event.key)
  )
    event.stopPropagation();
}
