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
/** Prove every seed action has an icon from the active Fluent or native Comfy family. */
import { createSeedActionIcon } from '../../../frontend/comfyui/ui/surface/appearance/SeedActionIcons.js';
import type { SeedMode } from '../../../frontend/comfyui/ui/graph/ComfySeedMode.js';

const actions: readonly (SeedMode | 'reuse-last')[] = [
  'fixed',
  'randomize',
  'increment',
  'decrement',
  'reuse-last',
];

test.each(actions)('uses a pinned Fluent icon for %s including its own reuse glyph', (action) => {
  const icon = createSeedActionIcon(document, action, true);
  expect(icon.dataset.scSeedIconFamily).toBe('fluent');
  expect(icon.style.getPropertyValue('--sc-seed-icon')).toContain('/icons/seed/');
  expect(icon.style.background).toBe('currentColor');
  if (action === 'reuse-last') expect(icon.style.mask).toContain('reuse-last.svg');
});

test('uses native Comfy seed symbols and a Lucide restore icon outside Fluent appearance', () => {
  const expected = {
    fixed: 'icon-[lucide--pencil-off]',
    randomize: 'icon-[lucide--shuffle]',
    'reuse-last': 'icon-[lucide--rotate-ccw]',
  };
  for (const action of actions) {
    const icon = createSeedActionIcon(document, action, false);
    expect(icon.dataset.scSeedIconFamily).toBe('comfy');
    expect(icon.style.mask).toBe('');
    if (action === 'increment' || action === 'decrement')
      expect(icon.textContent).toBe(action === 'increment' ? '+1' : '-1');
    else expect(icon.className).toBe(expected[action]);
  }
});
