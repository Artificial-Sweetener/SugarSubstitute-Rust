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
/** Prove seed split-button interaction and reversible native numeric entry. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeSeedControls } from '../../../frontend/comfyui/ui/surface/appearance/NativeSeedControls.js';
import type { NativeSeedField } from '../../../frontend/comfyui/ui/surface/appearance/NativeSeedControls.js';
import type { SeedMode } from '../../../frontend/comfyui/ui/graph/ComfySeedMode.js';
import { fluentSeedCss } from '../../../frontend/comfyui/ui/surface/appearance/FluentSeedStyles.js';

afterEach(() => document.body.replaceChildren());

/** Keep the numeric editor and a host-owned mode observable for interaction assertions. */
function fixture() {
  const root = document.createElement('article');
  const spinner = document.createElement('div');
  const input = document.createElement('input');
  input.value = '123456789';
  spinner.append(input);
  root.append(spinner);
  document.body.append(root);
  let mode: SeedMode = 'randomize';
  const write = jest.fn((value: SeedMode) => {
    mode = value;
  });
  const unsubscribe = jest.fn();
  let notify = (): void => undefined;
  const field: NativeSeedField = {
    numeric: {
      name: 'seed',
      label: '<b>seed</b>',
      precision: 0,
      minimum: 0,
      maximum: 1e15,
      step: 1,
      row: root,
      container: root,
      spinner,
      input,
      increment: null,
      decrement: null,
      hasNativeSlider: false,
      readValue: () => Number(input.value),
      writeValue: (value) => {
        input.value = String(value);
      },
      disabled: () => input.disabled || input.readOnly,
      subscribe: (listener) => {
        notify = listener;
        return unsubscribe;
      },
    },
    control: {
      modes: ['fixed', 'increment', 'decrement', 'randomize'],
      readMode: () => mode,
      writeMode: write,
    },
  };
  const controls = new NativeSeedControls();
  controls.reconcile(root, [field]);
  const primary = root.querySelector<HTMLButtonElement>('[data-sc-seed-toggle]');
  const arrow = root.querySelector<HTMLButtonElement>('[data-sc-seed-menu]');
  if (!primary || !arrow) throw new Error('Seed split button missing');
  return { root, field, controls, primary, arrow, write, unsubscribe, notify: () => notify() };
}

test('quick toggles fixed/randomize without changing the current numeric value', () => {
  const f = fixture();
  f.primary.click();
  expect(f.field.control.readMode()).toBe('fixed');
  f.primary.click();
  expect(f.field.control.readMode()).toBe('randomize');
  f.field.control.writeMode('increment');
  f.controls.reconcile(f.root, [f.field]);
  f.primary.click();
  expect(f.field.control.readMode()).toBe('randomize');
  expect(f.field.numeric.input.value).toBe('123456789');
  expect(f.primary.querySelector('span')?.style.getPropertyValue('--sc-seed-icon')).toContain(
    'game-die.svg',
  );
  expect(f.root.querySelector('b')).toBeNull();
  f.controls.clear();
});

test('dropdown offers the host modes, tracks selection and supports keyboard/dismissal', () => {
  const f = fixture();
  f.arrow.click();
  expect(f.arrow.getAttribute('aria-expanded')).toBe('true');
  expect(document.querySelectorAll('[role="menuitemradio"]')).toHaveLength(4);
  for (const [mode, filename] of Object.entries({
    fixed: 'locked',
    increment: 'add',
    decrement: 'subtract',
    randomize: 'game-die',
  })) {
    const icon = document.querySelector(`[data-sc-seed-option="${mode}"] [data-sc-seed-icon]`);
    expect(icon?.getAttribute('aria-hidden')).toBe('true');
    expect(icon?.getAttribute('style')).toContain(`${filename}.svg`);
  }
  let increment = document.querySelector<HTMLButtonElement>('[data-sc-seed-option="increment"]');
  increment?.click();
  expect(f.field.control.readMode()).toBe('increment');
  expect(f.arrow.getAttribute('aria-expanded')).toBe('false');
  f.arrow.click();
  increment = document.querySelector<HTMLButtonElement>('[data-sc-seed-option="increment"]');
  expect(document.activeElement).toBe(increment);
  increment?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
  expect(document.activeElement?.getAttribute('data-sc-seed-option')).toBe('decrement');
  document.activeElement?.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }),
  );
  expect(f.arrow.getAttribute('aria-expanded')).toBe('false');
  f.arrow.click();
  document.body.dispatchEvent(new Event('pointerdown', { bubbles: true }));
  expect(f.arrow.getAttribute('aria-expanded')).toBe('false');
  f.controls.clear();
});

test('follows disabled input and removes accessories, subscriptions and markers on replacement', () => {
  const f = fixture();
  f.controls.reconcile(f.root, [f.field]);
  expect(f.root.querySelectorAll('[data-sc-seed-split]')).toHaveLength(1);
  f.field.numeric.input.disabled = true;
  f.notify();
  expect(f.primary.disabled).toBe(true);
  expect(f.arrow.disabled).toBe(true);
  f.primary.click();
  expect(f.write).not.toHaveBeenCalled();
  f.controls.reconcile(f.root, []);
  expect(f.unsubscribe).toHaveBeenCalledTimes(1);
  expect(f.root.querySelector('[data-sc-seed-split]')).toBeNull();
  expect(f.field.numeric.spinner.hasAttribute('data-sc-seed-box')).toBe(false);
  expect(f.field.numeric.input.value).toBe('123456789');
  expect(fluentSeedCss()).toContain('padding-right:46px');
  expect(fluentSeedCss()).toContain('flex:0 1 192px');
  expect(fluentSeedCss()).toContain('min-width:60px');
  expect(fluentSeedCss()).toContain('font-variant-numeric:tabular-nums');
  f.controls.clear();
});

test('rolls back accessories when the native subscription fails', () => {
  const f = fixture();
  f.controls.clear();
  f.field.numeric.subscribe = () => {
    throw new Error('watch unavailable');
  };
  expect(() => f.controls.reconcile(f.root, [f.field])).toThrow('watch unavailable');
  expect(f.root.querySelector('[data-sc-seed-split]')).toBeNull();
  expect(f.field.numeric.spinner.hasAttribute('data-sc-seed-box')).toBe(false);
});
