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
/** Verify desktop capture and relaunch restoration over saved workflows Comfy opens without reading them. */
import { desktopRuntimeFixture } from '../support/desktop_runtime.js';
import { DesktopSessionCancellation } from '../../../frontend/comfyui/ui/workflow/domain/DesktopSession.js';
import { DesktopSessionClient } from '../../../frontend/comfyui/ui/workflow/application/DesktopSessionClient.js';
import { desktopSessionChunks } from '../../../frontend/comfyui/ui/workflow/application/DesktopSessionTransfer.js';
import { isDesktopCheckpoint } from '../../../integrations/shared/SessionSnapshot.js';
import { utf8Bytes } from '../../../integrations/shared/SessionPrimitives.js';
import { createHash } from 'node:crypto';
import type {
  SessionCheckpoint,
  SessionFence,
  SessionMessage,
  SessionOperation,
} from '../../../integrations/contracts/SessionProtocol.generated.js';

type Fixture = Awaited<ReturnType<typeof desktopRuntimeFixture>>;

/** A second launch generates its own identities; reusing the first launch's sequence would fabricate collisions. */
const RELAUNCH_IDENTITIES = 90_000;

const graphs = {
  'workflows/Proof A.json': {
    nodes: [{ id: 7, type: 'SaveImage', title: 'Final A' }],
    extra: { title: 'A' },
  },
  'workflows/Proof B.json': {
    nodes: [{ id: 7, type: 'SaveImage', title: 'Final B' }],
    extra: { title: 'B' },
  },
} as const;
const [pathA, pathB] = Object.keys(graphs) as [keyof typeof graphs, keyof typeof graphs];

/** Let every promise continuation the production owners scheduled finish. */
async function settle(): Promise<void> {
  for (let turn = 0; turn < 5; turn++) await new Promise((resolve) => setTimeout(resolve, 0));
}

/** Register Comfy's saved workflow files for one fixture's workflow directory. */
function saveWorkflows(f: Fixture): void {
  for (const [path, graph] of Object.entries(graphs)) f.host.save(path, graph);
}

/** Accept one generation for the active workflow and deliver its saved final and completed history. */
async function generate(f: Fixture, promptId: string, filename: string): Promise<void> {
  const observed = f.execution.submissionObserver.capture(await f.host.app.graphToPrompt());
  if (!observed) throw new Error('Missing actual queue observation.');
  const output = { images: [{ filename, type: 'output', subfolder: '' }] };
  Object.assign(f.api, {
    fetchApi: async () => ({
      ok: true,
      json: async () => ({
        [promptId]: {
          outputs: { '7': output },
          status: { completed: true, status_str: 'success' },
        },
      }),
    }),
  });
  observed.accepted({ prompt_id: promptId });
  f.api.emit('executed', { prompt_id: promptId, node: '7', output });
  f.api.emit('execution_success', { prompt_id: promptId });
  await settle();
}

/** Describe each saved final by its filename so restored pixels cannot be confused across workflows. */
function describeFinals(f: Fixture): void {
  f.setReceipt(async (file) => ({
    available: true,
    receipt: {
      file: {
        storage: file.type,
        filename: file.filename,
        subfolder: file.subfolder,
        managedArtifact: null,
      },
      digest: (file.filename.startsWith('a-') ? 'a' : 'b').repeat(64),
      bytes: 256,
      width: 512,
      height: 512,
      media: 'image',
      mime: 'image/png',
      metadataEmbedded: false,
      animated: false,
    },
  }));
}

/** Reproduce Comfy tab restoration: both saved tabs open, only the active one read and loaded. */
async function restoreTabs(f: Fixture): Promise<void> {
  saveWorkflows(f);
  const initial = f.host.workflow.activeWorkflow;
  if (!initial) throw new Error('Missing startup workflow fixture.');
  f.host.openInBackground(pathA);
  f.host.openInBackground(pathB);
  await f.host.workflow.closeWorkflow(initial);
  await f.host.open(pathA);
}

