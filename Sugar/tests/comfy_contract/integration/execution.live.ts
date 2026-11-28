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
/** Prove real Comfy validation, queue tuple consumption, output attribution and image persistence. */
import { readFile } from 'node:fs/promises';
import { test, expect } from '../support/LiveComfy.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';
import UPNG from 'upng-js';
import type { APIRequestContext } from '@playwright/test';
import type {
  ComfyQueueApi,
  ComfyQueueJobSource,
} from '../../../frontend/comfyui/ui/surface/frame/queue/ComfyQueueJobSource.js';

test('generation queue observes the browser client execution events and restores the job workflow name', async ({
  loaded: page,
}) => {
  const workflow: unknown = JSON.parse(
    await readFile(new URL('../support/solid-image-workflow.json', import.meta.url), 'utf8'),
  );
  const observation = await page.evaluateHandle(async (workflow) => {
    const adapter = '/extensions/SugarCubes/comfyui/ui/surface/frame/queue/ComfyQueueJobSource.js';
    const client = '/scripts/api.js';
    const { ComfyQueueJobSource: Source }: { ComfyQueueJobSource: typeof ComfyQueueJobSource } =
      await import(adapter);
    const { api }: { api: ComfyQueueApi & { clientId: string } } = await import(client);
    if (!api.clientId) throw new Error('ComfyUI did not assign the browser a queue client id');
    const errors: string[] = [];
    const events: { type: string; id: unknown }[] = [];
    const source = new Source(api, (e) => errors.push(String(e)));
    const listener = (event: Event) =>
      events.push({
        type: event.type,
        id: (event as CustomEvent<{ prompt_id: unknown }>).detail.prompt_id,
      });
    api.addEventListener('execution_start', listener);
    api.addEventListener('execution_success', listener);
    let latest = source.snapshot;
    const unsubscribe = source.subscribe((snapshot) => {
      latest = snapshot;
    });
    const response = await api.fetchApi('/sugarcubes/v2/executions/queue', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        schema_version: 1,
        workflow,
        proximity_connections: [],
        queue: { atomic: true, client_id: api.clientId },
        extra_data: { sugarcubes_output_saving: { workflow_name: 'Compatibility queue fixture' } },
      }),
    });
    if (!response.ok) throw new Error(await response.text());
    const receipt: { prompt_id: string } = await response.json();
    return {
      read: () => ({
        events: events.filter((event) => event.id === receipt.prompt_id).map((event) => event.type),
        job: latest.jobs.find((job) => job.id === receipt.prompt_id),
        errors,
      }),
      close: () => {
        unsubscribe();
        api.removeEventListener('execution_start', listener);
        api.removeEventListener('execution_success', listener);
      },
    };
  }, workflow);
  try {
    await expect
      .poll(() => observation.evaluate((observer) => observer.read()))
      .toMatchObject({
        events: ['execution_start', 'execution_success'],
        job: { status: 'completed', workflowName: 'Compatibility queue fixture' },
        errors: [],
      });
  } finally {
    await observation.evaluate((observer) => observer.close());
    await observation.dispose();
  }
});

/** Queue the actual graph and retrieve only the history owned by its receipt. */
async function executedOutputs(
  request: APIRequestContext,
  workflow: unknown,
): Promise<Record<string, unknown>> {
  const enqueued = await request.post('/sugarcubes/v2/executions/queue', {
    data: { schema_version: 1, workflow, proximity_connections: [], queue: { atomic: true } },
  });
  await expect(enqueued).toBeOK();
  const receipt: unknown = await enqueued.json();
  expect(receipt).toMatchObject({ accepted: true, report: { execution_owner: 'sugarcubes' } });
  if (!isRecord(receipt) || typeof receipt.prompt_id !== 'string')
    throw new Error('Missing queued prompt id');
  const id = receipt.prompt_id;
  const history = async (): Promise<unknown> => {
    const response = await request.get('/history/' + encodeURIComponent(id));
    await expect(response).toBeOK();
    const entries: unknown = await response.json();
    return isRecord(entries) ? entries[id] : undefined;
  };
  await expect.poll(history).toMatchObject({
    status: { status_str: 'success', completed: true },
  });
  const entry = await history();
  if (!isRecord(entry) || !isRecord(entry.outputs))
    throw new Error('Queue history lost the Cube output');
  return entry.outputs;
}

/** Decode the actual image file for this specific output and compare every RGBA channel. */
async function expectSwatch(
  request: APIRequestContext,
  node: unknown,
  rgba: readonly number[],
): Promise<void> {
  if (!isRecord(node) || !Array.isArray(node.images))
    throw new Error('Missing native output image list');
  expect(node.images).toHaveLength(1);
  const image: unknown = node.images[0];
  if (!isRecord(image) || typeof image.filename !== 'string' || typeof image.type !== 'string')
    throw new Error('Cube output is missing its Comfy image descriptor');
  const response = await request.get('/view', {
    params: {
      filename: image.filename,
      type: image.type,
      subfolder: typeof image.subfolder === 'string' ? image.subfolder : '',
    },
  });
  await expect(response).toBeOK();
  const bytes = await response.body();
  const png = UPNG.decode(Uint8Array.from(bytes).buffer);
  expect([png.width, png.height]).toEqual([32, 16]);
  const pixels = new Uint8Array(UPNG.toRGBA8(png)[0]!);
  const expected = Uint8Array.from({ length: 32 * 16 * 4 }, (_, channel) => rgba[channel % 4]!);
  expect(pixels).toEqual(expected);
}

test('direct Cube queue produces the authored 32 by 16 red image in Comfy history', async ({
  request,
}) => {
  const workflow: unknown = JSON.parse(
    await readFile(new URL('../support/solid-image-workflow.json', import.meta.url), 'utf8'),
  );
  const outputs = await executedOutputs(request, workflow);
  expect(Object.keys(outputs)).toEqual(['__sugarcubes_cube_output__:1:0']);
  await expectSwatch(request, outputs['__sugarcubes_cube_output__:1:0'], [255, 0, 0, 255]);
});

test('connected Cube outputs preserve link direction, pixel values and separate history identities', async ({
  request,
}) => {
  const source = [
    'use "Artificial-Sweetener/Base-Cubes/Compatibility.cube" as Left',
    'use "Artificial-Sweetener/Base-Cubes/Invert.cube" as Right',
    'connect Left.output.image to Right.input.image',
  ].join('\n');
  const response = await request.post('/sugarcubes/v2/sugarscript/compile', { data: { source } });
  await expect(response).toBeOK();
  const plan: unknown = await response.json();
  if (!isRecord(plan) || !isRecord(plan.workflow))
    throw new Error('No native workflow in compile result');
  expect(plan.workflow.links).toEqual([[1, 1, 0, 2, 0, '*']]);
  const outputs = await executedOutputs(request, plan.workflow);
  expect(Object.keys(outputs).sort()).toEqual([
    '__sugarcubes_cube_output__:1:0',
    '__sugarcubes_cube_output__:2:0',
  ]);
  await expectSwatch(request, outputs['__sugarcubes_cube_output__:1:0'], [255, 0, 0, 255]);
  await expectSwatch(request, outputs['__sugarcubes_cube_output__:2:0'], [0, 255, 255, 255]);
});
