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
/** Prove the native presentation status vocabulary independently of host operation order. */

import { nativeCanvasDesired } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasDesired.js';
import {
  NATIVE_CANVAS_RESULT_FAILURE,
  nativeCanvasFailureStatus,
  nativeCanvasPendingStatus,
  nativeCanvasSettledStatus,
} from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasStatus.js';
import type { CanvasSurfaceGeometry } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';

const geometry: CanvasSurfaceGeometry = {
  coordinateSpace: 'client',
  units: 'css-pixels',
  origin: { x: 0, y: 0 },
  viewport: { width: 400, height: 300 },
  clip: { x: 0, y: 0, width: 400, height: 300 },
  physicalPixelsPerCssPixel: 1,
  visible: true,
  focused: false,
};
const item = canvasCatalogFixture().entries[0]!.items[0]!;
const shown = nativeCanvasDesired(geometry, item, null, null, true, false);
const previewLane = nativeCanvasDesired(geometry, null, null, null, true, false);

test('loads shown content only on an available or still-discovered host', () => {
  expect(nativeCanvasPendingStatus(shown, true, () => false)).toEqual({
    state: 'loading',
    message: 'Loading the selected image.',
  });
  expect(nativeCanvasPendingStatus(shown, false, () => true)).toEqual({
    state: 'loading',
    message: '',
  });
  expect(nativeCanvasPendingStatus(shown, false, () => false).state).toBe('unavailable');
  expect(nativeCanvasPendingStatus(previewLane, true, () => true)).toEqual({
    state: 'empty',
    message: '',
  });
});

test('settles shown content and observed preview lanes as ready', () => {
  const idle = { displayed: false, observing: false, preparing: false };
  expect(nativeCanvasSettledStatus(shown, null, null).state).toBe('ready');
  expect(nativeCanvasSettledStatus(previewLane, idle, null).state).toBe('empty');
  expect(nativeCanvasSettledStatus(previewLane, { ...idle, preparing: true }, null).state).toBe(
    'loading',
  );
  expect(nativeCanvasSettledStatus(previewLane, { ...idle, observing: true }, 3).state).toBe(
    'ready',
  );
  expect(nativeCanvasSettledStatus(previewLane, { ...idle, displayed: true }, null).state).toBe(
    'ready',
  );
});

test('names rejected operations with the caller, host or default message', () => {
  expect(nativeCanvasFailureStatus({ status: 'error', message: '' })).toEqual({
    state: 'error',
    message: 'Native canvas presentation failed.',
  });
  expect(
    nativeCanvasFailureStatus({ status: 'error', message: 'decode' }, NATIVE_CANVAS_RESULT_FAILURE),
  ).toEqual({ state: 'error', message: NATIVE_CANVAS_RESULT_FAILURE });
  expect(nativeCanvasFailureStatus({ status: 'unavailable', message: 'busy' })).toEqual({
    state: 'unavailable',
    message: 'busy',
  });
  expect(nativeCanvasFailureStatus({ status: 'stale', message: '' }).message).toBe(
    'The native canvas is unavailable.',
  );
});
