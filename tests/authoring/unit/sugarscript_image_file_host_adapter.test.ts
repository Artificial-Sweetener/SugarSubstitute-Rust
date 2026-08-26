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
/** Verify script-only image handling composes with Comfy's file authority. */

import { jest } from '@jest/globals';
import {
  SugarScriptImageFileHostAdapter,
  type SugarScriptFileHost,
} from '../../../frontend/comfyui/ui/sugarscript/SugarScriptImageFileHostAdapter.js';
import type { PngArtifactInput } from '../../../frontend/comfyui/ui/sugarscript/PngSugarScriptArtifactReader.js';

test('imports a script-only image without invoking Comfy workflow loading', async () => {
  const original = jest.fn(async () => 'host');
  const host: SugarScriptFileHost = { handleFile: original };
  const importSource = jest.fn(async () => 'imported');
  const adapter = createAdapter(host, importSource);
  adapter.setup();

  const result = await host.handleFile(
    pngFile(textChunk('sugar_script', 'use "sdxl" as Base\n')),
    'file_drop',
  );

  expect(result).toBe('imported');
  expect(importSource).toHaveBeenCalledWith('use "sdxl" as Base\n');
  expect(original).not.toHaveBeenCalled();
  adapter.dispose();
  expect(host.handleFile).toBe(original);
});

test('materializes valid SugarScript from paired images without loading stale workflow state', async () => {
  const original = jest.fn(async () => 'host');
  const host: SugarScriptFileHost = { handleFile: original };
  const importSource = jest.fn(async () => undefined);
  const retainWorkflowSource = jest.fn(async () => undefined);
  const validateWorkflow = jest.fn(async () => undefined);
  createAdapter(host, importSource, retainWorkflowSource, undefined, validateWorkflow).setup();
  const paired = pngFile(
    textChunk('sugar_script', 'use "sdxl" as Base\n'),
    textChunk('workflow', JSON.stringify(managedWorkflow())),
  );

  await expect(
    host.handleFile(paired, 'file_drop', { deferWarnings: true }),
  ).resolves.toBeUndefined();

  expect(importSource).toHaveBeenCalledWith('use "sdxl" as Base\n');
  expect(validateWorkflow).toHaveBeenCalledWith(managedWorkflow());
  expect(original).not.toHaveBeenCalled();
  expect(retainWorkflowSource).not.toHaveBeenCalled();
});

test('delegates ordinary files without invoking either SugarScript use case', async () => {
  const original = jest.fn(async () => 'host');
  const host: SugarScriptFileHost = { handleFile: original };
  const importSource = jest.fn(async () => undefined);
  const retainWorkflowSource = jest.fn(async () => undefined);
  createAdapter(host, importSource, retainWorkflowSource).setup();
  const ordinary = {
    name: 'workflow.json',
    type: 'application/json',
    size: 2,
  } as File;

  await host.handleFile(ordinary, 'menu');

  expect(original).toHaveBeenCalledWith(ordinary, 'menu');
  expect(importSource).not.toHaveBeenCalled();
  expect(retainWorkflowSource).not.toHaveBeenCalled();
});

test('falls back to the attached workflow when SugarScript materialization fails', async () => {
  const original = jest.fn(async () => 'host');
  const host: SugarScriptFileHost = { handleFile: original };
  const push = jest.fn();
  const importWorkflow = jest.fn(async () => 'workflow');
  const adapter = new SugarScriptImageFileHostAdapter({
    host,
    importSource: async () => {
      throw new Error('located source diagnostic');
    },
    importWorkflow,
    validateWorkflow: async () => undefined,
    retainWorkflowSource: async () => {
      throw new Error('source diagnostics unavailable');
    },
    feedback: { push },
    readErrorMessage: (error) => String(error),
  });
  adapter.setup();
  const paired = pngFile(
    textChunk('sugar_script', 'invalid source'),
    textChunk('workflow', JSON.stringify(managedWorkflow())),
  );

  await expect(host.handleFile(paired)).resolves.toBe('workflow');

  expect(push).toHaveBeenCalledWith(
    'warning',
    'Workflow imported after SugarScript failed',
    'Error: located source diagnostic',
  );
  expect(original).not.toHaveBeenCalled();
  expect(importWorkflow).toHaveBeenCalledWith(managedWorkflow());
});

test('imports a workflow-only image through exact reconciliation instead of Comfy', async () => {
  const original = jest.fn(async () => 'host');
  const host: SugarScriptFileHost = { handleFile: original };
  const importWorkflow = jest.fn(async () => 'workflow');
  createAdapter(host, async () => undefined, undefined, importWorkflow).setup();

  await expect(
    host.handleFile(pngFile(textChunk('workflow', JSON.stringify(managedWorkflow())))),
  ).resolves.toBe('workflow');

  expect(importWorkflow).toHaveBeenCalledWith(managedWorkflow());
  expect(original).not.toHaveBeenCalled();
});

