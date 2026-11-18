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
/** Prove a person's input is reported once per burst, after it rests, and never after disposal. */
import { afterEach, beforeEach, expect, jest, test } from '@jest/globals';
import {
  AUTHORED_EDIT_QUIET_MS,
  AuthoredEditObservation,
} from '../../../frontend/comfyui/ui/workflow/infrastructure/AuthoredEditObservation.js';

beforeEach(() => {
  jest.useFakeTimers();
});
afterEach(() => {
  jest.useRealTimers();
  document.body.replaceChildren();
});

/** Observe the test document with the page's own timers. */
function observe() {
  const settled = jest.fn();
  const edits = new AuthoredEditObservation(
    {
      document,
      setTimeout: (handler, delay) => window.setTimeout(handler, delay),
      clearTimeout: (handle) => window.clearTimeout(handle),
    },
    settled,
  );
  edits.install();
  return { edits, settled };
}

test.each(['input', 'change', 'pointerup', 'keyup'])(
  '%s anywhere in the page is reported after input rests',
  (type) => {
    const { edits, settled } = observe();
    const field = document.createElement('input');
    document.body.append(field);
    // A surface that keeps its events to itself is still observed.
    field.addEventListener(type, (event) => event.stopPropagation());
    field.dispatchEvent(new Event(type, { bubbles: true }));
    jest.advanceTimersByTime(AUTHORED_EDIT_QUIET_MS - 1);
    expect(settled).not.toHaveBeenCalled();
    jest.advanceTimersByTime(1);
    expect(settled).toHaveBeenCalledTimes(1);
    edits.dispose();
  },
);

test('a burst of typing is one report, made after its last key', () => {
  const { edits, settled } = observe();
  for (let key = 0; key < 20; key++) {
    document.body.dispatchEvent(new Event('input', { bubbles: true }));
    jest.advanceTimersByTime(AUTHORED_EDIT_QUIET_MS - 50);
  }
  expect(settled).not.toHaveBeenCalled();
  jest.advanceTimersByTime(AUTHORED_EDIT_QUIET_MS);
  expect(settled).toHaveBeenCalledTimes(1);
  edits.dispose();
});

test('disposal stops observation and drops a report not yet made', () => {
  const { edits, settled } = observe();
  document.body.dispatchEvent(new Event('change', { bubbles: true }));
  edits.dispose();
  jest.advanceTimersByTime(AUTHORED_EDIT_QUIET_MS * 2);
  document.body.dispatchEvent(new Event('change', { bubbles: true }));
  jest.advanceTimersByTime(AUTHORED_EDIT_QUIET_MS * 2);
  expect(settled).not.toHaveBeenCalled();
});
