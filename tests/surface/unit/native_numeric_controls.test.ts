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
/** Prove native numeric ownership, Substitute slider selection, and reversible presentation. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeNumericControls } from '../../../frontend/comfyui/ui/surface/appearance/NativeNumericControls.js';
import type { NumericSliderControl } from '../../../frontend/comfyui/ui/surface/appearance/NativeNumericControls.js';
import type { NativeNumericField } from '../../../frontend/comfyui/ui/surface/appearance/ComfyNativeNumericField.js';
import { readNativeNumericField } from '../../../frontend/comfyui/ui/surface/appearance/ComfyNativeNumericField.js';
import {
  usesGimpSpinScale,
  usesNumericSlider,
} from '../../../frontend/comfyui/ui/surface/appearance/NumericSliderPresentation.js';
import { ensureOwnedControlAppearanceStyles } from '../../../frontend/comfyui/ui/surface/appearance/OwnedControlAppearanceStyles.js';
import { nativeNumericCss } from '../../../frontend/comfyui/ui/surface/appearance/NativeNumericStyles.js';
import { createNumericSlider } from '../../../frontend/comfyui/ui/surface/appearance/ControlFactory.js';
import { publishVisualStyle } from '../../../frontend/comfyui/ui/surface/appearance/VisualStyleBoundary.js';
import { Slider } from '@fluentui/web-components/slider/class.js';
import { SPINNER_WIDTH } from '../../../frontend/comfyui/ui/surface/appearance/ControlGeometry.js';
import { nativeVisualStyleCss } from '../../../frontend/comfyui/ui/surface/appearance/ThemeService.js';
import '../../../frontend/comfyui/ui/surface/appearance/FluentThemeRuntime.js';

afterEach(() => {
  document.body.replaceChildren();
  document.documentElement.removeAttribute('data-sc-visual-style');
});

test.each(['fluent', 'spectrum', 'photopro'] as const)(
  '%s shares compact widths between scalar and slider spinners while preserving full seed boxes',
  (style) => {
    const css = nativeVisualStyleCss(style, true);
    expect(css).toContain(
      `[data-sc-number-spinner]:not([data-sc-seed-box]) { width:${SPINNER_WIDTH};`,
    );
    expect(css).toContain(
      `[data-sc-number-pair] > :first-child { width:${SPINNER_WIDTH}; min-width:${SPINNER_WIDTH};`,
    );
    expect(css).toContain(`flex-basis:${SPINNER_WIDTH};`);
    ensureOwnedControlAppearanceStyles(document);
    const seed = document.createElement('div');
    seed.dataset.scNumberSpinner = '';
    seed.dataset.scSeedBox = '';
    document.body.append(seed);
    expect(getComputedStyle(seed).width).toBe('192px');
  },
);

/** Model the official slider's synchronous change notification on value assignment. */
class SliderFixture extends HTMLElement implements NumericSliderControl {
  /** Expose the fixture element through the same boundary as official components. */
  get root(): HTMLElement {
    return this;
  }
  /** Normalize the simulated vendor's string values. */
  readValue(): number {
    return Number(this.value);
  }
  /** Reflect numeric state using the fixture's synchronous vendor change notification. */
  synchronize(
    minimum: number,
    maximum: number,
    step: number,
    disabled: boolean,
    value: number | null,
  ): void {
    this.min = String(minimum);
    this.max = String(maximum);
    this.step = String(step);
    this.disabled = disabled;
    if (value !== null) this.value = String(value);
  }
  min = '';
  max = '';
  step = '';
  disabled = false;
  #value = '';
  /** Read the current displayed slider value. */
  get value(): string {
    return this.#value;
  }
  /** Notify value changes exactly as Microsoft's slider does. */
  set value(value: string) {
    this.#value = value;
    this.dispatchEvent(new Event('change'));
  }
}
customElements.define('numeric-slider-fixture', SliderFixture);

test('completes official Fluent component registration before creating numeric controls', () => {
  const registered = customElements.get('fluent-slider');
  expect(registered).toBeDefined();
  expect(registered?.prototype).toBeInstanceOf(Slider);
  expect(typeof Reflect.get(registered?.prototype ?? {}, 'connectedMoveCallback')).toBe('function');
});

