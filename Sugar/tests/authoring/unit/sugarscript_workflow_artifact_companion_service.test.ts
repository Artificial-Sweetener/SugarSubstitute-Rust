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
/** Verify paired workflow imports retain source without changing execution authority. */

import { jest } from '@jest/globals';
import type { SugarScriptAuthoringResponse } from '../../../frontend/comfyui/ui/sugarscript/SugarScriptAuthoringModels.js';
import { SugarScriptWorkflowArtifactCompanionService } from '../../../frontend/comfyui/ui/sugarscript/SugarScriptWorkflowArtifactCompanionService.js';

const semanticHash = 'a'.repeat(64);

test('retains valid imported source as detached when synchronization is unproven', async () => {
  const graph = { extra: { existing: true } };
  const compile = jest.fn(async (): Promise<SugarScriptAuthoringResponse> => validResponse());
  const service = new SugarScriptWorkflowArtifactCompanionService({ compile }, () => graph);

  await expect(service.retain('use "sdxl" as Base')).resolves.toEqual({
    state: 'detached',
    diagnostics: [],
  });

  expect(graph.extra).toEqual({
    existing: true,
    sugarcubes_sugarscript: {
      schema: 1,
      state: 'detached',
      reason: 'synchronization_unproven',
      source: 'use "sdxl" as Base',
      source_semantics: {
        version: 'sugarscript-native-plan-v1',
        semantic_hash: semanticHash,
      },
      diagnostics: [],
    },
  });
});

test('retains malformed source and its located diagnostics beside the valid workflow', async () => {
  const graph: { extra?: unknown } = {};
  const response: SugarScriptAuthoringResponse = {
    valid: false,
    plan: null,
    diagnostics: [
      {
        code: 'sugarscript.parse.expected_expression',
        message: 'Expected an expression.',
        severity: 'error',
        span: {
          start: { offset: 4, line: 2, column: 3 },
          end: { offset: 5, line: 2, column: 4 },
        },
      },
    ],
  };
  const service = new SugarScriptWorkflowArtifactCompanionService(
    { compile: async () => response },
    () => graph,
  );

  await expect(service.retain('bad source')).resolves.toMatchObject({ state: 'invalid' });

  expect(graph.extra).toEqual({
    sugarcubes_sugarscript: {
      schema: 1,
      state: 'invalid',
      reason: 'source_diagnostics',
      source: 'bad source',
      diagnostics: response.diagnostics,
    },
  });
});

test('retains source as unverified and reports transport failure to the host adapter', async () => {
  const graph: { extra?: unknown } = {};
  const service = new SugarScriptWorkflowArtifactCompanionService(
    {
      compile: async () => {
        throw new Error('authoring endpoint unavailable');
      },
    },
    () => graph,
  );

  await expect(service.retain('use "sdxl" as Base')).rejects.toThrow(
    'authoring endpoint unavailable',
  );

  expect(graph.extra).toEqual({
    sugarcubes_sugarscript: {
      schema: 1,
      state: 'unverified',
      reason: 'compilation_unavailable',
      source: 'use "sdxl" as Base',
      diagnostics: [],
    },
  });
});

test('fails before compilation when the authoritative root graph is unavailable', async () => {
  const compile = jest.fn(async (): Promise<SugarScriptAuthoringResponse> => validResponse());
  const service = new SugarScriptWorkflowArtifactCompanionService({ compile }, () => null);

  await expect(service.retain('use "sdxl" as Base')).rejects.toThrow(
    'Comfy root graph is unavailable',
  );
  expect(compile).not.toHaveBeenCalled();
});

function validResponse(): SugarScriptAuthoringResponse {
  return {
    valid: true,
    diagnostics: [],
    plan: { semanticHash, instances: [], connections: [] },
  };
}
