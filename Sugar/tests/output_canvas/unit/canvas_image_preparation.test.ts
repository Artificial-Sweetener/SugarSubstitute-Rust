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
/** Prove bounded navigation preparation preserves the old presentation until atomic adoption. */
import { expect, test } from '@jest/globals';
import { CanvasImagePreparation } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImagePreparation.js';
import { CanvasImageResources } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageResources.js';
import { CanvasImageBudget } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageBudget.js';
import { CanvasImageDecodeScheduler } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageDecodeScheduler.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import {
  canvasImageFixture,
  canvasImageInput,
  settleCanvasImages,
  CanvasImagePortFixture,
} from '../support/CanvasImageFixture.js';

test('a presentation callback failure cannot strand completed source preparation', async () => {
  const clock = new WorkflowTestClock(),
    port = new CanvasImagePortFixture(),
    budget = new CanvasImageBudget();
  const cause = new Error('Presentation failed'),
    errors: unknown[] = [];
  const resources = new CanvasImageResources({
    clock,
    budget,
    decodes: new CanvasImageDecodeScheduler(port, clock),
    changed: () => {
      throw cause;
    },
    logger: {
      error: (_message, context) => {
        errors.push(context.error);
      },
    },
  });
  const preparation = new CanvasImagePreparation(resources);
  preparation.update([{ slot: 'primary', input: canvasImageInput('old') }]);
  port.ready(0);
  await settleCanvasImages();
  const lease = preparation.prepare([{ slot: 'primary', input: canvasImageInput('next') }]);
  port.ready(1);
  await settleCanvasImages();
  expect(await lease!.ready).toBe(true);
  expect(errors).toContain(cause);
  preparation.dispose();
  resources.dispose();
  expect(budget.estimatedBytes).toBe(0);
});

test('cold source switching retains the current frame until the replacement is decoded and committed', async () => {
  const f = canvasImageFixture(),
    preparation = new CanvasImagePreparation(f.resources);
  const old = { slot: 'primary', input: canvasImageInput('old') },
    next = { slot: 'primary', input: canvasImageInput('next') };
  preparation.update([old]);
  f.port.ready(0);
  await settleCanvasImages();
  const lease = preparation.prepare([next]);
  expect(lease).not.toBeNull();
  expect(f.resources.read().find((x) => x.slot === 'primary')?.frame?.key).toBe('old');
  expect(f.port.requests[0]?.released).toBe(false);
  f.port.ready(1);
  await settleCanvasImages();
  expect(await lease!.ready).toBe(true);
  expect(f.resources.read().find((x) => x.slot === 'primary')?.frame?.key).toBe('old');
  expect(
    lease!.commit(() => {
      preparation.update([next]);
      return true;
    }),
  ).toBe(true);
  expect(f.resources.read().map((x) => x.frame?.key)).toEqual(['next']);
  expect(f.port.requests).toHaveLength(2);
  expect(f.port.requests[0]?.released).toBe(true);
  preparation.dispose();
  f.resources.dispose();
  expect(f.budget.estimatedBytes).toBe(0);
});

test('a complete incoming grid is staged before any current cell is replaced', async () => {
  const f = canvasImageFixture(),
    preparation = new CanvasImagePreparation(f.resources);
  const grid = (prefix: string) =>
    [0, 1, 2].map((i) => ({
      slot: `${prefix}:${i}`,
      input: {
        ...canvasImageInput(`${prefix}:${i}`, false, { width: 1080, height: 1512 }),
        role: 'thumbnail' as const,
      },
    }));
  preparation.update(grid('old'));
  f.port.ready(0, { width: 1080, height: 1512 });
  f.port.ready(1, { width: 1080, height: 1512 });
  await settleCanvasImages();
  f.port.ready(2, { width: 1080, height: 1512 });
  await settleCanvasImages();
  const lease = preparation.prepare(grid('next'));
  expect(lease).not.toBeNull();
  f.port.ready(3, { width: 1080, height: 1512 });
  f.port.ready(4, { width: 1080, height: 1512 });
  await settleCanvasImages();
  expect(
    f.resources
      .read()
      .filter((x) => x.slot.startsWith('old:'))
      .every((x) => x.frame),
  ).toBe(true);
  f.port.ready(5, { width: 1080, height: 1512 });
  await settleCanvasImages();
  expect(await lease!.ready).toBe(true);
  lease!.commit(() => {
    preparation.update(grid('next'));
    return true;
  });
  expect(f.resources.read().map((x) => x.frame?.key)).toEqual(['next:0', 'next:1', 'next:2']);
  expect(f.port.requests).toHaveLength(6);
  preparation.dispose();
  f.resources.dispose();
});

test('superseded preparations, decode failure, and hidden surfaces release staged work without clearing the current image', async () => {
  const f = canvasImageFixture(),
    preparation = new CanvasImagePreparation(f.resources);
  preparation.update([{ slot: 'primary', input: canvasImageInput('old') }]);
  f.port.ready(0);
  await settleCanvasImages();
  const first = preparation.prepare([{ slot: 'primary', input: canvasImageInput('first') }]);
  const second = preparation.prepare([{ slot: 'primary', input: canvasImageInput('second') }]);
  expect(await first!.ready).toBe(false);
  expect(first!.commit(() => true)).toBe(false);
  f.port.fail(2);
  await settleCanvasImages();
  expect(await second!.ready).toBe(false);
  expect(f.resources.read().find((x) => x.slot === 'primary')?.frame?.key).toBe('old');
  const third = preparation.prepare([{ slot: 'primary', input: canvasImageInput('third') }]);
  f.resources.active(false);
  expect(await third!.ready).toBe(false);
  expect(f.budget.estimatedBytes).toBe(0);
  preparation.dispose();
  f.resources.dispose();
});

test('preparation honors slot and memory limits while leaving the outgoing presentation owned', async () => {
  const f = canvasImageFixture(),
    preparation = new CanvasImagePreparation(f.resources);
  preparation.update([{ slot: 'primary', input: canvasImageInput('old') }]);
  f.port.ready(0);
  await settleCanvasImages();
  expect(
    preparation.prepare(
      [0, 1].map((i) => ({ slot: `next:${i}`, input: canvasImageInput(`next:${i}`) })),
    ),
  ).toBeNull();
  const lease = preparation.prepare([
    { slot: 'primary', input: canvasImageInput('huge', false, { width: 10000, height: 10000 }) },
  ]);
  expect(await lease!.ready).toBe(false);
  expect(f.resources.read().map((x) => x.frame?.key)).toEqual(['old']);
  expect(f.port.requests).toHaveLength(1);
  preparation.dispose();
  f.resources.dispose();
});