test('keeps primary numeric gestures with the controls and restores graph gestures on release', () => {
  const { root, field, controls } = fixture();
  const graphStart = jest.fn();
  root.addEventListener('pointerdown', graphStart);
  const nativeStart = jest.fn();
  field.input.addEventListener('pointerdown', nativeStart);
  controls.reconcile(root, [field], new Set());
  const slider = root.querySelector<SliderFixture>('numeric-slider-fixture');
  if (!slider) throw new Error('Slider fixture missing');
  for (const target of [field.input, field.increment, field.decrement, slider]) {
    if (!target) throw new Error('Native numeric control missing');
    const event = new MouseEvent('pointerdown', { bubbles: true, cancelable: true, button: 0 });
    target.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
  }
  expect(nativeStart).toHaveBeenCalledTimes(1);
  expect(graphStart).not.toHaveBeenCalled();
  const custom = document.createElement('div');
  root.append(custom);
  custom.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, button: 0 }));
  field.input.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, button: 1 }));
  expect(graphStart).toHaveBeenCalledTimes(2);
  controls.clear();
  field.input.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, button: 0 }));
  expect(graphStart).toHaveBeenCalledTimes(3);
});

test('allows slider drag updates and completion to reach its window listeners', () => {
  const { root, field, controls, write } = fixture();
  controls.reconcile(root, [field], new Set());
  const slider = root.querySelector<SliderFixture>('numeric-slider-fixture');
  if (!slider) throw new Error('Slider fixture missing');
  const move = jest.fn(() => {
    slider.value = '0.5';
  });
  const finish = jest.fn();
  window.addEventListener('pointermove', move);
  window.addEventListener('pointerup', finish);
  try {
    slider.dispatchEvent(new MouseEvent('pointermove', { bubbles: true, buttons: 1 }));
    slider.dispatchEvent(new MouseEvent('pointerup', { bubbles: true }));
    expect(move).toHaveBeenCalledTimes(1);
    expect(finish).toHaveBeenCalledTimes(1);
    expect(write).toHaveBeenCalledWith(0.5);
    expect(field.input.value).toBe('0.5');
  } finally {
    window.removeEventListener('pointermove', move);
    window.removeEventListener('pointerup', finish);
    controls.clear();
  }
});

test.each(['fluent', 'spectrum', 'photopro'] as const)(
  'writes each %s slider input while dragging and ignores the duplicate final change',
  (style) => {
    publishVisualStyle(document, style);
    const { root, field, write } = fixture();
    const controls = new NativeNumericControls(createNumericSlider);
    try {
      controls.reconcile(root, [field], new Set());
      const slider = root.querySelector<HTMLElement>('[data-sc-number-slider]');
      if (!slider) throw new Error('Numeric slider missing');
      Reflect.set(slider, 'value', style === 'spectrum' ? 0.4 : '0.4');
      slider.dispatchEvent(new Event('input', { bubbles: true }));
      expect(field.input.value).toBe('0.4');
      Reflect.set(slider, 'value', style === 'spectrum' ? 0.6 : '0.6');
      slider.dispatchEvent(new Event('input', { bubbles: true }));
      expect(field.input.value).toBe('0.6');
      slider.dispatchEvent(new Event('change', { bubbles: true }));
      expect(write).toHaveBeenCalledTimes(2);
      controls.clear();
      Reflect.set(slider, 'value', style === 'spectrum' ? 0.8 : '0.8');
      slider.dispatchEvent(new Event('input', { bubbles: true }));
      expect(write).toHaveBeenCalledTimes(2);
    } finally {
      controls.clear();
    }
  },
);

/** Keep integrated GIMP numeric presentation on the characterized native binding. */
test('GIMP presents bounded numeric fields through the existing native input and value owner', () => {
  publishVisualStyle(document, 'gimp');
  const { root, field, controls, write, external } = fixture('cfg', 'CFG', 2);
  const input = field.input;
  controls.reconcile(root, [field], new Set());
  const slider = root.querySelector<SliderFixture>('[data-sc-number-slider]');
  expect(slider).not.toBeNull();
  expect(slider?.dataset.scNumberLabel).toBe('CFG');
  expect(root.querySelector('input')).toBe(input);
  external(0.7);
  expect(slider?.readValue()).toBe(0.7);
  expect(write).not.toHaveBeenCalled();
  if (!slider) throw new Error('The GIMP numeric presentation did not mount.');
  slider.value = '0.4';
  slider.dispatchEvent(new Event('input', { bubbles: true }));
  expect(input.value).toBe('0.4');
  expect(write).toHaveBeenCalledTimes(1);
  controls.clear();
  expect(root.querySelector('input')).toBe(input);
});