test('a background saved tab Comfy has not read is read in place after startup, never reported as an unavailable graph', async () => {
  const f = await desktopRuntimeFixture();
  try {
    await restoreTabs(f);
    const release = f.host.holdLoads();
    f.host.committed();
    await settle();
    expect(f.errors).toEqual([]);
    expect(f.runtime.readiness().committed).toBe(false);
    await expect(
      f.runtime.capture(f.nextId(), 20, new DesktopSessionCancellation()),
    ).rejects.toMatchObject({ code: 'not-committed' });
    release();
    await settle();
    expect(f.errors).toEqual([]);
    expect(f.host.loaded()).toEqual([pathA, pathB]);
    expect(f.host.workflow.activeWorkflow?.path).toBe(pathA);
    expect(f.runtime.readiness().committed).toBe(true);
    const checkpoint = await f.runtime.capture(f.nextId(), 21, new DesktopSessionCancellation());
    const entry = checkpoint.entries.find((candidate) => candidate.document.path === pathB);
    expect(isDesktopCheckpoint(checkpoint)).toBe(true);
    expect(checkpoint.entries.map((candidate) => candidate.document.path)).toEqual([pathA, pathB]);
    expect(entry?.document).toMatchObject({ temporary: false, dirty: false, name: 'Proof B' });
    expect(JSON.parse(entry?.document.graphJson ?? 'null')).toEqual(graphs[pathB]);
  } finally {
    f.dispose();
  }
});

test('opening a saved workflow through the store is not an error and is read exactly once, by Comfy', async () => {
  const f = await desktopRuntimeFixture();
  try {
    f.host.committed();
    saveWorkflows(f);
    const release = f.host.holdLoads();
    const opening = f.host.open(pathA);
    await settle();
    expect(f.errors).toEqual([]);
    expect(f.runtime.readiness().committed).toBe(false);
    release();
    await opening;
    await settle();
    expect(f.errors).toEqual([]);
    expect(f.host.loaded()).toEqual([pathA]);
    expect(f.runtime.readiness().committed).toBe(true);
    const checkpoint = await f.runtime.capture(f.nextId(), 22, new DesktopSessionCancellation());
    expect(checkpoint.entries.map((entry) => entry.document.path)).toEqual([null, pathA]);
    expect(checkpoint.activeEntry).toBe(checkpoint.entries[1]?.id);
  } finally {
    f.dispose();
  }
});

test('a background tab that Comfy is itself reading is never read a second time', async () => {
  const f = await desktopRuntimeFixture();
  try {
    await restoreTabs(f);
    f.host.committed();
    await settle();
    const closing = f.host.workflow.openWorkflows.find((item) => item.path === pathB);
    if (!closing) throw new Error('Missing background tab.');
    await f.host.workflow.closeWorkflow(closing);
    const release = f.host.holdLoads();
    const opening = f.host.open(pathB);
    const active = f.host.workflow.activeWorkflow;
    if (!active) throw new Error('Missing active workflow.');
    await f.host.app.loadGraphData(graphs[pathA], true, true, active);
    await settle();
    release();
    await opening;
    await settle();
    expect(f.errors).toEqual([]);
    expect(f.host.loaded().filter((path) => path === pathB)).toHaveLength(2);
    expect(f.host.loaded()).toEqual([pathA, pathB, pathB]);
  } finally {
    f.dispose();
  }
});

