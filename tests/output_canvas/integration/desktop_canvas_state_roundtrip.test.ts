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
/** Preserve exact navigation, comparison and separate camera lanes through actual production restore owners. */
import { desktopRuntimeFixture } from '../../workflow/support/desktop_runtime.js';
import { DesktopSessionCancellation } from '../../../frontend/comfyui/ui/workflow/domain/DesktopSession.js';
import type {
  SessionEntry,
  SessionOutput,
  SessionSelection,
} from '../../../integrations/contracts/SessionProtocol.generated.js';

/**
 * Comfy reopens a checkpoint's tabs from its own persistence before startup commits; desktop
 * restore then binds each entry to the tab Comfy reopened rather than opening tabs itself.
 */
async function reopenCheckpointTabs(
  f: Awaited<ReturnType<typeof desktopRuntimeFixture>>,
  entries: readonly SessionEntry[],
  active: number,
): Promise<void> {
  await f.host.reopen(
    entries.map((entry) => entry.document),
    active,
  );
  f.host.committed();
}

test('roundtrips scene/source/batch navigation and linked shared inspection without replacing independent cameras', async () => {
  const f = await desktopRuntimeFixture();
  const base = f.checkpoint.entries[0];
  const original = base?.outputs[0];
  const source = base?.sources[0];
  const submission = base?.submissions[0];
  if (!base || !original || !source || !submission || !original.scene)
    throw new Error('Missing scene contract fixture.');
  const scene = { ...original.scene, sceneId: f.nextId(), order: 1, title: 'Second scene' };
  const second: SessionOutput = {
    ...original,
    id: f.nextId(),
    promptId: 'second-prompt',
    runId: f.nextId(),
    runOrder: 2,
    scene,
    file: { ...original.file, filename: 'second.png' },
  };
  const otherSource = {
    ...source,
    id: f.nextId(),
    nodeId: '8',
    executionPath: '8',
    label: 'Detail',
  };
  const other: SessionOutput = {
    ...second,
    id: f.nextId(),
    sourceId: otherSource.id,
    file: { ...original.file, filename: 'detail.png' },
  };
  const selection = (output: SessionOutput): SessionSelection => ({
    outputId: output.id,
    sourceId: output.sourceId,
    sceneId: output.scene?.sceneId ?? null,
    batchIndex: output.batchIndex,
    listIndex: output.listIndex,
  });
  const shared = {
    mode: 'custom' as const,
    centerX: -3.25,
    centerY: 2.75,
    spanX: 0.0005,
    spanY: 0.001,
  };
  const cameras = [
    {
      outputId: original.id,
      inspection: { mode: 'custom' as const, centerX: -2, centerY: 3, spanX: 0.007, spanY: 0.008 },
    },
    {
      outputId: second.id,
      inspection: {
        mode: 'native' as const,
        centerX: 0.2,
        centerY: 1.2,
        spanX: 0.002,
        spanY: 0.003,
      },
    },
    {
      outputId: other.id,
      inspection: {
        mode: 'custom' as const,
        centerX: 1.6,
        centerY: -0.5,
        spanX: 0.004,
        spanY: 0.005,
      },
    },
  ];
  const entry: SessionEntry = {
    ...base,
    document: {
      ...base.document,
      graphJson: JSON.stringify({
        nodes: [
          { id: 7, type: 'SaveImage' },
          { id: 8, type: 'SaveImage' },
        ],
        extra: {},
      }),
    },
    sources: [source, otherSource],
    outputs: [original, second, other],
    submissions: [
      submission,
      {
        ...submission,
        promptId: 'second-prompt',
        runId: second.runId,
        runOrder: 2,
        scene,
        sources: [source, otherSource],
      },
    ],
    view: {
      ...base.view,
      linked: true,
      selection: selection(second),
      comparison: {
        enabled: true,
        selection: selection(other),
        orientation: 'horizontal',
        split: 0.31,
      },
      sharedInspection: shared,
      cameras,
      navigation: {
        sceneOverview: false,
        sceneId: scene.sceneId,
        sourceId: source.id,
        batchIndex: 1,
        batchOverview: false,
      },
    },
  };
  const checkpoint = { ...f.checkpoint, entries: [entry], activeEntry: entry.id };
  await reopenCheckpointTabs(f, [entry], 0);
  await f.runtime.materialize(checkpoint, new DesktopSessionCancellation());
  const actual = await f.runtime.capture(f.nextId(), 99, new DesktopSessionCancellation());
  const captured = actual.entries[0];
  expect(captured?.view).toEqual(entry.view);
  expect(captured?.document.sourceIdentity).toBe(entry.document.sourceIdentity);
  expect(captured?.outputs.map((output) => output.id).sort()).toEqual(
    entry.outputs.map((output) => output.id).sort(),
  );
  expect(f.runtime.fence().bindings[0]).toMatchObject({
    entryId: entry.id,
    graphRevision: entry.document.graphRevision,
    sourceRevision: entry.sourceRevision,
    navigationRevision: entry.navigationRevision,
  });
  const context = f.execution.sessions.read().active;
  const view = context ? f.coordinator.primary(context.sessionId) : null;
  if (!context || !view) throw new Error('Missing restored view.');
  const slot = {
    sourceId: 'source:7' as const,
    batchIndex: 1,
    scene: [scene.groupId, scene.sceneId] as const,
  };
  expect(f.inspections.inspection(view, slot)?.region.centerX).toBe(shared.centerX);
  f.inspections.setLinked(view, false, null);
  expect(f.inspections.inspection(view, slot)?.mode).toBe('native');
  expect(f.inspections.inspection(view, slot)?.region.centerY).toBe(1.2);
  const firstSlot = { ...slot, scene: [original.scene.groupId, original.scene.sceneId] as const };
  expect(f.inspections.inspection(view, firstSlot)?.region.centerX).toBe(-2);
  const catalog = f.coordinator.catalog(view);
  if (!catalog) throw new Error('Missing camera scene catalog.');
  f.selection.apply(view, catalog, { type: 'select-scene', scene: original.scene.sceneId });
  const switched = await f.runtime.capture(f.nextId(), 100, new DesktopSessionCancellation());
  expect(switched.entries[0]?.view.cameras).toEqual(cameras);
  expect(switched.entries[0]?.view.navigation.sceneId).toBe(original.scene.sceneId);
  f.dispose();
  expect(f.errors).toEqual([]);
});