test('GIMP excludes seed inputs identified by the host adapter even with a nonstandard name', () => {
  publishVisualStyle(document, 'gimp');
  const { root, field, controls } = fixture('entropy', 'Random state', 0);
  controls.reconcile(root, [field], new Set([field.input]));
  expect(root.querySelector('[data-sc-number-slider]')).toBeNull();
  expect(field.input.hasAttribute('data-sc-number-input')).toBe(true);
  controls.clear();
});

test.each(['width', 'height'])(
  'GIMP retains an ordinary %s entry with its native step buttons',
  (name) => {
    publishVisualStyle(document, 'gimp');
    const { root, field, controls } = fixture(name, name, 0);
    controls.reconcile(root, [field], new Set());
    expect(root.querySelector('[data-sc-number-slider]')).toBeNull();
    expect(field.increment?.hasAttribute('data-sc-number-increment')).toBe(true);
    expect(field.decrement?.hasAttribute('data-sc-number-decrement')).toBe(true);
    expect(root.querySelector('input')).toBe(field.input);
    controls.clear();
  },
);

/** Build one native numeric field without replacing its editor or value callback. */
function fixture(name = 'denoise', label = name, precision = 2) {
  const root = document.createElement('article');
  const row = document.createElement('div');
  const container = document.createElement('div');
  const spinner = document.createElement('div');
  const input = document.createElement('input');
  const increment = document.createElement('button');
  const decrement = document.createElement('button');
  spinner.append(decrement, input, increment);
  container.append(spinner);
  row.append(container);
  root.append(row);
  document.body.append(root);
  let value = 0.2;
  let listener: (() => void) | null = null;
  const unsubscribe = jest.fn(() => {
    listener = null;
  });
  const write = jest.fn((next: number) => {
    value = next;
    input.value = String(next);
    listener?.();
  });
  const field: NativeNumericField = {
    name,
    label,
    precision,
    minimum: 0,
    maximum: 1,
    step: 0.01,
    row,
    container,
    spinner,
    input,
    increment,
    decrement,
    hasNativeSlider: false,
    rangeFill: null,
    readValue: () => value,
    disabled: () => input.disabled || input.readOnly,
    writeValue: write,
    subscribe: (next) => {
      listener = next;
      return unsubscribe;
    },
  };
  const controls = new NativeNumericControls(() => new SliderFixture());
  return {
    root,
    field,
    controls,
    write,
    unsubscribe,
    external: (next: number) => {
      value = next;
      listener?.();
    },
  };
}

test.each([
  ['denoise', 'denoise', 2, true],
  ['variation_strength', 'variation_strength', 2, true],
  ['value', 'Scale_Factor', 1, true],
  ['value', ' scale-factor ', 1, true],
  ['cfg', 'cfg', 1, false],
  ['seed', 'seed', 0, false],
  ['denoise', 'denoise', 0, false],
])('matches Substitute slider policy for %s / %s', (name, label, precision, expected) => {
  expect(
    usesNumericSlider({ name, label, precision, minimum: 0, maximum: 1, step: 0.01 }, 'fluent'),
  ).toBe(expected);
});

test('rejects invalid, reversed, and unsafe slider ranges', () => {
  const spec = {
    name: 'denoise',
    label: 'denoise',
    precision: 2,
    minimum: 0,
    maximum: 1,
    step: 0.01,
  };
  for (const change of [{ step: 0 }, { maximum: Infinity }, { minimum: 2 }, { maximum: 1e20 }])
    expect(usesNumericSlider({ ...spec, ...change }, 'fluent')).toBe(false);
});

test.each<[string, string, boolean]>([
  ['denoise', 'Denoise', false],
  ['variation_strength', 'Variation strength', false],
  ['value', 'Scale Factor', false],
  ['opacity', 'Opacity', true],
  ['cfg', 'CFG', true],
  ['width', 'Width', false],
  ['height', 'Height', false],
])('selects the intended GIMP numeric variant for %s', (name, label, expected) => {
  expect(usesGimpSpinScale({ name, label, precision: 2, minimum: 0, maximum: 1, step: 0.01 })).toBe(
    expected,
  );
});

