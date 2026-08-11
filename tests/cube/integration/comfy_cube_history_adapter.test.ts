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
/** Verify Cube history enters Comfy's authoritative canvas transaction boundary. */

import { jest } from '@jest/globals';
import { createComfyCubeHistoryAdapter } from '../../../frontend/comfyui/ui/cube/ComfyCubeHistoryAdapter.js';

describe('createComfyCubeHistoryAdapter', () => {
  test('uses canvas history events instead of graph callbacks when both exist', () => {
    const emitBeforeChange = jest.fn();
    const emitAfterChange = jest.fn();
    const setDirty = jest.fn();
    const graphBeforeChange = jest.fn();
    const graphAfterChange = jest.fn();
    const graphSetDirtyCanvas = jest.fn();
    const history = createComfyCubeHistoryAdapter({
      canvas: { emitBeforeChange, emitAfterChange, setDirty },
      graph: {
        beforeChange: graphBeforeChange,
        afterChange: graphAfterChange,
        setDirtyCanvas: graphSetDirtyCanvas,
      },
    });

    history.beforeChange();
    history.setDirtyCanvas(true, true);
    history.afterChange();

    expect(emitBeforeChange).toHaveBeenCalledTimes(1);
    expect(emitAfterChange).toHaveBeenCalledTimes(1);
    expect(setDirty).toHaveBeenCalledWith(true, true);
    expect(graphBeforeChange).not.toHaveBeenCalled();
    expect(graphAfterChange).not.toHaveBeenCalled();
    expect(graphSetDirtyCanvas).not.toHaveBeenCalled();
  });

  test('uses the graph boundary only for hosts without canvas history events', () => {
    const beforeChange = jest.fn();
    const afterChange = jest.fn();
    const setDirtyCanvas = jest.fn();
    const history = createComfyCubeHistoryAdapter({
      canvas: {},
      graph: { beforeChange, afterChange, setDirtyCanvas },
    });

    history.beforeChange();
    history.setDirtyCanvas(false, true);
    history.afterChange();

    expect(beforeChange).toHaveBeenCalledTimes(1);
    expect(afterChange).toHaveBeenCalledTimes(1);
    expect(setDirtyCanvas).toHaveBeenCalledWith(false, true);
  });
});
