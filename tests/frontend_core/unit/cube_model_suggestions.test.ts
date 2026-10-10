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
/** Verify authoring suggestions remain available without library presentation. */
import { expect, jest, test } from '@jest/globals';
import {
  CubeModelSuggestions,
  type CubeModelSuggestionFetch,
} from '../../../frontend/comfyui/ui/core/CubeModelSuggestions.js';

test('loads normalized vocabulary once and isolates dialog edits', async () => {
  const fetch = jest.fn<CubeModelSuggestionFetch>(async () => ({
    ok: true,
    text: async () => ' Anima \nSDXL\nAnima\n',
  }));
  const suggestions = new CubeModelSuggestions(fetch, { warn: jest.fn() });
  await Promise.all([suggestions.setup(), suggestions.setup()]);
  expect(fetch).toHaveBeenCalledTimes(1);
  expect(suggestions.getModelSuggestions()).toEqual(['Anima', 'SDXL']);
  const copy = suggestions.getModelSuggestions();
  expect(copy).not.toBe(suggestions.getModelSuggestions());
});

test('tries the host extension URL after a resource failure', async () => {
  const fetch = jest.fn<CubeModelSuggestionFetch>();
  fetch.mockRejectedValueOnce(new Error('resource missing'));
  fetch.mockResolvedValueOnce({ ok: true, text: async () => 'Flux\n' });
  const warn = jest.fn();
  const suggestions = new CubeModelSuggestions(fetch, { warn });
  await suggestions.setup();
  expect(fetch).toHaveBeenLastCalledWith(
    '/extensions/ComfyUI-SugarCubes/models.txt',
    expect.objectContaining({ signal: expect.any(AbortSignal) }),
  );
  expect(suggestions.getModelSuggestions()).toEqual(['Flux']);
  expect(warn).toHaveBeenCalledTimes(1);
});

test('retains Other when both bundled resources are unavailable', async () => {
  const suggestions = new CubeModelSuggestions(async () => ({ ok: false, text: async () => '' }), {
    warn: jest.fn(),
  });
  await suggestions.setup();
  expect(suggestions.getModelSuggestions()).toEqual(['Other']);
});