test('keeps slider, typed entry, buttons, and external updates synchronized through one native owner', () => {
  const { root, field, controls, write, external, unsubscribe } = fixture();
  controls.reconcile(root, [field], new Set());
  const slider = root.querySelector<SliderFixture>('numeric-slider-fixture');
  if (!slider) throw new Error('Slider fixture missing');
  expect(slider.value).toBe('0.2');
  expect(write).not.toHaveBeenCalled();
  slider.value = '0.4';
  expect(write).toHaveBeenCalledTimes(1);
  expect(field.input.value).toBe('0.4');
  field.writeValue(0.37);
  expect(slider.value).toBe('0.37');
  write.mockClear();
  external(0.63);
  expect(slider.value).toBe('0.63');
  expect(write).not.toHaveBeenCalled();
  controls.reconcile(root, [field], new Set());
  expect(root.querySelectorAll('numeric-slider-fixture')).toHaveLength(1);
  field.input.disabled = true;
  controls.reconcile(root, [field], new Set());
  expect(slider.disabled).toBe(true);
  slider.value = '0.9';
  expect(write).not.toHaveBeenCalled();
  controls.clear();
  expect(unsubscribe).toHaveBeenCalledTimes(1);
  expect(slider.isConnected).toBe(false);
  expect(field.spinner.hasAttribute('data-sc-number-spinner')).toBe(false);
  expect(field.input.isConnected).toBe(true);
  slider.value = '0.1';
  expect(write).not.toHaveBeenCalled();
});

test('stacks integer controls without adding sliders and releases stale controls on native replacement', () => {
  const { root, field, controls } = fixture('steps', 'steps', 0);
  controls.reconcile(root, [field], new Set());
  expect(field.increment?.hasAttribute('data-sc-number-increment')).toBe(true);
  expect(field.decrement?.hasAttribute('data-sc-number-decrement')).toBe(true);
  expect(root.querySelector('numeric-slider-fixture')).toBeNull();
  controls.reconcile(root, [], new Set());
  expect(field.input.hasAttribute('data-sc-number-input')).toBe(false);
  expect(nativeNumericCss()).toContain('[data-sc-number-increment] { top:0; }');
  expect(nativeNumericCss()).toContain('[data-sc-number-decrement] { bottom:0; }');
  controls.clear();
});

test('centers recognized numeric editors and uses the desktop slider cursor only while themed', () => {
  const style = document.createElement('style');
  style.textContent = nativeNumericCss();
  const { root, field, controls } = fixture();
  root.append(style);
  const custom = document.createElement('input');
  custom.style.textAlign = 'right';
  root.append(custom);
  const originalAlignment = getComputedStyle(field.input).textAlign;
  controls.reconcile(root, [field], new Set());
  const slider = root.querySelector<SliderFixture>('numeric-slider-fixture');
  if (!slider) throw new Error('Slider fixture missing');
  expect(getComputedStyle(field.input).textAlign).toBe('center');
  expect(getComputedStyle(field.input).paddingLeft).toBe('8px');
  expect(getComputedStyle(field.input).paddingRight).toBe('8px');
  expect(getComputedStyle(slider).cursor).toBe('default');
  expect(nativeNumericCss()).toContain('cursor:default !important');
  expect(nativeNumericCss()).toContain('[data-sc-number-slider]::part(track-container)');
  expect(getComputedStyle(custom).textAlign).toBe('right');
  controls.clear();
  expect(getComputedStyle(field.input).textAlign).toBe(originalAlignment);
});

test('releases presentation when a native value subscription cannot be established', () => {
  const { root, field, controls } = fixture();
  field.subscribe = () => {
    throw new Error('Native watcher unavailable');
  };
  expect(() => controls.reconcile(root, [field], new Set())).toThrow('Native watcher unavailable');
  expect(root.querySelector('numeric-slider-fixture')).toBeNull();
  expect(field.input.hasAttribute('data-sc-number-input')).toBe(false);
  expect(field.container.hasAttribute('data-sc-number-pair')).toBe(false);
  expect(field.input.isConnected).toBe(true);
  controls.clear();
});

