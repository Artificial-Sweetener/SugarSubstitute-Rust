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
/** Prove owned surfaces show the node cards' number control, not a slider of their own. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NumericSpinnerSlider } from '../../../frontend/comfyui/ui/surface/appearance/NumericSpinnerSlider.js';
import { StyledNumericSlider } from '../../../frontend/comfyui/ui/surface/appearance/OwnedNumericSlider.js';
import { renderSettingsRow } from '../../../frontend/comfyui/ui/settings/workspace/SettingsCards.js';
import { SettingsState } from '../../../frontend/comfyui/ui/settings/workspace/SettingsModel.js';

const SPEC = {
  name: 'quality',
  label: 'Quality',
  precision: null,
  minimum: 1,
  maximum: 100,
  step: 1,
};

afterEach(() => {
  jest.useRealTimers();
  document.documentElement.removeAttribute('data-sc-visual-style');
  document.body.replaceChildren();
});

function parts(pair: NumericSpinnerSlider) {
  const root = pair.root;
  return {
    input: root.querySelector<HTMLInputElement>('[data-sc-number-spinner] [data-sc-number-input]')!,
    increment: root.querySelector<HTMLButtonElement>('[data-sc-number-increment]')!,
    decrement: root.querySelector<HTMLButtonElement>('[data-sc-number-decrement]')!,
    slider: () => root.querySelector<HTMLInputElement>('[data-sc-number-slider]'),
  };
}

test('carries the node cards markers so each visual style draws it as on the node cards', () => {
  document.documentElement.setAttribute('data-sc-visual-style', 'photopro');
  const pair = new NumericSpinnerSlider(document, SPEC, true, jest.fn());
  pair.show(90);
  const { input, slider } = parts(pair);
  expect(pair.root.hasAttribute('data-sc-number-pair')).toBe(true);
  expect(pair.root.hasAttribute('data-sc-style-theme-root')).toBe(true);
  expect(pair.root.hasAttribute('data-sc-style-surface')).toBe(true);
  expect(input.value).toBe('90');
  expect(slider()?.getAttribute('aria-label')).toBe('Quality slider');
  // PhotoPro's own node-card slider, not ComfyUI's.
  expect(slider()?.classList.contains('sc-comfy-numeric-slider')).toBe(false);
  expect(slider()?.value).toBe('90');
  pair.dispose();
});

test('a change of visual style replaces the slider with the new style', () => {
  const pair = new NumericSpinnerSlider(document, SPEC, true, jest.fn());
  pair.show(40);
  expect(parts(pair).slider()?.classList.contains('sc-comfy-numeric-slider')).toBe(true);
  document.documentElement.setAttribute('data-sc-visual-style', 'photopro');
  document.dispatchEvent(new Event('sugarcubes:visual-style-change'));
  expect(pair.root.querySelectorAll('[data-sc-number-slider]')).toHaveLength(1);
  expect(parts(pair).slider()?.classList.contains('sc-comfy-numeric-slider')).toBe(false);
  expect(parts(pair).slider()?.value).toBe('40');
  pair.dispose();
});

test('typed values commit clamped to the step grid; Escape abandons; steppers step', () => {
  const change = jest.fn<(value: number) => void>();
  const pair = new NumericSpinnerSlider(document, { ...SPEC, step: 5 }, false, change);
  document.body.append(pair.root);
  pair.show(50);
  const { input, increment, decrement, slider } = parts(pair);
  expect(slider()).toBeNull();
  input.focus();
  input.value = '73';
  input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
  expect(input.value).toBe('50');
  input.focus();
  input.value = '73';
  input.dispatchEvent(new Event('change'));
  expect(change).toHaveBeenLastCalledWith(71);
  input.value = '500';
  input.dispatchEvent(new Event('change'));
  expect(change).toHaveBeenLastCalledWith(100);
  pair.show(100);
  increment.click();
  expect(change).toHaveBeenLastCalledWith(100);
  decrement.click();
  expect(change).toHaveBeenLastCalledWith(96);
  pair.show(96, true);
  expect(input.disabled).toBe(true);
  expect(increment.disabled && decrement.disabled).toBe(true);
  pair.dispose();
});

test('a settled drag is one commit; a live drag commits every value', () => {
  jest.useFakeTimers();
  const settled = jest.fn<(value: number) => void>();
  const pair = new NumericSpinnerSlider(document, SPEC, true, settled);
  pair.show(10);
  const slider = parts(pair).slider()!;
  for (const value of ['20', '30', '40']) {
    slider.value = value;
    slider.dispatchEvent(new Event('input'));
  }
  expect(parts(pair).input.value).toBe('40');
  expect(settled).not.toHaveBeenCalled();
  jest.advanceTimersByTime(200);
  expect(settled.mock.calls).toEqual([[40]]);
  const live = jest.fn<(value: number) => void>();
  const following = new NumericSpinnerSlider(document, SPEC, true, live, 'live');
  following.show(10);
  const liveSlider = parts(following).slider()!;
  for (const value of ['20', '30']) {
    liveSlider.value = value;
    liveSlider.dispatchEvent(new Event('input'));
  }
  expect(live.mock.calls).toEqual([[20], [30]]);
  pair.dispose();
  following.dispose();
});

test('settings slider rows draw the node cards pair and follow the row enabled state', () => {
  const value = new SettingsState(90);
  const enabled = new SettingsState(true);
  const view = renderSettingsRow(
    document,
    {
      id: 'generation.output.jpeg.quality',
      title: 'JPEG quality',
      controls: [{ kind: 'slider', value, min: 1, max: 100, step: 1 }],
      enabled,
    },
    jest.fn(),
  );
  document.body.append(view.element);
  const pair = view.element.querySelector<HTMLElement>('[data-sc-number-pair]');
  expect(pair).not.toBeNull();
  expect(
    view.element.querySelectorAll('input[type="range"]:not([data-sc-number-slider])'),
  ).toHaveLength(0);
  const input = pair!.querySelector<HTMLInputElement>('[data-sc-number-input]')!;
  expect(input.value).toBe('90');
  enabled.set(false);
  expect(input.disabled).toBe(true);
  expect(pair!.querySelector<HTMLInputElement>('[data-sc-number-slider]')!.disabled).toBe(true);
  enabled.set(true);
  expect(input.disabled).toBe(false);
  view.dispose();
});

test('a standalone styled slider keeps its place, value and attributes across a style change', () => {
  const input = jest.fn<(value: number) => void>();
  const slider = new StyledNumericSlider(
    document,
    { ...SPEC, attributes: { 'data-canvas-option': 'size' } },
    input,
  );
  const host = document.createElement('span');
  host.append(slider.root);
  slider.synchronize(1, 100, 1, false, 40);
  const first = slider.root;
  document.documentElement.setAttribute('data-sc-visual-style', 'photopro');
  document.dispatchEvent(new Event('sugarcubes:visual-style-change'));
  expect(slider.root).not.toBe(first);
  expect(host.firstElementChild).toBe(slider.root);
  expect(slider.root.getAttribute('data-canvas-option')).toBe('size');
  expect(slider.root.hasAttribute('data-sc-style-theme-root')).toBe(true);
  expect((slider.root as HTMLInputElement).value).toBe('40');
  (slider.root as HTMLInputElement).value = '55';
  slider.root.dispatchEvent(new Event('input'));
  expect(input).toHaveBeenLastCalledWith(55);
  slider.dispose();
});
