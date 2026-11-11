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
/** Prove one lifecycle owner distinguishes reversible caching from final page teardown. */
import { jest } from '@jest/globals';
import { ModelPageActivity } from '../../../frontend/comfyui/ui/model_library/ModelPageActivity.js';

test('pauses and resumes cached pages once despite duplicate browser transitions', () => {
  const change = jest.fn();
  const discard = jest.fn();
  const page = new ModelPageActivity(window, change, discard);
  try {
    page.start();
    page.start();
    window.dispatchEvent(new PageTransitionEvent('pageshow'));
    expect(change.mock.calls).toEqual([[true]]);
    window.dispatchEvent(new PageTransitionEvent('pagehide', { persisted: true }));
    window.dispatchEvent(new PageTransitionEvent('pagehide', { persisted: true }));
    expect(page.active).toBe(false);
    expect(change.mock.calls).toEqual([[true], [false]]);
    expect(discard).not.toHaveBeenCalled();
    window.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: true }));
    window.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: true }));
    expect(page.active).toBe(true);
    expect(change.mock.calls).toEqual([[true], [false], [true]]);
  } finally {
    page.dispose();
  }
});

test.each([false, true])('explicit disposal while cached=%s prevents restoration', (cached) => {
  const change = jest.fn();
  const discard = jest.fn();
  const page = new ModelPageActivity(window, change, discard);
  page.start();
  if (cached) window.dispatchEvent(new PageTransitionEvent('pagehide', { persisted: true }));
  page.dispose();
  page.dispose();
  window.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: true }));
  window.dispatchEvent(new PageTransitionEvent('pagehide'));
  expect(page.active).toBe(false);
  expect(change.mock.calls).toEqual([[true], [false]]);
  expect(discard).not.toHaveBeenCalled();
});

test('final teardown discards owners and removes restoration subscriptions', () => {
  const change = jest.fn();
  const discard = jest.fn();
  const page = new ModelPageActivity(window, change, discard);
  page.start();
  window.dispatchEvent(new PageTransitionEvent('pagehide'));
  window.dispatchEvent(new PageTransitionEvent('pagehide'));
  window.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: true }));
  expect(page.active).toBe(false);
  expect(change.mock.calls).toEqual([[true], [false]]);
  expect(discard).toHaveBeenCalledTimes(1);
});