test('a saved tab Comfy cannot read is reported once, keeps capture refused, and recovers when closed', async () => {
  const f = await desktopRuntimeFixture();
  try {
    await restoreTabs(f);
    f.host.failLoad(pathB);
    f.host.committed();
    await settle();
    expect(f.errors).toHaveLength(1);
    expect(String(f.errors[0])).toContain(pathB);
    f.host.notify();
    await settle();
    expect(f.errors).toHaveLength(1);
    expect(f.runtime.readiness().committed).toBe(false);
    await expect(
      f.runtime.capture(f.nextId(), 23, new DesktopSessionCancellation()),
    ).rejects.toMatchObject({ code: 'not-committed' });
    const tab = f.host.workflow.openWorkflows.find((item) => item.path === pathB);
    if (!tab) throw new Error('Missing unreadable tab.');
    await f.host.workflow.closeWorkflow(tab);
    await settle();
    expect(f.runtime.readiness().committed).toBe(true);
    const checkpoint = await f.runtime.capture(f.nextId(), 24, new DesktopSessionCancellation());
    expect(checkpoint.entries.map((entry) => entry.document.path)).toEqual([pathA]);
  } finally {
    f.dispose();
  }
});

/** First launch: open both saved workflows through the store, generate in each, and capture. */
async function captureFirstLaunch(): Promise<{
  readonly checkpoint: SessionCheckpoint;
  readonly saved: SessionCheckpoint['entries'];
}> {
  const first = await desktopRuntimeFixture();
  try {
    first.host.committed();
    saveWorkflows(first);
    describeFinals(first);
    await first.host.open(pathA);
    await generate(first, 'prompt-a', 'a-final.png');
    await first.host.open(pathB);
    await generate(first, 'prompt-b', 'b-final.png');
    await first.host.open(pathA);
    await settle();
    const checkpoint = await first.runtime.capture(
      first.nextId(),
      30,
      new DesktopSessionCancellation(),
    );
    expect(first.errors).toEqual([]);
    const saved = checkpoint.entries.filter((entry) => entry.document.path !== null);
    expect(saved.map((entry) => entry.document.path)).toEqual([pathA, pathB]);
    expect(saved.map((entry) => entry.outputs.map((output) => output.file.filename))).toEqual([
      ['a-final.png'],
      ['b-final.png'],
    ]);
    return { checkpoint, saved };
  } finally {
    first.dispose();
  }
}

test('two saved workflows opened through the store, generated in, captured and relaunched restore their tabs and finals', async () => {
  const { checkpoint, saved } = await captureFirstLaunch();
  const relaunched = await desktopRuntimeFixture(undefined, RELAUNCH_IDENTITIES);
  try {
    describeFinals(relaunched);
    await restoreTabs(relaunched);
    relaunched.host.committed();
    await settle();
    expect(relaunched.errors).toEqual([]);
    expect(relaunched.runtime.readiness().committed).toBe(true);
    const restoredCheckpoint = {
      ...checkpoint,
      entries: saved,
      activeEntry: saved[0]?.id ?? null,
    };
    await relaunched.runtime.materialize(restoredCheckpoint, new DesktopSessionCancellation());
    await settle();
    expect(relaunched.errors).toEqual([]);
    expect(relaunched.host.workflow.openWorkflows.map((item) => item.path)).toEqual([pathA, pathB]);
    expect(relaunched.host.workflow.activeWorkflow?.path).toBe(pathA);
    const recaptured = await relaunched.runtime.capture(
      relaunched.nextId(),
      31,
      new DesktopSessionCancellation(),
    );
    expect(recaptured.entries.map((entry) => entry.id)).toEqual(saved.map((entry) => entry.id));
    expect(recaptured.entries.map((entry) => entry.outputs)).toEqual(
      saved.map((entry) => entry.outputs),
    );
    expect(recaptured.entries.map((entry) => entry.document.path)).toEqual([pathA, pathB]);
    expect(recaptured.activeEntry).toBe(saved[0]?.id);
  } finally {
    relaunched.dispose();
  }
});

/** Hash at the external cryptography port, as the production page does through WebCrypto. */
async function digest(text: string): Promise<string> {
  return createHash('sha256').update(text, 'utf8').digest('hex');
}

/** Whether the page acknowledged the materialization it was asked to perform. */
function isMaterialized(message: SessionMessage): boolean {
  return message.operation.kind === 'acknowledged' && message.operation.phase === 'materialized';
}

