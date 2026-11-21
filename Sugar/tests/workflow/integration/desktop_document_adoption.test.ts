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
/** Prove a restored tab shows the newer of the checkpoint's document and the one Comfy reopened. */
import { expect, test } from '@jest/globals';
import { desktopRuntimeFixture } from '../support/desktop_runtime.js';
import { DesktopSessionCancellation } from '../../../frontend/comfyui/ui/workflow/domain/DesktopSession.js';

const older = { nodes: [{ id: 7, type: 'SaveImage', title: 'Before the last edits' }], extra: {} };

/** Reopen every checkpoint tab as Comfy does after a page that died: with an older document. */
async function reopenedOlder(activeIndex = 0) {
  const f = await desktopRuntimeFixture();
  const tabs = await f.host.reopen(
    f.checkpoint.entries.map((entry) => ({
      ...entry.document,
      graphJson: JSON.stringify(older),
      baselineGraphJson: null,
    })),
    activeIndex,
  );
  f.host.committed();
  const saved = f.checkpoint.entries.map((entry) => JSON.parse(entry.document.graphJson) as object);
  const restore = () => f.runtime.materialize(f.checkpoint, new DesktopSessionCancellation());
  return { f, tabs, saved, restore };
}

test('a tab Comfy reopened with an older document takes the document its checkpoint saved', async () => {
  const { f, tabs, saved, restore } = await reopenedOlder();
  const bindings = await restore();
  // The shown tab is loaded with the saved graph; the background tab holds it for its activation.
  expect(f.host.app.rootGraph.serialize()).toEqual(saved[0]);
  expect(tabs[0]?.changeTracker?.activeState).toEqual(saved[0]);
  expect(tabs[1]?.changeTracker?.activeState).toEqual(saved[1]);
  expect(tabs.map((tab) => tab.isModified)).toEqual(
    f.checkpoint.entries.map((entry) => entry.document.dirty),
  );
  // Comfy reopened every tab in checkpoint order: none was opened, closed or moved.
  expect(f.host.workflow.openWorkflows).toEqual(tabs);
  expect(f.host.workflow.activeWorkflow).toBe(tabs[0]);
  // The restore is not an edit: every tab keeps the revision its checkpoint recorded.
  expect(bindings.map((binding) => binding.graphRevision)).toEqual(
    f.checkpoint.entries.map((entry) => entry.document.graphRevision),
  );
  expect(f.errors).toEqual([]);
  f.dispose();
});

test('a draft Comfy kept after the checkpoint was captured stays, and so does a file saved after it', async () => {
  const { f, tabs, restore } = await reopenedOlder();
  const [savedTab, temporaryTab] = tabs;
  if (!savedTab || !temporaryTab) throw new Error('The fixture needs a saved and a temporary tab.');
  f.setDraftTimes((path) => (path === temporaryTab.path ? f.checkpoint.capturedAt + 1 : null));
  savedTab.lastModified = f.checkpoint.capturedAt + 1;
  await restore();
  expect(f.host.app.rootGraph.serialize()).toEqual(older);
  expect(
    temporaryTab.changeTracker?.activeState ?? JSON.parse(temporaryTab.content ?? '{}'),
  ).toEqual(older);
  expect(f.errors).toEqual([]);
  f.dispose();
});

test('a draft older than the checkpoint does not keep its tab, whichever tab Comfy showed', async () => {
  const { f, tabs, saved, restore } = await reopenedOlder(1);
  f.setDraftTimes(() => f.checkpoint.capturedAt - 1);
  await restore();
  // The tab the checkpoint showed is shown again, each tab holding its saved graph.
  const shown = f.checkpoint.entries.findIndex((entry) => entry.id === f.checkpoint.activeEntry);
  expect(shown).toBe(0);
  expect(f.host.app.rootGraph.serialize()).toEqual(saved[0]);
  expect(tabs[0]?.changeTracker?.activeState).toEqual(saved[0]);
  expect(tabs[1]?.changeTracker?.activeState).toEqual(saved[1]);
  expect(f.host.workflow.activeWorkflow).toBe(tabs[0]);
  expect(f.errors).toEqual([]);
  f.dispose();
});

test('when Comfy’s draft times cannot be read, every tab keeps what Comfy reopened', async () => {
  const { f, tabs, restore } = await reopenedOlder();
  f.setDraftTimes(() => 'unreadable');
  await restore();
  expect(f.host.app.rootGraph.serialize()).toEqual(older);
  expect(f.host.workflow.openWorkflows).toEqual(tabs);
  expect(f.errors).toEqual([]);
  f.dispose();
});

test('a tab that already shows what its checkpoint saved is not loaded again', async () => {
  const f = await desktopRuntimeFixture();
  await f.host.reopen(
    f.checkpoint.entries.map((entry) => entry.document),
    0,
  );
  f.host.committed();
  let loads = 0;
  const load = f.host.app.loadGraphData.bind(f.host.app);
  f.host.app.loadGraphData = async (...args: Parameters<typeof load>) => {
    loads += 1;
    await load(...args);
  };
  await f.runtime.materialize(f.checkpoint, new DesktopSessionCancellation());
  expect(loads).toBe(0);
  expect(f.errors).toEqual([]);
  f.dispose();
});
