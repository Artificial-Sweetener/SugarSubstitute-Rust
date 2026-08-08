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
/** Verify one source policy serves picker and browser catalog shapes. */

import { resolveCubePackIdentity } from '../../frontend/comfyui/ui/core/CubePackIdentity.js';

test('resolves current picker repoRef into its pack name', () => {
  expect(
    resolveCubePackIdentity({
      source: { kind: 'github', repoRef: 'Artificial-Sweetener/Base-Cubes' },
    }),
  ).toEqual({
    key: 'github:artificial-sweetener/base-cubes',
    label: 'Base-Cubes',
    authorLabel: 'Artificial-Sweetener',
  });
});

test('preserves browser and local source compatibility through the same owner', () => {
  expect(
    resolveCubePackIdentity({
      source: { type: 'github', repo_ref: 'Artificial-Sweetener/Base-Cubes' },
    }).label,
  ).toBe('Base-Cubes');
  expect(resolveCubePackIdentity({ source: { kind: 'local', namespace: 'personal' } })).toEqual({
    key: 'local:personal',
    label: 'local',
    authorLabel: 'personal',
  });
});
