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
/** Prove the output presenter follows the first conclusive native discovery. */

import { OutputPresenterSelection } from '../../../frontend/comfyui/ui/output_canvas/application/OutputPresenterSelection.js';

/** Mutable host facts read through the selection's narrow port. */
function facts(prefersNative: boolean) {
  const state = { prefersNative, surface: null as object | null, discovered: false };
  const selection = new OutputPresenterSelection({
    prefersNative: () => state.prefersNative,
    surface: () => state.surface,
    discovered: () => state.discovered,
  });
  return { state, selection };
}

test('a preferring host stays pending until discovery, then keeps its negotiated surface', () => {
  const { state, selection } = facts(true);
  expect(selection.current()).toBe('pending');
  state.surface = {};
  state.discovered = true;
  expect(selection.current()).toBe('native');
  // A later context refresh withdraws the surface while it renegotiates.
  state.surface = null;
  expect(selection.current()).toBe('native');
});

test.each(['unsupported', 'unavailable', 'failed'])(
  'a preferring host whose startup discovery is %s falls back to the browser for the page',
  () => {
    const { state, selection } = facts(true);
    expect(selection.current()).toBe('pending');
    state.discovered = true;
    expect(selection.current()).toBe('browser');
    state.surface = {};
    expect(selection.current()).toBe('browser');
  },
);

test('without a host preference the browser presents until a surface is negotiated', () => {
  const { state, selection } = facts(false);
  expect(selection.current()).toBe('browser');
  expect(selection.current()).toBe('browser');
  state.surface = {};
  expect(selection.current()).toBe('native');
});
