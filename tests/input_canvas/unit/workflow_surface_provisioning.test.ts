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
/** Prove coalesced surface sync and binding of provisioned mask loaders against a fake canvas. */
import { jest } from '@jest/globals';
import {
  WorkflowSurfaceProvisioning,
  type WorkflowSurfaceGraph,
} from '../../../frontend/comfyui/ui/input_canvas/application/WorkflowSurfaceProvisioning.js';
import type { WorkflowSurfaceFacts } from '../../../frontend/comfyui/ui/input_canvas/domain/WorkflowSurfacePlan.js';
import type { CanvasWorkflowHost } from '../../../integrations/contracts/CanvasWorkflowSurfaces.js';
import type {
  CanvasInputSource,
  CanvasWorkflowCommand,
  CanvasWorkflowResult,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import type { CanvasSurfaceFileReference } from '../../../integrations/contracts/CanvasSurfaceProtocol.generated.js';

const photo = { filename: 'photo.png', subfolder: '', type: 'input' } as const;
const picked: CanvasSurfaceFileReference = { filename: 'mask.png', subfolder: '', type: 'input' };

/** Answer every sync with deterministic documents and layer ids. */
function canvas(): CanvasWorkflowHost & { commands: CanvasWorkflowCommand[] } {
  const commands: CanvasWorkflowCommand[] = [];
  return {
    canvasId: 'native',
    commands,
    execute: jest.fn(async (command: CanvasWorkflowCommand): Promise<CanvasWorkflowResult> => {
      commands.push(command);
      if (command.type !== 'sync') return { kind: 'activated' };
      return {
        kind: 'synced',
        surfaces: command.surfaces.map((surface) => ({
          key: surface.key,
          document: {
            id: 'doc:' + surface.key,
            geometryRevision: 'g1',
            width: 64,
            height: 32,
          },
          masks: surface.masks.map((mask) => ({ key: mask.key, layerId: 'layer:' + mask.key })),
        })),
      };
    }),
  };
}

/** Hold mutable facts and record provisioned sources per endpoint. */
function graph(initial: WorkflowSurfaceFacts) {
  let facts = initial;
  const listeners = new Set<() => void>();
  const provisioned = new Map<string, ReadonlyMap<string | null, CanvasInputSource>>();
  const port: WorkflowSurfaceGraph = {
    workflow: jest.fn(() => 'workflow-a'),
    facts: () => facts,
    binding: (endpoint) => ({ provision: (sources) => provisioned.set(endpoint, sources) }),
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  return {
    port,
    provisioned,
    change(next: WorkflowSurfaceFacts) {
      facts = next;
      for (const listener of listeners) listener();
    },
  };
}

const paired: WorkflowSurfaceFacts = {
  images: [{ key: 'image', label: 'Load Image', file: photo }],
  masks: [
    {
      key: 'mask',
      label: 'Load Mask',
      opacity: 0.5,
      image: 'image',
      authority: { kind: 'missing', reason: 'no_relevant_spatial_root_with_dimensions' },
      regions: null,
      channel: 'alpha',
      entries: [{ entry: null, label: 'Load Mask', initial: null }],
    },
  ],
};

/** Compose the provisioning owner with fake boundaries. */
function provisioning(
  facts: WorkflowSurfaceFacts = paired,
  sizes: Record<string, { width: number; height: number } | null> = {},
) {
  const host = canvas(),
    fake = graph(facts),
    notice = jest.fn<(title: string, message: string) => void>(),
    report = jest.fn<(error: unknown) => void>();
  const service = new WorkflowSurfaceProvisioning(
    fake.port,
    () => host,
    () => '#479ef5',
    { read: async (file) => sizes[file.filename] ?? null },
    notice,
    report,
  );
  return { host, fake, notice, report, service };
}

test('a file image with a paired mask syncs once and binds the mask loader to its layer', async () => {
  const { host, fake, report, service } = provisioning();
  await Promise.all([service.request(), service.request(), service.request()]);
  expect(host.commands).toEqual([
    {
      type: 'sync',
      workflow: 'workflow-a',
      surfaces: [
        {
          key: 'image',
          label: 'Load Image',
          base: { kind: 'file', file: photo },
          masks: [
            {
              key: 'mask',
              label: 'Load Mask',
              color: '#479ef5',
              opacity: 0.5,
              initial: null,
              channel: 'alpha',
            },
          ],
        },
      ],
    },
  ]);
  expect(fake.provisioned.get('mask')).toEqual(
    new Map([
      [
        null,
        {
          kind: 'canvas',
          canvasId: 'native',
          documentId: 'doc:image',
          target: { kind: 'layer', layerId: 'layer:mask' },
        },
      ],
    ]),
  );
  expect(service.state?.surfaces[0]?.masks[0]?.layerId).toBe('layer:mask');
  expect(service.document('native', 'doc:image')).toMatchObject({ width: 64, height: 32 });
  expect(service.document('other', 'doc:image')).toBeNull();
  expect(report).not.toHaveBeenCalled();
});

test('unchanged plans are not resent, changes during a sync coalesce into one follow-up', async () => {
  const { host, fake, service } = provisioning();
  await service.request();
  await service.request();
  expect(host.commands).toHaveLength(1);
  const running = service.request();
  fake.change({ ...paired, masks: [] });
  fake.change({ images: paired.images, masks: [] });
  await running;
  expect(host.commands).toHaveLength(2);
  expect(host.commands[1]).toMatchObject({ surfaces: [{ key: 'image', masks: [] }] });
  await service.refresh();
  expect(host.commands).toHaveLength(3);
});

test('a picked mask file seeds only a matching mask; a mismatch on an existing mask is reported', async () => {
  const withInitial = (initial: CanvasSurfaceFileReference | null): WorkflowSurfaceFacts => ({
    ...paired,
    masks: paired.masks.map((mask) => ({
      ...mask,
      entries: [{ entry: null, label: 'Load Mask', initial }],
    })),
  });
  const silent = provisioning(withInitial(picked), {
    'photo.png': { width: 64, height: 32 },
    'mask.png': { width: 10, height: 10 },
  });
  await silent.service.request();
  expect(silent.host.commands[0]).toMatchObject({
    surfaces: [{ masks: [{ key: 'mask', initial: null }] }],
  });
  expect(silent.notice).not.toHaveBeenCalled();
  silent.fake.change(withInitial({ ...picked, filename: 'other.png' }));
  await silent.service.request();
  expect(silent.notice).toHaveBeenCalledWith(
    'Mask dimensions could not be verified',
    expect.stringContaining('could not be verified'),
  );
  const matching = provisioning(withInitial(picked), {
    'photo.png': { width: 64, height: 32 },
    'mask.png': { width: 64, height: 32 },
  });
  await matching.service.request();
  expect(matching.host.commands[0]).toMatchObject({
    surfaces: [{ masks: [{ initial: picked }] }],
  });
  const mismatch = provisioning(paired, {
    'photo.png': { width: 64, height: 32 },
    'mask.png': { width: 32, height: 32 },
  });
  await mismatch.service.request();
  mismatch.fake.provisioned.clear();
  mismatch.fake.change(withInitial(picked));
  await mismatch.service.request();
  // The rejected file leaves the plan unchanged, yet its loader returns to the mask's layer.
  expect(mismatch.host.commands).toHaveLength(1);
  expect(mismatch.fake.provisioned.get('mask')?.get(null)).toMatchObject({
    target: { kind: 'layer', layerId: 'layer:mask' },
  });
  expect(mismatch.notice).toHaveBeenCalledTimes(1);
  expect(mismatch.notice).toHaveBeenCalledWith(
    'Mask dimensions do not match',
    'The selected mask dimensions (32x32) do not match the loaded input image (64x32).',
  );
});

test('failed syncs are reported, publish nothing and are retried on the next change', async () => {
  const { host, report, service } = provisioning();
  jest
    .mocked(host.execute)
    .mockResolvedValueOnce({ kind: 'failure', status: 'error', message: 'Host refused.' });
  await service.request();
  expect(report).toHaveBeenCalledWith(new Error('Host refused.'));
  expect(service.state).toBeNull();
  await service.request();
  expect(service.state?.surfaces).toHaveLength(1);
  jest.mocked(host.execute).mockResolvedValueOnce({
    kind: 'synced',
    surfaces: [],
  });
  await service.refresh();
  expect(report).toHaveBeenLastCalledWith(new Error('The input canvas could not be updated.'));
});

test('no canvas or workflow means no state and no sync', async () => {
  const fake = graph(paired);
  const listener = jest.fn();
  let host: CanvasWorkflowHost | null = canvas();
  const service = new WorkflowSurfaceProvisioning(
    fake.port,
    () => host,
    () => '#479ef5',
    { read: async () => null },
    jest.fn(),
    jest.fn(),
  );
  service.subscribe(listener);
  await service.request();
  expect(service.state).not.toBeNull();
  host = null;
  await service.request();
  expect(service.state).toBeNull();
  expect(listener).toHaveBeenCalledTimes(2);
  service.dispose();
  await service.request();
  expect(listener).toHaveBeenCalledTimes(2);
});

test('a directly edited size authority resizes its existing mask-only surface before syncing', async () => {
  const sized = (width: number): WorkflowSurfaceFacts => ({
    images: [],
    masks: [
      {
        ...paired.masks[0]!,
        image: null,
        authority: {
          kind: 'resolved',
          key: 'latent',
          width,
          height: 32,
          nodes: [{ node: '4', output: 0, width: 'width', height: 'height' }],
        },
      },
    ],
  });
  const { host, fake, service } = provisioning(sized(64));
  await service.request();
  fake.change(sized(128));
  await service.request();
  expect(host.commands.map((command) => command.type)).toEqual(['sync', 'resize', 'sync']);
  expect(host.commands[1]).toMatchObject({
    width: 128,
    height: 32,
    mode: { kind: 'canvas', anchor: 'center' },
  });
});
