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
/** Exercise immutable final import through the actual result and scene owners with no live job resurrection. */
import { desktopSessionFixture } from '../../workflow/support/desktop_session.js';
import { restoreDesktopResults } from '../../../frontend/comfyui/ui/execution/application/WorkflowDesktopResultImport.js';
import { WorkflowDesktopOutputs } from '../../../frontend/comfyui/ui/execution/application/WorkflowDesktopOutputs.js';
import { WorkflowOutputIdentities } from '../../../frontend/comfyui/ui/execution/application/WorkflowOutputIdentities.js';
import { WorkflowSubmissionSnapshots } from '../../../frontend/comfyui/ui/execution/application/WorkflowSubmissionSnapshots.js';
import { WorkflowResultStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowResultStore.js';
import { WorkflowRunRegistry } from '../../../frontend/comfyui/ui/execution/application/WorkflowRunRegistry.js';
import { DesktopSessionCancellation } from '../../../frontend/comfyui/ui/workflow/domain/DesktopSession.js';
import {
  outputSourceId,
  promptId,
  workflowSessionId,
} from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import type {
  DesktopArtifactResult,
  DesktopOutputArtifacts,
} from '../../../frontend/comfyui/ui/execution/domain/DesktopOutputArtifacts.js';
import type { SessionEntry } from '../../../integrations/contracts/SessionProtocol.generated.js';
import type { WorkflowSessionContext } from '../../../frontend/comfyui/ui/execution/domain/WorkflowSession.js';
import { decodeComfyMedia } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyMediaDecoder.js';

/** Admit one portable receipt while allowing actual byte availability to vary per test. */
function fixture(available = true, animated = false) {
  const { checkpoint } = desktopSessionFixture();
  const entry = checkpoint.entries[0];
  if (!entry || !entry.outputs[0]) throw new Error('Missing contract output.');
  let counter = 300;
  const identities = new WorkflowOutputIdentities(
    () => `00000000-0000-4000-8000-${(++counter).toString(16).padStart(12, '0')}`,
  );
  identities.restoreSources(entry.id, entry.sources);
  identities.restoreResults(entry.id, entry.submissions, entry.outputs);
  const submissions = new WorkflowSubmissionSnapshots();
  submissions.restore(entry.id, entry.submissions);
  const artifact = entry.outputs[0];
  const resolved: DesktopArtifactResult = available
    ? {
        available: true,
        receipt: {
          file: artifact.file,
          digest: artifact.digest,
          bytes: artifact.bytes,
          width: artifact.width,
          height: artifact.height,
          media: artifact.media,
          mime: artifact.mime,
          metadataEmbedded: artifact.metadataEmbedded,
          animated,
        },
        url: 'https://host.invalid/retained',
      }
    : { available: false, code: 'changed', message: 'The recorded output changed.' };
  let requests = 0;
  const artifacts: DesktopOutputArtifacts = {
    receipt: async () => {
      requests++;
      return resolved;
    },
    resolve: async () => {
      requests++;
      return resolved;
    },
  };
  const outputs = new WorkflowDesktopOutputs(identities, submissions, artifacts);
  const context: WorkflowSessionContext = {
    sessionId: workflowSessionId('fresh'),
    generation: 4,
    title: 'Restored',
    path: '',
  };
  const binding = {
    entryId: entry.id,
    runtimeSession: context.sessionId,
    generation: context.generation,
    graphRevision: entry.document.graphRevision,
    sourceRevision: entry.sourceRevision,
    navigationRevision: entry.navigationRevision,
  };
  return {
    artifacts,
    checkpoint,
    entry,
    identities,
    submissions,
    outputs,
    context,
    binding,
    requests: () => requests,
  };
}

test('imports exact UUID finals and sparse batch positions with a restore-only publication and no live run', async () => {
  const f = fixture();
  const store = new WorkflowResultStore();
  store.synchronize([f.context]);
  const events: string[] = [];
  store.subscribe((_id, reason) => events.push(reason));
  const imported = await restoreDesktopResults(
    f.artifacts,
    f.entry,
    f.binding,
    f.context,
    f.checkpoint.id,
    new DesktopSessionCancellation(),
  );
  expect(store.restoreSession(f.context, imported)).toBe(true);
  const result = store.read(f.context.sessionId);
  expect(result?.outputs[0]?.items[0]).toMatchObject({
    id: f.entry.outputs[0]?.id,
    batchIndex: 1,
    provenance: { kind: 'desktop-session', sessionId: f.context.sessionId, generation: 4 },
    presentationUrl: 'https://host.invalid/retained',
  });
  expect(result?.outputs[0]?.batchCount).toBe(2);
  expect(result?.runStatus).toBe('idle');
  expect(result?.preview).toBeNull();
  expect(result?.scenes?.running).toBeNull();
  expect(events).toEqual(['restore']);
  const runs = new WorkflowRunRegistry();
  runs.restoreOrder(f.context.sessionId, imported.runOrder);
  expect(runs.outstanding()).toEqual([]);
  expect(runs.reserve({ session: f.context, sources: [] }).order).toBe(imported.runOrder + 1);
  const captured = await f.outputs.capture(
    f.binding,
    f.checkpoint.scope.backend,
    result!,
    new DesktopSessionCancellation(),
  );
  expect(captured).toEqual(f.entry.outputs);
  expect(f.requests()).toBe(1);
});

test('unavailable and changed artifacts retain exact membership and survive recapture without a fabricated replacement receipt', async () => {
  const f = fixture(false);
  const imported = await restoreDesktopResults(
    f.artifacts,
    f.entry,
    f.binding,
    f.context,
    f.checkpoint.id,
    new DesktopSessionCancellation(),
  );
  const item = imported.outputs[0]?.items[0];
  expect(item).toMatchObject({
    id: f.entry.outputs[0]?.id,
    mediaKind: 'unsupported',
    unavailable: { code: 'changed' },
  });
  const store = new WorkflowResultStore();
  store.synchronize([f.context]);
  store.restoreSession(f.context, imported);
  expect(
    await f.outputs.capture(
      f.binding,
      f.checkpoint.scope.backend,
      store.read(f.context.sessionId)!,
      new DesktopSessionCancellation(),
    ),
  ).toEqual(f.entry.outputs);
  expect(f.requests()).toBe(1);
});

test('animation classification follows verified probe facts and does not enter the persisted closed output schema', async () => {
  const f = fixture(true, true);
  const entry: SessionEntry = {
    ...f.entry,
    outputs: f.entry.outputs.map((output) => ({
      ...output,
      mime: 'image/webp',
      file: { ...output.file, filename: 'misleading.png' },
    })),
  };
  const imported = await restoreDesktopResults(
    f.artifacts,
    entry,
    f.binding,
    f.context,
    f.checkpoint.id,
    new DesktopSessionCancellation(),
  );
  expect(imported.outputs[0]?.items[0]?.mediaKind).toBe('animated');
  expect(imported.outputs[0]?.items[0]?.artifact?.output).not.toHaveProperty('animated');
});

test('a newer real run keeps its finals and blocks a delayed restored import', async () => {
  const f = fixture();
  const imported = await restoreDesktopResults(
    f.artifacts,
    f.entry,
    f.binding,
    f.context,
    f.checkpoint.id,
    new DesktopSessionCancellation(),
  );
  const store = new WorkflowResultStore();
  store.synchronize([f.context]);
  const source = {
    id: outputSourceId('7'),
    executionPath: '7',
    label: 'Genuine',
    breadcrumb: [],
    order: 0,
  };
  const run = {
    session: f.context,
    promptId: promptId('live'),
    order: 9,
    status: 'running' as const,
    sources: [source],
  };
  store.updateRun(run);
  const media = decodeComfyMedia({ images: [{ filename: 'live.png' }] });
  if (!media) throw new Error('Missing media.');
  store.retain({ run, source, ...media, merge: false, delivery: 'live' });
  expect(store.restoreSession(f.context, imported)).toBe(false);
  expect(store.read(f.context.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe('live.png');
});

test('initial receipt refusal preserves the visible source final for a later successful capture', async () => {
  const f = fixture(false);
  const store = new WorkflowResultStore();
  store.synchronize([f.context]);
  const source = {
    id: outputSourceId('7'),
    executionPath: 'cube/7',
    label: 'Changed label',
    breadcrumb: [],
    order: 0,
  };
  const run = {
    session: f.context,
    promptId: promptId('accepted-prompt-1'),
    order: 1,
    status: 'running' as const,
    sources: [source],
  };
  const media = decodeComfyMedia({ images: [{ filename: 'final.png' }] });
  if (!media) throw new Error('Missing media.');
  store.retain({ run, source, ...media, merge: false, delivery: 'live' });
  await expect(
    f.outputs.capture(
      f.binding,
      f.checkpoint.scope.backend,
      store.read(f.context.sessionId)!,
      new DesktopSessionCancellation(),
    ),
  ).rejects.toThrow('unavailable');
  expect(store.read(f.context.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe('final.png');
});

test('workflow snapshot source identity retains its observed graph revision and literal label after a reused node changes', async () => {
  const f = fixture();
  const store = new WorkflowResultStore();
  store.synchronize([f.context]);
  const source = {
    id: outputSourceId('7'),
    executionPath: 'cube/7',
    label: 'Original snapshot label',
    breadcrumb: [],
    order: 0,
  };
  const media = decodeComfyMedia({ images: [{ filename: 'snapshot.png' }] });
  if (!media) throw new Error('Missing snapshot media.');
  store.restore(f.context, source, media.payload, 'host-snapshot');
  const result = store.read(f.context.sessionId);
  if (!result) throw new Error('Missing snapshot result.');
  const original = { ...f.binding, graphRevision: 7 };
  f.outputs.retain(original, result, new Set());
  const expected = f.identities
    .inventory(original.entryId)
    .find((source) => source.graphRevision === 7);
  const edited = { ...original, graphRevision: 8 };
  f.identities.sources(edited, [{ ...source, label: 'Reused node title' }]);
  f.outputs.retain(edited, result, new Set());
  const captured = await f.outputs.capture(
    edited,
    f.checkpoint.scope.backend,
    result,
    new DesktopSessionCancellation(),
  );
  expect(captured[0]?.sourceId).toBe(expected?.id);
  expect(
    f.identities.inventory(edited.entryId).find((source) => source.id === captured[0]?.sourceId),
  ).toMatchObject({ graphRevision: 7, label: 'Original snapshot label' });
});
