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
/** Prove how a refused viewport request is classified while keys and gestures overtake each other. */
import { expect, test } from '@jest/globals';
import { viewportRejection } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasViewportIntent.js';

test('a command a newer camera superseded recovers the camera shown without a failure', () => {
  expect(viewportRejection('stale', true, true, false)).toEqual({
    recover: true,
    retry: true,
    settled: false,
    parked: false,
    failed: false,
  });
});

test('a superseded automatic read leaves the gesture camera to report itself', () => {
  expect(viewportRejection('stale', false, true, false)).toEqual({
    recover: false,
    retry: false,
    settled: true,
    parked: false,
    failed: false,
  });
});

test('a stale answer while a preview owns the camera is parked rather than failed', () => {
  for (const admitted of [true, false])
    expect(viewportRejection('stale', admitted, true, true)).toEqual({
      recover: false,
      retry: false,
      settled: false,
      parked: true,
      failed: false,
    });
});

test('an unavailable host recovers a command but fails an automatic read', () => {
  expect(viewportRejection('unavailable', true, true, false)).toMatchObject({
    recover: true,
    retry: true,
    failed: false,
  });
  expect(viewportRejection('unavailable', false, true, false)).toMatchObject({
    recover: false,
    retry: false,
    failed: true,
  });
});

test('cancellation retries an admitted command without reporting a failure', () => {
  expect(viewportRejection('cancelled', true, true, false)).toMatchObject({
    retry: true,
    failed: false,
  });
  expect(viewportRejection('cancelled', false, true, false)).toMatchObject({
    retry: false,
    failed: false,
  });
});

test('an obsolete request never publishes or settles anything', () => {
  for (const status of ['stale', 'error', 'unavailable', 'cancelled'] as const)
    expect(viewportRejection(status, false, false, false)).toMatchObject({
      settled: false,
      parked: false,
      failed: false,
    });
});

test('a host error is a failure', () => {
  expect(viewportRejection('error', true, true, false)).toMatchObject({
    recover: false,
    failed: true,
  });
});
