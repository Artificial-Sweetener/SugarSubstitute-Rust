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
/** Prove owned surfaces receive immutable, deduplicated accepted appearance facts. */
import { expect, jest, test } from '@jest/globals';
import { AppearanceState } from '../../../frontend/comfyui/ui/surface/appearance/AppearanceState.js';

test('publishes coherent appearance changes and releases surface subscriptions', () => {
  const state = new AppearanceState();
  const snapshots: object[] = [];
  const unsubscribe = state.subscribeAppearance(() => {
    snapshots.push(state.appearance);
  });
  state.update('fluent', false);
  state.update('fluent', false);
  state.update('comfy', false);
  expect(snapshots).toEqual([
    { style: 'fluent', dark: false },
    { style: 'comfy', dark: false },
  ]);
  expect(snapshots.every(Object.isFrozen)).toBe(true);
  unsubscribe();
  state.update('fluent', true);
  expect(snapshots).toHaveLength(2);
  const late = jest.fn();
  state.subscribeAppearance(late);
  state.dispose();
  state.update('comfy', true);
  expect(late).not.toHaveBeenCalled();
});
