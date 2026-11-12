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
/** Verify bounded script-only recipe-image metadata extraction. */

import { TextDecoder, TextEncoder } from 'node:util';
import {
  PngSugarScriptArtifactError,
  readPngSugarScriptMetadata,
  readScriptOnlySugarScript,
  type PngArtifactInput,
} from '../../../frontend/comfyui/ui/sugarscript/PngSugarScriptArtifactReader.js';

Object.assign(globalThis, { TextDecoder, TextEncoder });

test('reads Substitute-compatible tEXt SugarScript metadata', async () => {
  const source = 'use "Artificial-Sweetener/Base-Cubes/demo.cube" as Demo';
  const image = pngInput(textChunk('sugar_script', latin1(source)));

  await expect(readScriptOnlySugarScript(image)).resolves.toBe(source);
});

test('reads Unicode SugarScript from an uncompressed iTXt record', async () => {
  const source = '# café 日本語\nuse "demo.cube" as Demo';
  const image = pngInput(internationalTextChunk('sugar_script', source));

  await expect(readScriptOnlySugarScript(image)).resolves.toBe(source);
});

test('reports workflow presence without choosing source over workflow authority', async () => {
  const image = pngInput(
    textChunk('workflow', latin1('{"nodes":[]}')),
    textChunk('sugar_script', latin1('use "demo.cube" as Demo')),
  );

  await expect(readPngSugarScriptMetadata(image)).resolves.toEqual({
    sugarScript: 'use "demo.cube" as Demo',
    workflow: { nodes: [] },
  });
  await expect(readScriptOnlySugarScript(image)).rejects.toMatchObject({
    code: 'image.workflow_authority_present',
  });
});

test.each([
  ['missing source', pngInput(), 'image.sugarscript_missing'],
  [
    'duplicate source',
    pngInput(
      textChunk('sugar_script', latin1('first')),
      textChunk('sugar_script', latin1('second')),
    ),
    'image.duplicate_sugarscript',
  ],
  [
    'invalid workflow',
    pngInput(textChunk('workflow', latin1('{not json'))),
    'image.invalid_workflow',
  ],
  [
    'truncated chunk',
    rawInput(new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10, 0])),
    'image.invalid_png',
  ],
] as const)('rejects %s before compilation', async (_name, image, code) => {
  await expect(readScriptOnlySugarScript(image)).rejects.toEqual(expect.objectContaining({ code }));
});

test('rejects an oversized image before reading it', async () => {
  const input: PngArtifactInput = {
    size: 128 * 1024 * 1024 + 1,
    arrayBuffer: async () => {
      throw new Error('must not read');
    },
  };

  await expect(readScriptOnlySugarScript(input)).rejects.toEqual(
    expect.objectContaining({ code: 'image.too_large' }),
  );
});

test('exposes stable typed adapter failures', () => {
  expect(new PngSugarScriptArtifactError('image.test', 'message')).toMatchObject({
    name: 'PngSugarScriptArtifactError',
    code: 'image.test',
    message: 'message',
  });
});

function pngInput(...metadataChunks: Uint8Array[]): PngArtifactInput {
  const signature = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]);
  const header = chunk('IHDR', new Uint8Array(13));
  const end = chunk('IEND', new Uint8Array());
  return rawInput(concatenate(signature, header, ...metadataChunks, end));
}

function rawInput(bytes: Uint8Array): PngArtifactInput {
  return {
    size: bytes.byteLength,
    arrayBuffer: async () => bytes.slice().buffer,
  };
}

function textChunk(keyword: string, value: Uint8Array): Uint8Array {
  return chunk('tEXt', concatenate(latin1(keyword), new Uint8Array([0]), value));
}

function internationalTextChunk(keyword: string, value: string): Uint8Array {
  return chunk(
    'iTXt',
    concatenate(latin1(keyword), new Uint8Array([0, 0, 0, 0, 0]), new TextEncoder().encode(value)),
  );
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
