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
/** Prove real compiled Cube routes replace previews and finals under one exposed stage identity. */

import { jest } from '@jest/globals';
import { workflowResultsFixture } from '../support/workflow_results.js';
import { decodeComfyPromptSources } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptSourceDecoder.js';
import { decodeComfyMedia } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyMediaDecoder.js';
import { buildCubeOutputExecutionId } from '../../../frontend/comfyui/ui/cube/execution/CubeOutputExecutionIdentity.js';
import { outputSourceId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { ComfyWorkflowPreviewDelivery } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowPreviewDelivery.js';
import { ComfyCubeLivePreview } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyCubeLivePreview.js';
import { ComfyWorkflowOutputProjection } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowOutputProjection.js';

test('accepts only bound finals and routes stage previews independently of the visible surface', () => {
  const f = workflowResultsFixture();
  const marker = buildCubeOutputExecutionId(23, 0);
  const sources = decodeComfyPromptSources({
    output: {
      'instance:sampler': { class_type: 'KSampler' },
      'instance:decode': { class_type: 'VAEDecode' },
      [marker]: {
        class_type: 'SugarCubes.CubeOutput',
        inputs: { instance_id: 'instance', value: ['instance:decode', 0] },
      },
    },
  });
  const context = f.ingestor.submitting({ ...f.a, sources });
  f.ingestor.accepted(context, f.id);
  f.ingestor.started(f.id);
  f.ingestor.executing(f.id, ['instance:sampler']);
  const attributed = f.ingestor.resolveSource(f.id, ['instance:sampler']);
  expect(attributed?.source.id).toBe(outputSourceId(marker));
  if (!attributed) throw new Error('Missing attributable stage preview');
  const sessions = new WorkflowSessionStore();
  sessions.synchronize([f.a.session], f.a.session.sessionId);
  const delivery = new ComfyWorkflowPreviewDelivery(f.results, { error: jest.fn() });
  const urls = { create: jest.fn(() => 'blob:owned-stage'), revoke: jest.fn() };
  const previews = new ComfyCubeLivePreview(sessions, f.results, delivery, urls, () => 0, {
    read: () => true,
    subscribe: () => () => undefined,
  });
  const projection = new ComfyWorkflowOutputProjection(previews, sessions, f.results);
  const unsubscribe = projection.subscribe(jest.fn());
  expect(projection.livePreview(marker)).toEqual([]);
  delivery.deliver(attributed, new Blob(['frame']));
  expect(projection.livePreview(marker)).toEqual(['blob:owned-stage']);
  const preview = {
    sessionId: f.a.session.sessionId,
    generation: f.a.session.generation,
    promptId: f.id,
    sourceId: attributed.source.id,
    runOrder: attributed.run.order,
    sequence: 2,
    token: 'frame',
    dimensions: null,
  };
  expect(f.results.publishPreview(preview)).toBe(true);
  const media = decodeComfyMedia({ images: [{ filename: 'stage.png', type: 'temp' }] })!;
  expect(
    f.ingestor.final({
      promptId: f.id,
      executionPaths: ['instance:sampler'],
      ...media,
      merge: false,
    }),
  ).toBe(false);
  expect(f.results.read(f.a.session.sessionId)?.preview?.sourceId).toBe(outputSourceId(marker));
  expect(
    f.ingestor.final({
      promptId: f.id,
      executionPaths: ['instance:decode'],
      ...media,
      merge: false,
    }),
  ).toBe(true);
  const final = f.results.read(f.a.session.sessionId);
  expect(final?.preview).toBeNull();
  expect(final?.outputs).toHaveLength(1);
  expect(final?.outputs[0]?.source.id).toBe(outputSourceId(marker));
  expect(projection.output(marker)?.items[0]?.file.filename).toBe('stage.png');
  expect(projection.livePreview(marker)).toEqual([]);
  expect(urls.revoke).toHaveBeenCalledWith('blob:owned-stage');
  expect(f.results.publishPreview({ ...preview, sequence: 3 })).toBe(false);
  const persisted = decodeComfyMedia({ images: [{ filename: 'persisted.png', type: 'output' }] })!;
  expect(
    f.ingestor.final({ promptId: f.id, executionPaths: [marker], ...persisted, merge: false }),
  ).toBe(true);
  expect(f.results.read(f.a.session.sessionId)?.outputs).toHaveLength(1);
  expect(f.results.read(f.a.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe(
    'persisted.png',
  );
  f.results.synchronize([]);
  expect(f.ingestor.resolveSource(f.id, ['instance:sampler'])).toBeNull();
  unsubscribe();
  previews.dispose();
  delivery.dispose();
});
