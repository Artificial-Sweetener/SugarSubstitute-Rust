/** @jest-environment node */
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
/** Freeze the existing graph input record grammar independently of native archive authority. */
import { readFileSync } from 'node:fs';
import {
  readComfyWorkflowInputState,
  WORKFLOW_INPUT_STATE_KEY,
} from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyWorkflowInputState.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';

const value: unknown = JSON.parse(
  readFileSync(
    new URL('../support/comfy_workflow_input_state_vectors.json', import.meta.url),
    'utf8',
  ),
);
if (!isRecord(value) || !Array.isArray(value.cases))
  throw new Error('Input state vectors are unavailable.');
for (const fixture of value.cases) {
  if (!isRecord(fixture) || typeof fixture.name !== 'string' || typeof fixture.valid !== 'boolean')
    throw new Error('Input state vector is invalid.');
  test(fixture.name, () => {
    const state = readComfyWorkflowInputState({
      extra: { [WORKFLOW_INPUT_STATE_KEY]: fixture.record },
    });
    expect(state !== null).toBe(fixture.valid);
    if (state && isRecord(fixture.record)) {
      expect(state).toEqual({
        version: 1,
        id: fixture.record.id,
        route: fixture.record.route,
        surface: fixture.record.surface,
        mask: fixture.record.mask,
      });
    }
  });
}
