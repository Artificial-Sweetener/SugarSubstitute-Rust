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
/** Verify renderer reconciliation stays bounded and event-driven. */

import { jest } from '@jest/globals';
import { CubeRendererTransitionStabilizer } from '../../frontend/comfyui/ui/surface/CubeRendererTransitionStabilizer.js';

describe('CubeRendererTransitionStabilizer', () => {
  test('reconciles only the configured transition frames and then goes idle', () => {
    const frames: FrameRequestCallback[] = [];
    const reconcile = jest.fn();
    const stabilizer = new CubeRendererTransitionStabilizer({
      requestFrame: (callback) => {
        frames.push(callback);
        return frames.length;
      },
      cancelFrame: jest.fn(),
      frameCount: 3,
    });

    stabilizer.run(reconcile);
    frames[0]?.(0);
    frames[1]?.(16);
    frames[2]?.(32);

    expect(reconcile).toHaveBeenCalledTimes(3);
    expect(frames).toHaveLength(3);
    stabilizer.dispose();
  });
});