test('renders markup-like labels literally and retains native sliders without duplication', () => {
  const { root, field, controls } = fixture('denoise', '<img src=x>');
  controls.reconcile(root, [field], new Set());
  expect(root.querySelector('img')).toBeNull();
  expect(root.querySelector('numeric-slider-fixture')?.getAttribute('aria-label')).toBe(
    '<img src=x> slider',
  );
  field.hasNativeSlider = true;
  controls.reconcile(root, [field], new Set());
  expect(root.querySelector('numeric-slider-fixture')).toBeNull();
  controls.clear();
});

test('adapts the live numeric component model and watcher while rejecting unknown component markup', () => {
  const { field } = fixture();
  const layout = document.createElement('div');
  layout.dataset.scStyleLayout = '';
  layout.append(document.createElement('span'), field.container);
  field.row.append(layout);
  field.input.setAttribute('role', 'spinbutton');
  field.increment?.setAttribute('data-testid', 'increment');
  field.decrement?.setAttribute('data-testid', 'decrement');
  const rangeFill = document.createElement('div');
  rangeFill.className = 'pointer-events-none absolute size-full';
  field.spinner.prepend(rangeFill);
  const emit = jest.fn();
  const stop = jest.fn();
  const watch = jest.fn(() => stop);
  const numeric = {
    type: { __name: 'ScrubableNumberInput' },
    component: {
      props: { modelValue: 0.2, step: 0.01 },
      proxy: { $watch: watch },
      emit,
      subTree: { el: field.spinner },
    },
  };
  const widget = {
    component: {
      props: {
        widget: {
          name: 'value',
          label: 'Scale Factor',
          options: { min: 1, max: 5, step2: 0.1, precision: 1 },
        },
      },
      subTree: { children: [numeric] },
    },
  };
  const adapted = readNativeNumericField(field.row, widget);
  if (!adapted) throw new Error('Native boundary missing');
  expect(adapted.label).toBe('Scale Factor');
  expect(adapted.maximum).toBe(5);
  expect(adapted.rangeFill).toBe(rangeFill);
  adapted.writeValue(0.4);
  expect(emit).toHaveBeenCalledWith('update:modelValue', 0.4);
  const unsubscribe = adapted.subscribe(() => undefined);
  expect(watch).toHaveBeenCalledTimes(1);
  unsubscribe();
  expect(stop).toHaveBeenCalledTimes(1);
  numeric.type.__name = 'CustomNumberInput';
  expect(readNativeNumericField(field.row, widget)).toBeNull();
});

test('styles late-mounted and replaced native buttons without replacing the input or slider', () => {
  const { root, field, controls } = fixture();
  const input = field.input;
  controls.reconcile(root, [{ ...field, increment: null, decrement: null }], new Set());
  const slider = root.querySelector('numeric-slider-fixture');
  controls.reconcile(root, [field], new Set());
  expect(field.increment?.hasAttribute('data-sc-number-increment')).toBe(true);
  expect(field.decrement?.hasAttribute('data-sc-number-decrement')).toBe(true);
  const increment = document.createElement('button');
  field.spinner.append(increment);
  controls.reconcile(root, [{ ...field, increment }], new Set());
  expect(field.increment?.hasAttribute('data-sc-number-increment')).toBe(false);
  expect(increment.hasAttribute('data-sc-number-increment')).toBe(true);
  expect(root.querySelector('input')).toBe(input);
  expect(root.querySelector('numeric-slider-fixture')).toBe(slider);
  controls.clear();
  expect(increment.hasAttribute('data-sc-number-increment')).toBe(false);
  expect(input.hasAttribute('data-sc-number-input')).toBe(false);
});

test('suppresses only the recognized native range fill while Fluent presentation owns the field', () => {
  const { root, field, controls } = fixture();
  const style = document.createElement('style');
  style.textContent = nativeNumericCss();
  root.append(style);
  const fill = document.createElement('div');
  const custom = document.createElement('div');
  field.spinner.prepend(fill, custom);
  controls.reconcile(root, [{ ...field, rangeFill: fill }], new Set());
  expect(getComputedStyle(fill).display).toBe('none');
  expect(custom.hasAttribute('data-sc-native-number-fill')).toBe(false);
  controls.clear();
  expect(fill.hasAttribute('data-sc-native-number-fill')).toBe(false);
  expect(getComputedStyle(fill).display).toBe('block');
});