test('the restore the host plans on the first committed Ready is admitted over the real protocol while a background tab is read', async () => {
  const { checkpoint, saved } = await captureFirstLaunch();
  const relaunched = await desktopRuntimeFixture(undefined, RELAUNCH_IDENTITIES);
  const sent: SessionMessage[] = [];
  const planned = { ...checkpoint, entries: saved, activeEntry: saved[0]?.id ?? null };
  let restoring = false;
  /** Deliver the checkpoint to the page the way the native host does: begin, chunks, commit, materialize. */
  const restoreFrom = async (fence: SessionFence): Promise<void> => {
    const requestId = relaunched.nextId();
    const transferId = relaunched.nextId();
    const text = JSON.stringify(planned);
    const chunks = desktopSessionChunks(text);
    const message = (operation: SessionOperation): SessionMessage => ({
      protocol: 'sugarcubes.desktop-session',
      version: 1,
      requestId,
      scope: checkpoint.scope,
      origin: relaunched.origin,
      operation,
    });
    await client.receive(
      message({
        kind: 'begin',
        transfer: {
          id: transferId,
          checkpointId: planned.id,
          revision: planned.revision,
          bytes: utf8Bytes(text),
          chunks: chunks.length,
          digest: await digest(text),
          direction: 'materialize',
        },
      }),
    );
    for (const [index, chunk] of chunks.entries())
      await client.receive(message({ kind: 'chunk', transferId, index, text: chunk }));
    await client.receive(message({ kind: 'commit', transferId }));
    await client.receive(message({ kind: 'materialize', transferId, fence }));
  };
  const client: DesktopSessionClient = new DesktopSessionClient({
    transport: {
      scope: checkpoint.scope,
      origin: relaunched.origin,
      webview: relaunched.fence.webview,
      subscribe: () => () => {},
      send: async (message) => {
        sent.push(message);
        const operation = message.operation;
        // The host plans its restore on the first complete Ready and answers with that Ready's fence.
        if (operation.kind === 'ready' && operation.committed && !restoring) {
          restoring = true;
          setTimeout(() => void restoreFrom(operation.fence), 0);
        }
      },
    },
    runtime: relaunched.runtime,
    nextId: relaunched.nextId,
    digest,
    report: (error) => relaunched.errors.push(error),
  });
  try {
    client.install();
    describeFinals(relaunched);
    await restoreTabs(relaunched);
    relaunched.host.committed();
    for (let turn = 0; turn < 20 && !sent.some(isMaterialized); turn++) await settle();
    expect(sent.filter((message) => message.operation.kind === 'error')).toEqual([]);
    expect(sent.some(isMaterialized)).toBe(true);
    expect(relaunched.errors).toEqual([]);
    expect(relaunched.host.workflow.openWorkflows.map((item) => item.path)).toEqual([pathA, pathB]);
    expect(relaunched.host.workflow.activeWorkflow?.path).toBe(pathA);
  } finally {
    client.dispose();
    relaunched.dispose();
  }
});

test('readiness stays closed from the first read until the newly read tab is observed, so a published fence never moves on its own', async () => {
  const f = await desktopRuntimeFixture();
  try {
    await restoreTabs(f);
    const releaseTrackers = f.host.holdTrackers();
    f.host.committed();
    await settle();
    const tab = f.host.workflow.openWorkflows.find((item) => item.path === pathB);
    expect(tab?.content).not.toBeNull();
    expect(tab?.isLoaded).toBe(false);
    expect(f.runtime.readiness().committed).toBe(false);
    releaseTrackers();
    await settle();
    expect(f.runtime.readiness().committed).toBe(true);
    const published = f.runtime.fence();
    expect(published.bindings.every((binding) => binding.graphRevision > 0)).toBe(true);
    await settle();
    expect(f.runtime.fence()).toEqual(published);
    expect(f.errors).toEqual([]);
  } finally {
    f.dispose();
  }
});
