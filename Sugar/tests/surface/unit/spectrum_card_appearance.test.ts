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
/** Safeguard Spectrum card colors and rounding while preserving masonry geometry and Fluent styling. */
import { afterEach, expect, test } from '@jest/globals';
import { spectrumAppearanceCss } from '../../../frontend/comfyui/ui/surface/appearance/SpectrumAppearanceStyles.js';
import { fluentAppearanceCss } from '../../../frontend/comfyui/ui/surface/appearance/FluentAppearanceStyles.js';

afterEach(() => {
  document.head.replaceChildren();
  document.body.replaceChildren();
});

/** Mount native presentation boundaries without replacing field or layout owners. */
function cardFixture(css: string) {
  const style = document.createElement('style');
  style.textContent = css;
  document.head.append(style);
  const surface = document.createElement('div');
  surface.className = 'sugarcubes-workflow-masonry';
  surface.dataset.scStyleSurface = '';
  const outer = document.createElement('div');
  outer.className = 'sugarcubes-cube-face__node-card';
  const card = document.createElement('div');
  card.dataset.scStyleNode = '';
  const wrapper = document.createElement('div');
  wrapper.dataset.testid = 'node-inner-wrapper';
  const body = document.createElement('div');
  body.dataset.testid = 'node-body-test';
  const fields = document.createElement('div');
  fields.dataset.scStyleFields = '';
  const row = document.createElement('div');
  row.dataset.scStyleRow = '';
  row.dataset.scStyleColumn = '2';
  const input = document.createElement('input');
  input.value = '<literal retained value>';
  row.append(input);
  fields.append(row);
  body.append(fields);
  wrapper.append(body);
  card.append(wrapper);
  outer.append(card);
  surface.append(outer);
  document.body.append(surface);
  return { card, outer, fields, row, input, style };
}

test.each([true, false])(
  'uses Photoshop panel colors and proportional card corners, dark=%s',
  (dark) => {
    const css = spectrumAppearanceCss(dark);
    const { card, outer } = cardFixture(css);
    const palette = getComputedStyle(card);
    expect(palette.getPropertyValue('--sc-card-fill')).toBe('var(--sc-photoshop-panel)');
    expect(palette.getPropertyValue('--sc-card-stroke')).toBe('var(--system-card-border-color)');
    expect(palette.getPropertyValue('--sc-node-card-radius')).toBe(
      'var(--spectrum-corner-radius-large-default)',
    );
    expect(palette.getPropertyValue('--sc-row-stroke')).toBe('transparent');
    expect(css).toContain(
      `--sc-photoshop-panel:var(--uxp-host-background-color,${dark ? '#323232' : '#f0f0f0'})`,
    );
    expect(css).toContain(`--sc-photoshop-field:${dark ? '#2a2a2a' : '#fcfcfc'}`);
    expect(css).toContain('--system-textfield-background-color:var(--sc-photoshop-field)');
    expect(getComputedStyle(outer).borderRadius).toBe('var(--sc-node-card-radius)');
    expect(css).toContain(
      `--spectrum-background-layer-2-color:var(--spectrum-gray-${dark ? '75' : '25'})`,
    );
  },
);

test('retains masonry row geometry, field identity and values across the two styled providers', () => {
  const mounted = cardFixture(fluentAppearanceCss(true));
  const geometry = (): readonly string[] => {
    const row = getComputedStyle(mounted.row);
    const fields = getComputedStyle(mounted.fields);
    return [
      row.display,
      row.paddingTop,
      row.paddingBottom,
      row.paddingLeft,
      row.paddingRight,
      row.gridTemplateColumns,
      fields.gridTemplateColumns,
      fields.gap,
    ];
  };
  const fluent = geometry();
  expect(getComputedStyle(mounted.card).getPropertyValue('--sc-row-stroke')).toBe(
    'var(--sc-card-stroke)',
  );
  mounted.input.focus();
  mounted.input.setSelectionRange(2, 7);
  mounted.style.textContent = spectrumAppearanceCss(true);
  expect(geometry()).toEqual(fluent);
  expect(mounted.row.querySelector('input')).toBe(mounted.input);
  expect(document.activeElement).toBe(mounted.input);
  expect(mounted.input.selectionStart).toBe(2);
  expect(mounted.input.selectionEnd).toBe(7);
  expect(mounted.input.value).toBe('<literal retained value>');
  expect(mounted.row.querySelector('literal')).toBeNull();
  mounted.style.textContent = fluentAppearanceCss(true);
  expect(geometry()).toEqual(fluent);
  expect(getComputedStyle(mounted.card).getPropertyValue('--sc-row-stroke')).toBe(
    'var(--sc-card-stroke)',
  );
});
