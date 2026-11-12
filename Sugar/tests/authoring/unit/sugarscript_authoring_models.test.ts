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
/** Verify untrusted SugarScript plans fail closed before graph mutation. */

import { readSugarScriptAuthoringResponse } from '../../../frontend/comfyui/ui/sugarscript/SugarScriptAuthoringModels.js';

const INSTANCE = {
  instance_id: 'instance-1',
  alias: 'Cube',
  bypassed: false,
  payload: { cube: { cube_id: 'local/tests/cube.cube', version: '1.0.0' } },
};

test('reads one complete native workflow plan', () => {
  const result = readSugarScriptAuthoringResponse({
    valid: true,
    diagnostics: [],
    plan: {
      semantic_hash: 'a'.repeat(64),
      instances: [INSTANCE],
      connections: [],
    },
  });

  expect(result.plan?.instances[0]).toMatchObject({ instanceId: 'instance-1', alias: 'Cube' });
});

test('rejects duplicate ids and dangling connections', () => {
  expect(() =>
    readSugarScriptAuthoringResponse({
      valid: true,
      diagnostics: [],
      plan: {
        semantic_hash: 'a'.repeat(64),
        instances: [INSTANCE, INSTANCE],
        connections: [],
      },
    }),
  ).toThrow('duplicate instance ids');

  expect(() =>
    readSugarScriptAuthoringResponse({
      valid: true,
      diagnostics: [],
      plan: {
        semantic_hash: 'a'.repeat(64),
        instances: [INSTANCE],
        connections: [
          {
            source_instance_id: 'instance-1',
            source_binding: 'output.image',
            target_instance_id: 'missing',
            target_binding: 'input.image',
          },
        ],
      },
    }),
  ).toThrow('unknown instance');
});

test('retains located diagnostics when compilation has no plan', () => {
  const result = readSugarScriptAuthoringResponse({
    valid: false,
    plan: null,
    diagnostics: [
      {
        code: 'sugarscript.parse.unexpected_token',
        severity: 'error',
        message: 'Unexpected token.',
        span: {
          start: { offset: 4, line: 2, column: 1 },
          end: { offset: 5, line: 2, column: 2 },
        },
      },
    ],
  });

  expect(result.diagnostics[0]?.span.start.line).toBe(2);
  expect(result.plan).toBeNull();
});

test('rejects coercible source positions from an untrusted response', () => {
  expect(() =>
    readSugarScriptAuthoringResponse({
      valid: false,
      plan: null,
      diagnostics: [
        {
          code: 'sugarscript.parse.test',
          severity: 'error',
          message: 'Invalid.',
          span: {
            start: { offset: '0', line: 1, column: 1 },
            end: { offset: 1, line: 1, column: 2 },
          },
        },
      ],
    }),
  ).toThrow('source position');
});
