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
/** Verify Nodes 1.0 Cube-face body measurement. */

import { measureCubeFaceNodeBodyHeight } from '../../frontend/comfyui/ui/surface/CubeFaceNodeMeasurement.js';

describe('CubeFaceNodeMeasurement', () => {
  test('does not count the hidden-slot top gutter as bottom body space', () => {
    const prompt = {
      id: 'prompt',
      title: 'Positive prompt',
      type: 'CLIPTextEncode',
      widgets: [{ name: 'text', type: 'customtext', computedHeight: 80 }],
    };
    const ordinary = {
      id: 'ordinary',
      title: 'Notes',
      type: 'Notes',
      widgets: [{ name: 'text', type: 'customtext', computedHeight: 80 }],
    };

    expect(measureCubeFaceNodeBodyHeight(ordinary)).toBe(measureCubeFaceNodeBodyHeight(prompt));
  });

  test('leaves one native bottom margin after a compact widget stack', () => {
    const sampler = {
      id: 'sampler',
      type: 'KSampler',
      widgets: Array.from({ length: 7 }, (_, index) => ({ name: `widget-${String(index)}` })),
    };

    expect(measureCubeFaceNodeBodyHeight(sampler)).toBe(182);
  });
});
