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
  test('reserves the hidden-slot gutter only for ordinary widget cards', () => {
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

    expect(measureCubeFaceNodeBodyHeight(prompt)).toBe(88);
    expect(measureCubeFaceNodeBodyHeight(ordinary)).toBe(92);
  });

  test('fits one compact combo below its native hidden-slot gutter', () => {
    const combo = {
      id: 'style',
      type: 'PromptEncodeStyle',
      widgets: [{ name: 'encode_style', computedHeight: 24 }],
    };

    expect(measureCubeFaceNodeBodyHeight(combo)).toBe(36);
  });

  test('leaves one native bottom margin after a compact widget stack', () => {
    const sampler = {
      id: 'sampler',
      type: 'KSampler',
      widgets: Array.from({ length: 7 }, (_, index) => ({ name: `widget-${String(index)}` })),
    };

    expect(measureCubeFaceNodeBodyHeight(sampler)).toBe(180);
  });

  test('retains native preview height for an ordered mask loader in Nodes 1.0', () => {
    const maskLoader = {
      id: 'mask-loader',
      type: 'SimpleSyrup.LoadMaskBatch',
      size: [320, 420],
      imgs: [{ src: 'mask-one.png' }, { src: 'mask-two.png' }],
      widgets: [
        { name: 'channel', computedHeight: 24 },
        { name: 'replace', computedHeight: 24 },
        { name: 'add', computedHeight: 24 },
      ],
    };

    expect(measureCubeFaceNodeBodyHeight(maskLoader)).toBe(420);
  });
});