test.each(['scene', 'batch'] as const)(
  'roundtrips the %s overview independently from follow and source-preview intent',
  async (level) => {
    const f = await desktopRuntimeFixture();
    const base = f.checkpoint.entries[0];
    if (!base) throw new Error('Missing entry.');
    const accepted = base.submissions[0];
    if (!accepted?.scene) throw new Error('Missing scene submission.');
    const pending = {
      ...accepted,
      promptId: 'pending-second-scene',
      runId: f.nextId(),
      runOrder: 2,
      scene: { ...accepted.scene, sceneId: f.nextId(), order: 1, title: 'Pending second scene' },
    };
    const entry: SessionEntry = {
      ...base,
      submissions: [...base.submissions, pending],
      view: {
        ...base.view,
        follow: 'auto',
        sourcePreview: true,
        mode: 'overview',
        navigation: {
          ...base.view.navigation,
          sceneOverview: level === 'scene',
          sceneId: level === 'scene' ? null : base.view.navigation.sceneId,
          batchOverview: level === 'batch',
        },
      },
    };
    await reopenCheckpointTabs(f, [entry], 0);
    await f.runtime.materialize(
      { ...f.checkpoint, entries: [entry], activeEntry: entry.id },
      new DesktopSessionCancellation(),
    );
    const captured = (await f.runtime.capture(f.nextId(), 100, new DesktopSessionCancellation()))
      .entries[0];
    expect(captured?.view.follow).toBe('auto');
    expect(captured?.view.sourcePreview).toBe(true);
    expect(captured?.view.mode).toBe('overview');
    if (level === 'batch') expect(captured?.view.navigation.batchOverview).toBe(true);
    else expect(captured?.view.navigation.sceneOverview).toBe(true);
    f.dispose();
  },
);

test('an inactive restored auto view keeps its current image intent when Comfy later activates that occurrence', async () => {
  const f = await desktopRuntimeFixture();
  const base = f.checkpoint.entries[0];
  if (!base) throw new Error('Missing inactive workflow fixture.');
  const a: SessionEntry = {
    ...base,
    document: {
      ...base.document,
      name: 'Inactive A',
      graphJson: JSON.stringify({ nodes: [{ id: 7, type: 'SaveImage' }], extra: {} }),
    },
    outputs: base.outputs.map((output) => ({ ...output, scene: null })),
    submissions: base.submissions.map((submission) => ({ ...submission, scene: null })),
    view: {
      ...base.view,
      follow: 'auto',
      sourcePreview: true,
      mode: 'image',
      navigation: {
        ...base.view.navigation,
        sceneId: null,
        sceneOverview: false,
        batchOverview: false,
      },
      selection: base.view.selection ? { ...base.view.selection, sceneId: null } : null,
    },
  };
  const b: SessionEntry = {
    ...base,
    id: f.nextId(),
    document: {
      ...base.document,
      name: 'Active B',
      graphJson: '{"nodes":[],"extra":{}}',
      temporary: true,
      path: null,
      sourceIdentity: null,
      graphRevision: 0,
    },
    input: null,
    sourceRevision: 0,
    navigationRevision: 0,
    sources: [],
    outputs: [],
    submissions: [],
    view: {
      ...base.view,
      follow: 'auto',
      selection: null,
      cameras: [],
      comparison: { enabled: false, selection: null, orientation: 'vertical', split: 0.5 },
      navigation: {
        sceneOverview: false,
        sceneId: null,
        sourceId: null,
        batchIndex: 0,
        batchOverview: false,
      },
    },
  };
  await reopenCheckpointTabs(f, [a, b], 1);
  await f.runtime.materialize(
    { ...f.checkpoint, entries: [a, b], activeEntry: b.id },
    new DesktopSessionCancellation(),
  );
  const binding = f.runtime.fence().bindings.find((binding) => binding.entryId === a.id);
  const context = f.execution.sessions
    .read()
    .open.find((context) => context.sessionId === binding?.runtimeSession);
  const target = f.host.workflow.openWorkflows.find(
    (workflow) => workflow.filename === 'Inactive A',
  );
  const view = context ? f.coordinator.primary(context.sessionId) : null;
  if (!target || !context || !view) throw new Error('Inactive restored occurrence was not bound.');
  expect(f.views.read(view).mode).toBe('image');
  // Comfy reads an inactive reopened tab's file only when the user activates it.
  await f.host.open(target.path);
  expect(f.views.read(view).mode).toBe('image');
  expect(f.views.read(view).follow).toBe('auto');
  expect(f.views.read(view).selection?.batchIndex).toBe(1);
  expect(f.execution.sessions.read().active?.sessionId).toBe(context.sessionId);
  f.dispose();
  expect(f.errors).toEqual([]);
});
