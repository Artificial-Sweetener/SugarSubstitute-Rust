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
/** Verify prompt-only Cube auto-connect routing after native subgraph flattening. */

import { ProximityPromptPatcher } from '../../../frontend/comfyui/ui/overlays/proximity/ProximityPromptPatcher.js';
import type { ProximityMatch } from '../../../frontend/comfyui/ui/overlays/proximity/ProximityModel.js';

describe('ProximityPromptPatcher', () => {
  test('connects every resolved nested input target without mutating workflow data', () => {
    const payload = {
      output: {
        '10:producer': { inputs: {}, class_type: 'Producer' },
        '20:nested:first': { inputs: { image: 1 }, class_type: 'Consumer' },
        '20:second': { inputs: {}, class_type: 'Consumer' },
      },
      workflow: { nodes: ['unchanged'] },
    };
    const patcher = new ProximityPromptPatcher();

    const result = patcher.apply(payload, [
      createMatch([
        { nodeId: '20:nested:first', inputSlot: 0, inputName: 'image' },
        { nodeId: '20:second', inputSlot: 2, inputName: 'source' },
      ]),
    ]);

    expect(result.payload).not.toBe(payload);
    expect(result.payload).toEqual({
      ...payload,
      output: {
        '10:producer': { inputs: {}, class_type: 'Producer' },
        '20:nested:first': {
          inputs: { image: ['10:producer', 1] },
          class_type: 'Consumer',
        },
        '20:second': {
          inputs: { source: ['10:producer', 1] },
          class_type: 'Consumer',
        },
      },
    });
    expect(payload.output['20:nested:first'].inputs.image).toBe(1);
    expect(result.applied).toHaveLength(1);
  });

  test('does not replace an explicit prompt connection', () => {
    const payload = {
      output: {
        target: { inputs: { image: ['explicit', 0] }, class_type: 'Consumer' },
      },
    };
    const result = new ProximityPromptPatcher().apply(payload, [
      createMatch([{ nodeId: 'target', inputSlot: 0, inputName: 'image' }]),
    ]);

    expect(result.payload).toBe(payload);
    expect(result.applied).toEqual([]);
  });
});

function createMatch(promptTargets: ProximityMatch['promptTargets']): ProximityMatch {
  return {
    outputId: 10,
    outputSlot: 0,
    outputNode: {},
    outputPos: [0, 0],
    inputId: 20,
    inputSlot: 0,
    inputName: 'image',
    inputNode: {},
    inputPos: [10, 0],
    originId: '10:producer',
    originSlot: 1,
    promptTargets,
    distance: 10,
  };
}
