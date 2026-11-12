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
/** Prove compact masonry is one persisted preference independent of visual style. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  COMPACT_MASONRY_CHANGE,
  COMPACT_MASONRY_SETTING,
  registerCompactMasonryPreference,
} from '../../../frontend/comfyui/ui/surface/masonry/CompactMasonryPreference.js';
import { ensureWorkflowMasonryStyles } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryStyles.js';
import { ensureNodeCardAppearanceStyles } from '../../../frontend/comfyui/ui/surface/appearance/NodeCardAppearanceStyles.js';
import { nativeVisualStyleCss } from '../../../frontend/comfyui/ui/surface/appearance/ThemeService.js';
import { isRecord, type UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';

afterEach(() => {
  document.documentElement.removeAttribute('data-sc-compact-masonry');
  document.head.replaceChildren();
});

test('switches compact packing without touching the selected visual style', () => {
  const registrations = new Map<string, UnknownRecord>();
  const report = jest.fn();
  const change = jest.fn();
  document.documentElement.dataset.scVisualStyle = 'photopro';
  document.addEventListener(COMPACT_MASONRY_CHANGE, change);
  const release = registerCompactMasonryPreference(
    {
      ui: {
        settings: {
          getSettingValue: () => true,
          addSetting: (value: unknown) => {
            if (isRecord(value) && typeof value.id === 'string') registrations.set(value.id, value);
          },
        },
      },
    },
    document,
    report,
  );
  expect(document.documentElement.hasAttribute('data-sc-compact-masonry')).toBe(true);
  const registration = registrations.get(COMPACT_MASONRY_SETTING);
  expect(registration?.type).toBe('boolean');
  const onChange = registration?.onChange;
  if (typeof onChange !== 'function') throw new Error('Compact setting callback is unavailable.');
  onChange(false);
  expect(document.documentElement.hasAttribute('data-sc-compact-masonry')).toBe(false);
  expect(document.documentElement.dataset.scVisualStyle).toBe('photopro');
  expect(change).toHaveBeenCalledTimes(2);
  onChange('invalid');
  expect(report).toHaveBeenCalledWith(expect.any(TypeError));
  release();
  document.removeEventListener(COMPACT_MASONRY_CHANGE, change);
  document.documentElement.removeAttribute('data-sc-visual-style');
});

test('packs compact masonry while preserving the native resize corner', () => {
  ensureWorkflowMasonryStyles(document);
  const css = document.getElementById('sugarcubes-workflow-masonry-styles')?.textContent;
  expect(css).toContain(':root[data-sc-compact-masonry] .sugarcubes-workflow-masonry, ');
  expect(css).toContain('--sugarcubes-masonry-gap:0;');
  expect(css).toContain(
    'padding:0 0 var(--sc-panel-resize-corner,0px) var(--sc-masonry-stack-width,0px);',
  );
  ensureNodeCardAppearanceStyles(document);
  expect(document.getElementById('sugarcubes-node-card-appearance')?.textContent).toContain(
    'border-radius:0 !important;',
  );
  expect(document.getElementById('sugarcubes-node-card-appearance')?.textContent).toContain(
    'border-width:0 1px 1px 0;',
  );
  expect(document.getElementById('sugarcubes-node-card-appearance')?.textContent).toContain(
    '[data-testid="node-inner-wrapper"] { border:0 !important; }',
  );
  expect(nativeVisualStyleCss('photopro', false)).toContain('padding-top:4px; padding-bottom:4px;');
});
