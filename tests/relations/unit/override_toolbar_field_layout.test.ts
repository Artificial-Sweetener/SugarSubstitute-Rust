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
/** Verify width contention preserves full controls whenever scrolling cannot be avoided. */
import { expect, test } from '@jest/globals';
import { OverrideToolbarFieldLayout } from '../../../frontend/comfyui/ui/relations/presentation/OverrideToolbarFieldLayout.js';

test('shrinks only when minimum widths can fit, restores preferred widths on overflow, and recovers after resizing', () => {
  const fields = document.createElement('div');
  let available = 400;
  Object.defineProperties(fields, {
    clientWidth: { get: () => available },
    scrollWidth: {
      get: () =>
        fields.hasAttribute('data-sc-override-overflow') ? 720 : Math.max(344, available),
    },
  });
  const layout = new OverrideToolbarFieldLayout(fields);
  layout.reconcile();
  expect(fields.hasAttribute('data-sc-override-overflow')).toBe(false);
  available = 240;
  layout.reconcile();
  expect(fields.hasAttribute('data-sc-override-overflow')).toBe(true);
  expect(fields.scrollWidth).toBe(720);
  layout.reconcile();
  expect(fields.scrollWidth).toBe(720);
  available = 400;
  layout.reconcile();
  expect(fields.hasAttribute('data-sc-override-overflow')).toBe(false);
  fields.hidden = true;
  available = 0;
  layout.reconcile();
  expect(fields.hasAttribute('data-sc-override-overflow')).toBe(false);
});