test('reports source compilation failure without invoking Comfy as a fallback', async () => {
  const original = jest.fn(async () => 'host');
  const host: SugarScriptFileHost = { handleFile: original };
  const importSource = jest.fn(async () => {
    throw new Error('located source diagnostic');
  });
  const push = jest.fn();
  const adapter = new SugarScriptImageFileHostAdapter({
    host,
    importSource,
    importWorkflow: async () => undefined,
    validateWorkflow: async () => undefined,
    retainWorkflowSource: async () => undefined,
    feedback: { push },
    readErrorMessage: (error) => String(error),
  });
  adapter.setup();

  await expect(
    host.handleFile(pngFile(textChunk('sugar_script', 'invalid source'))),
  ).resolves.toBeUndefined();

  expect(original).not.toHaveBeenCalled();
  expect(push).toHaveBeenCalledWith(
    'error',
    'SugarScript image import failed',
    'Error: located source diagnostic',
  );
});

test('delegates an unrelated malformed PNG once', async () => {
  const original = jest.fn(async () => 'host');
  const host: SugarScriptFileHost = { handleFile: original };
  createAdapter(
    host,
    jest.fn(async () => undefined),
  ).setup();
  const malformed = {
    name: 'ordinary.png',
    type: 'image/png',
    size: 9,
    arrayBuffer: async () => new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10, 0]).buffer,
  } as File;

  await expect(host.handleFile(malformed, 'file_drop')).resolves.toBe('host');
  expect(original).toHaveBeenCalledTimes(1);
});

test('does not remove a host wrapper installed after setup', () => {
  const original = jest.fn(async () => 'host');
  const host: SugarScriptFileHost = { handleFile: original };
  const adapter = createAdapter(
    host,
    jest.fn(async () => undefined),
  );
  adapter.setup();
  const laterWrapper = jest.fn(async () => 'later');
  host.handleFile = laterWrapper;

  adapter.dispose();

  expect(host.handleFile).toBe(laterWrapper);
});

function createAdapter(
  host: SugarScriptFileHost,
  importSource: (source: string) => Promise<unknown>,
  retainWorkflowSource: (source: string) => Promise<unknown> = async () => undefined,
  importWorkflow: (workflow: Record<string, unknown>) => Promise<unknown> = async () => undefined,
  validateWorkflow: (workflow: Record<string, unknown>) => Promise<unknown> = async () => undefined,
): SugarScriptImageFileHostAdapter {
  return new SugarScriptImageFileHostAdapter({
    host,
    importSource,
    importWorkflow,
    validateWorkflow,
    retainWorkflowSource,
    readErrorMessage: (error) => String(error),
  });
}

function managedWorkflow(): Record<string, unknown> {
  return {
    groups: [
      {
        sugarcubes: {
          managed: true,
          cube_id: 'Artificial-Sweetener/Base-Cubes/demo.cube',
          cube_version: '1.0.0',
        },
      },
    ],
  };
}

function pngFile(...metadataChunks: Uint8Array[]): File {
  const signature = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]);
  const header = chunk('IHDR', new Uint8Array(13));
  const end = chunk('IEND', new Uint8Array());
  const input = rawInput(concatenate(signature, header, ...metadataChunks, end));
  return {
    name: 'recipe.png',
    type: 'image/png',
    size: input.size,
    arrayBuffer: input.arrayBuffer,
  } as File;
}

function rawInput(bytes: Uint8Array): PngArtifactInput {
  return {
    size: bytes.byteLength,
    arrayBuffer: async () => bytes.slice().buffer,
  };
}

function textChunk(keyword: string, value: string): Uint8Array {
  return chunk('tEXt', concatenate(latin1(keyword), new Uint8Array([0]), latin1(value)));
}

function chunk(type: string, data: Uint8Array): Uint8Array {
  const bytes = new Uint8Array(12 + data.byteLength);
  new DataView(bytes.buffer).setUint32(0, data.byteLength, false);
  bytes.set(latin1(type), 4);
  bytes.set(data, 8);
  return bytes;
}

function latin1(value: string): Uint8Array {
  return Uint8Array.from(value, (character) => character.charCodeAt(0));
}

function concatenate(...parts: Uint8Array[]): Uint8Array {
  const result = new Uint8Array(parts.reduce((length, part) => length + part.byteLength, 0));
  let offset = 0;
  for (const part of parts) {
    result.set(part, offset);
    offset += part.byteLength;
  }
  return result;
}
