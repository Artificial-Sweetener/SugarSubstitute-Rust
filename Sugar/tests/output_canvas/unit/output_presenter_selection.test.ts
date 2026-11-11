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
/** Prove native still-image ownership and distinct media-viewer handoffs. */

import {
  OutputPresenterSelection,
  canvasRequiresBrowserMedia,
} from '../../../frontend/comfyui/ui/output_canvas/application/OutputPresenterSelection.js';
import { mixedMediaFixture } from '../support/CanvasMixedMediaFixture.js';

/** Mutable host facts read through the selection's narrow port. */
function facts(requiresNative: boolean) {
  const state = {
    requiresNative,
    surface: null as object | null,
    discovered: false,
    browserMedia: false,
  };
  const port = {
    requiresNative: () => state.requiresNative,
    surface: () => state.surface,
    discovered: () => state.discovered,
    browserMedia: () => state.browserMedia,
  };
  const selection = new OutputPresenterSelection(port);
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
  state.discovered = false;
  expect(selection.current()).toBe('pending');
  state.discovered = true;
  expect(selection.current()).toBe('native');
});

test.each(['unsupported', 'unavailable', 'failed'])(
  'a native host whose startup discovery is %s never selects browser image pixels',
  () => {
    const { state, selection } = facts(true);
    expect(selection.current()).toBe('pending');
    state.discovered = true;
    expect(selection.current()).toBe('native');
    state.surface = {};
    expect(selection.current()).toBe('native');
  },
);

test('without a host preference the browser presents until a surface is negotiated', () => {
  const { state, selection } = facts(false);
  expect(selection.current()).toBe('browser');
  expect(selection.current()).toBe('browser');
  state.surface = {};
  expect(selection.current()).toBe('native');
});

test('native image to browser media and back preserves the admitted native presenter for Input and stills', () => {
  const { state, selection } = facts(true);
  state.surface = {};
  state.discovered = true;
  expect(selection.current()).toBe('native');
  state.browserMedia = true;
  expect(selection.current()).toBe('browser');
  state.browserMedia = false;
  expect(selection.current()).toBe('native');
  state.surface = null;
  expect(selection.current()).toBe('native');
});

test('content routing consumes the existing selection and grid plan', () => {
  const mixed = mixedMediaFixture();
  expect(canvasRequiresBrowserMedia(mixed.plan(0))).toBe(false);
  expect(canvasRequiresBrowserMedia(mixed.plan(1))).toBe(true);
  expect(canvasRequiresBrowserMedia(mixed.plan(3))).toBe(true);
  expect(canvasRequiresBrowserMedia(mixed.plan(0, true))).toBe(true);
});

test('restored still images retain native ownership for every encoded format', () => {
  const mixed = mixedMediaFixture();
  const plan = mixed.plan(0);
  if (!plan.primary?.item) throw new Error('Image fixture has no selected item.');
  const restoredWebp = {
    ...plan,
    primary: { ...plan.primary, item: { ...plan.primary.item, encodedMime: 'image/webp' } },
  };
  expect(canvasRequiresBrowserMedia(restoredWebp)).toBe(false);
  const { state, selection } = facts(true);
  state.discovered = true;
  state.browserMedia = canvasRequiresBrowserMedia(restoredWebp);
  expect(selection.current()).toBe('native');
});
