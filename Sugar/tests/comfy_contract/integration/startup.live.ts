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
/** Verify the effect of SugarCubes' prestartup hook in the running host and native loader menu. */
import { expect, test } from '../support/LiveComfy.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';

test('saved model root redirects the native loader catalog before nodes register', async ({
  request,
}) => {
  const status = await request.get('/sugarcubes/v1/models/root');
  await expect(status).toBeOK();
  const root: unknown = await status.json();
  if (!isRecord(root)) throw new Error('Invalid model-root response');
  expect(root).toMatchObject({ schemaVersion: 1, usesDefault: false, restartRequired: false });
  expect(root.activeModelRoot).toEqual(root.configuredModelRoot);
  expect(root.activeModelRoot).toEqual(expect.stringMatching(/compatibility-models$/));
  const schema = await request.get('/object_info/CheckpointLoaderSimple');
  await expect(schema).toBeOK();
  const info: unknown = await schema.json();
  expect(info).toMatchObject({
    CheckpointLoaderSimple: { input: { required: { ckpt_name: expect.any(Array) } } },
  });
  expect(JSON.stringify(info)).toContain('"compatibility-catalog-only.safetensors"');
});

test('output management recognizes native save and preview nodes and their image inputs', async ({
  request,
}) => {
  const response = await request.get('/sugarcubes/output-saving/capabilities');
  await expect(response).toBeOK();
  expect(await response.json()).toMatchObject({
    success: true,
    data: {
      nodes: expect.arrayContaining([
        { class_type: 'SaveImage', managed: true, kind: 'save', image_input: 'images' },
        { class_type: 'PreviewImage', managed: true, kind: 'preview', image_input: 'images' },
      ]),
    },
  });
  for (const name of ['SaveImage', 'PreviewImage']) {
    const info = await request.get('/object_info/' + name);
    await expect(info).toBeOK();
    const schema: unknown = await info.json();
    expect(schema).toHaveProperty([name, 'output_node'], true);
    expect(schema).toHaveProperty([name, 'input', 'required', 'images', '0'], 'IMAGE');
  }
});
