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
/** Prove PhotoPro retains SugarCubes layout while using OpenPhoto paint and native gestures. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { publishVisualStyle } from '../../../frontend/comfyui/ui/surface/appearance/VisualStyleBoundary.js';
import {
  applyOwnedTheme,
  nativeVisualStyleCss,
} from '../../../frontend/comfyui/ui/surface/appearance/ThemeService.js';
import {
  createBooleanCheckbox,
  createBooleanSegments,
  createFinderInput,
  createNumericSlider,
  createWorkflowTablist,
} from '../../../frontend/comfyui/ui/surface/appearance/ControlFactory.js';
import {
  photoProPaletteCss,
  photoProTokenDeclarations,
} from '../../../frontend/comfyui/ui/surface/appearance/PhotoProThemeTokens.js';
import { photoProMenuCss } from '../../../frontend/comfyui/ui/surface/appearance/PhotoProMenuStyles.js';
import {
  PHOTOPRO_PALETTE_SETTING,
  registerPhotoProPalettePreference,
} from '../../../frontend/comfyui/ui/surface/appearance/PhotoProPalettePreference.js';
import { isRecord, type UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';
import { SPINNER_WIDTH } from '../../../frontend/comfyui/ui/surface/appearance/ControlGeometry.js';

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
  document.documentElement.removeAttribute('data-sc-visual-style');
  document.documentElement.removeAttribute('data-sc-photopro-palette');
});

test('maps OpenPhoto medium-gray paint onto the shared native card and masonry geometry', () => {
  const css = nativeVisualStyleCss('photopro', false);
  expect(css).toContain('--sc-photopro-pasteboard:#282828');
  expect(css).toContain('--sc-surface:#535353');
  expect(css).toContain('--sc-field-fill:#454545');
  expect(css).toContain('--sc-node-card-radius:4px');
  expect(css).toContain('--sugarcubes-masonry-column-width:400');
  expect(css).toContain('[data-sc-style-row] { padding-top:8px; padding-bottom:8px; }');
  expect(nativeVisualStyleCss('photopro', true)).toContain('--sc-surface:#535353');
});

test('places the shared-height spinner after the classic Photoshop track and pin', () => {
  const css = nativeVisualStyleCss('photopro', false);
  expect(css).toContain(`[data-sc-number-pair] { flex:0 0 calc(${SPINNER_WIDTH} + 100px);`);
  expect(css).toContain('[data-sc-number-pair] > :first-child { order:1; align-self:center; }');
  expect(css).not.toContain('[data-sc-number-pair] [data-sc-number-spinner] {');
  expect(css).toContain(
    ':is([data-sc-number-increment],[data-sc-number-decrement]) { display:none; }',
  );
  expect(css).toContain('input[data-sc-number-slider]::-webkit-slider-runnable-track');
  expect(css).toContain('input[data-sc-number-slider]::-moz-range-track');
  expect(css).toContain('input[data-sc-number-slider]::-moz-range-progress');
  expect(css).toContain('--sc-photopro-range-thumb-width:11px');
  expect(css).toContain('width:var(--sc-photopro-range-thumb-width); height:10px; margin-top:2px');
  expect(css).toContain('clip-path:polygon(50% 0, 100% 62%');
  expect(css).toContain('top:-4px; order:0; height:23px');
  expect(css).toContain('height:23px; margin:0 0 0 10px');
  expect(css).toContain('margin-top:2px');
  expect(css).toContain('transform:translateY(5px)');
  expect(css).toContain('cursor:default; flex:1 1 auto');
  expect(css).not.toContain('height:14px !important');
  expect(css).not.toContain('border-radius:50%; background:var(--sc-surface)');
  expect(css).not.toContain('accent-color:var(--sc-brand-fill)');
});

test('insets only the painted track to meet the thumb point at both native travel limits', () => {
  const css = nativeVisualStyleCss('photopro', false);
  const paint =
    'background:linear-gradient(var(--sc-photopro-range-track),var(--sc-photopro-range-track)) center / calc(100% - var(--sc-photopro-range-thumb-width)) 100% no-repeat;';
  for (const track of ['::-webkit-slider-runnable-track', '::-moz-range-track']) {
    const rule = css.slice(css.indexOf(`input[data-sc-number-slider]${track} {`)).split('}')[0];
    expect(rule).toContain(paint);
    expect(rule).not.toMatch(/(?:padding|margin|width):/);
  }
  const progress = css
    .slice(css.indexOf('input[data-sc-number-slider]::-moz-range-progress {'))
    .split('}')[0];
  expect(progress).toContain('background:transparent');
});

test('paints native comboboxes as one compact field while retaining model picker decoration', () => {
  const css = nativeVisualStyleCss('photopro', false);
  expect(css).toContain('[data-sc-style-combo-frame] {');
  expect(css).toContain('align-self:center; border-radius:3px');
  expect(css).toContain('--sc-control-height:22px');
  expect(css).toContain('[data-sc-number-spinner] { padding-right:0; }');
  expect(css).toContain(
    'height:var(--sc-control-height,33px); min-height:var(--sc-control-height,33px)',
  );
  expect(css).toContain(
    '[data-sc-number-spinner] {\n      position:relative; display:flex; box-sizing:border-box; padding-right:20px;\n      height:var(--sc-control-height,33px)',
  );
  expect(css).toContain(
    '[data-sc-style-combo-frame]:not(.sc-model-field) > button[aria-hidden="true"]::after',
  );
  expect(css).toContain('[data-sc-style-combo-frame]:not(.sc-model-field) > button {');
  expect(css).toContain('height:100%; border:0; border-radius:0; background:transparent');
  expect(css).not.toContain(
    '[data-sc-style-popup][role="listbox"] [role="option"][data-state="checked"]',
  );
});

test('centers the PhotoPro check mark inside its checkbox', () => {
  const css = nativeVisualStyleCss('photopro', false);
  expect(css).toContain(
    '[data-sc-boolean-checkbox] { appearance:none; position:relative; box-sizing:border-box; width:14px; height:14px',
  );
  expect(css).toContain(
    "[data-sc-boolean-checkbox]:checked::after { content:''; position:absolute; box-sizing:border-box; width:9px; height:6px; left:50%; top:50%",
  );
  expect(css).toContain('transform:translate(-50%,-65%) rotate(-45deg)');
  expect(css).not.toContain('margin:2px 0 0 2px');
});

test('uses native collapse chevrons, smaller headings, and no field separators', () => {
  const css = nativeVisualStyleCss('photopro', false);
  expect(css).toContain('.sugarcubes-workflow-masonry__node-icon { display:none; }');
  expect(css).toContain('[data-testid="node-title"] {\n      padding-left:0; font-size:10px');
  expect(css).toContain(
    '[data-sc-titlebar-collapsed="true"] [data-testid="node-collapse-button"] i { transform:rotate(-90deg) !important; }',
  );
  expect(css).toContain('[data-testid="node-collapse-button"] {\n      left:0; right:auto;');
  expect(css).toContain('color:var(--sc-text-subtle); background:transparent; border:0;');
  expect(css).toContain('[data-sc-style-ports]:not([hidden])');
  expect(css).toContain('border-top:0 !important; border-bottom:0 !important;');
  expect(css).toContain('[data-sc-style-row][data-sc-style-column="2"]::before { display:none; }');
});

test('squares each PhotoPro masonry section to its measured extent without changing card sizes', () => {
  const css = nativeVisualStyleCss('photopro', false);
  expect(css).toContain(
    'html[data-sc-visual-style="photopro"] .sugarcubes-workflow-masonry__section::before',
  );
  expect(css).toContain('height:calc(var(--sc-masonry-section-height,36px) - 36px)');
  expect(css).toContain('box-shadow:inset 0 -1px 0 var(--sc-border)');
  expect(nativeVisualStyleCss('fluent', false)).not.toContain('--sc-masonry-section-height');
  expect(nativeVisualStyleCss('spectrum', false)).not.toContain('--sc-masonry-section-height');
});

test('places neutral activation beside the title and presents advanced settings as an ellipsis button', () => {
  const css = nativeVisualStyleCss('photopro', false);
  expect(css).toContain('--color-bypass:var(--sc-text-secondary)');
  expect(css).toContain('[data-cube-card-activation] { margin-inline-start:8px; padding:0; }');
  expect(css).toContain('width:28px; min-width:28px; height:20px; min-height:20px');
  expect(css).toContain(
    ':has(> [data-testid="advanced-inputs-button"]) { margin-top:0; z-index:auto; background:var(--sc-card-fill); }',
  );
  expect(css).toContain('[data-testid="advanced-inputs-button"]::after { content:\'…\'');
  expect(css).not.toContain(
    '.p-toggleswitch-checked .p-toggleswitch-slider { background:var(--sc-brand-fill)',
  );
});

test('skins action and ordinary choice menus once while preserving model picker popups', () => {
  const css = photoProMenuCss();
  expect(css).toContain(
    ':is([data-sc-menu-surface="actions"],[data-sc-menu-surface="choices"][data-sc-style-popup])',
  );
  expect(css).toContain('border-radius:0; background:#f0f0f0; color:#111; color-scheme:light');
  expect(css).toContain(
    '[data-sc-menu-surface="choices"][data-sc-style-popup]) :is([role^="menuitem"],[role="option"])',
  );
  expect(css).toContain('[data-state="checked"]');
  expect(css).toContain('[role="option"] > [aria-hidden="true"] { display:none; }');
  expect(css).toContain(
    '[role="option"] > :not([aria-hidden="true"]) { color:#111 !important; opacity:1; }',
  );
  expect(css).not.toContain('.sc-model-popup');
});

test('offers the four Photoshop brightness schemes through one semantic token owner', () => {
  expect(photoProTokenDeclarations('black')).toContain('--sc-surface:#242424');
  expect(photoProTokenDeclarations('dark-gray')).toContain('--sc-surface:#323232');
  expect(photoProTokenDeclarations('medium-gray')).toContain('--sc-surface:#535353');
  expect(photoProTokenDeclarations('light-gray')).toContain('--sc-surface:#d6d6d6');
  expect(photoProPaletteCss()).toContain('--sc-photopro-scroll-track:#161616');
  expect(photoProPaletteCss()).toContain('--sc-photopro-scroll-track:#b8b8b8');
  const css = nativeVisualStyleCss('photopro', false);
  expect(css).toContain(
    'html[data-sc-visual-style="photopro"][data-sc-photopro-palette="black"] :is([data-sc-style-theme-root],[data-sc-theme="photopro"])',
  );
  expect(css).toContain(
    'html[data-sc-visual-style="photopro"][data-sc-photopro-palette="light-gray"] :is([data-sc-style-theme-root],[data-sc-theme="photopro"])',
  );
  const registrations = new Map<string, UnknownRecord>();
  const report = jest.fn();
  const release = registerPhotoProPalettePreference(
    {
      ui: {
        settings: {
          getSettingValue: () => 'dark-gray',
          addSetting: (entry: unknown) => {
            if (isRecord(entry) && typeof entry.id === 'string') registrations.set(entry.id, entry);
          },
        },
      },
    },
    document,
    report,
  );
  expect(document.documentElement.dataset.scPhotoproPalette).toBe('dark-gray');
  const setting = registrations.get(PHOTOPRO_PALETTE_SETTING);
  expect(setting?.options).toEqual([
    { value: 'black', text: 'Black' },
    { value: 'dark-gray', text: 'Dark Gray' },
    { value: 'medium-gray', text: 'Medium Gray' },
    { value: 'light-gray', text: 'Light Gray' },
  ]);
  const onChange = setting?.onChange;
  if (typeof onChange !== 'function') throw new Error('PhotoPro palette callback is unavailable.');
  onChange('light-gray');
  expect(document.documentElement.dataset.scPhotoproPalette).toBe('light-gray');
  onChange('invalid');
  expect(report).toHaveBeenCalledWith(expect.any(TypeError));
  release();
});

test('scopes PhotoPro tokens to owned controls and removes them on release', () => {
  publishVisualStyle(document, 'photopro');
  const root = document.createElement('section');
  document.body.append(root);
  applyOwnedTheme(root, false);
  expect(root.dataset.scTheme).toBe('photopro');
  expect(root.dataset.scColorScheme).toBe('light');
  expect(document.getElementById('sugarcubes-theme-providers')?.textContent).toContain(
    '[data-sc-theme="photopro"][data-sc-color-scheme="dark"]',
  );
  applyOwnedTheme(root, null);
  expect(root.hasAttribute('data-sc-theme')).toBe(false);
});

test('uses native PhotoPro inputs with one value and keyboard selection owner', () => {
  publishVisualStyle(document, 'photopro');
  const checkbox = createBooleanCheckbox(document);
  const slider = createNumericSlider(document);
  const finder = createFinderInput(document);
  const segments = createBooleanSegments(document);
  const workflow = createWorkflowTablist(document);
  expect(checkbox).toBeInstanceOf(HTMLInputElement);
  expect(slider.root).toBeInstanceOf(HTMLInputElement);
  slider.synchronize(0, 100, 5, false, 35);
  expect(slider.readValue()).toBe(35);
  finder.setAttribute('placeholder', '<b>literal</b>');
  finder.value = '<img src=x>';
  document.body.append(finder, segments.group, workflow);
  expect(finder.control.value).toBe('<img src=x>');
  expect(finder.control.placeholder).toBe('<b>literal</b>');
  expect(finder.querySelector('img')).toBeNull();
  const [off, on] = segments.segments;
  segments.group.append(off, on);
  segments.group.activeid = off.id;
  let changes = 0;
  segments.group.addEventListener('change', () => changes++);
  on.click();
  expect(segments.group.activeid).toBe(on.id);
  expect(changes).toBe(1);
  segments.release();
});
