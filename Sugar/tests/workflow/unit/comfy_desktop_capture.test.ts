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
/** @jest-environment node */
/** Keep a checkpoint capture to one moment of its workflow while receipts are awaited. */
import { jest } from '@jest/globals';
import { ComfyDesktopCapture } from '../../../frontend/comfyui/ui/workflow/infrastructure/ComfyDesktopCapture.js';
import {
  DesktopSessionCancellation,
  DesktopSessionRefusal,
} from '../../../frontend/comfyui/ui/workflow/domain/DesktopSession.js';
import type { SessionBinding } from '../../../integrations/contracts/SessionProtocol.generated.js';
import { desktopSessionFixture } from '../support/desktop_session.js';

test('a workflow that advances while its output receipts are awaited makes the capture stale, not invalid', async () => {
  const shared = desktopSessionFixture();
  const entry = shared.checkpoint.entries[0];
  if (!entry) throw new Error('The fixture needs an entry.');
  const context = { sessionId: 'workflow:capture', generation: 0, title: 'Capture' };
  let binding: SessionBinding = {
    entryId: entry.id,
    runtimeSession: context.sessionId,
    generation: 0,
    graphRevision: entry.document.graphRevision,
    sourceRevision: 0,
    navigationRevision: 0,
  };
  const capture = new ComfyDesktopCapture({
    execution: {
      sessions: { read: () => ({ open: [context], active: context, loading: false }) },
      results: { read: () => ({ outputs: [] }) },
    },
    documents: { capture: () => entry.document },
    bindings: { binding: () => binding },
    outputs: {
      capture: async () => {
        // A result arriving now records its sources against the next graph revision.
        binding = { ...binding, graphRevision: binding.graphRevision + 1 };
        return [];
      },
    },
    identities: { inventory: () => [] },
    submissions: { read: () => [] },
    canvas: { capture: () => entry.view },
    presentation: { capture: () => ({ canvas: true, masonry: false, editorWidth: 0 }) },
    input: new Map(),
    assets: new Map(),
    scope: shared.checkpoint.scope,
    versions: async () => ({ frontend: '1', sugarcubes: '1' }),
    now: () => 1,
    previousId: () => null,
    assertCurrent: () => {},
  } as unknown as ConstructorParameters<typeof ComfyDesktopCapture>[0]);
  const refused = capture.capture(shared.checkpoint.id, 5, new DesktopSessionCancellation());
  await expect(refused).rejects.toBeInstanceOf(DesktopSessionRefusal);
  await expect(refused).rejects.toMatchObject({ code: 'stale' });
});

test('a workflow that cannot be captured again keeps its last captured state instead of refusing the save', async () => {
  const shared = desktopSessionFixture();
  const entry = shared.checkpoint.entries[0];
  if (!entry) throw new Error('The fixture needs an entry.');
  const context = { sessionId: 'workflow:kept', generation: 0, title: 'Kept' };
  const binding: SessionBinding = {
    entryId: entry.id,
    runtimeSession: context.sessionId,
    generation: 0,
    graphRevision: entry.document.graphRevision,
    sourceRevision: entry.sourceRevision,
    navigationRevision: entry.navigationRevision,
  };
  let failing = false;
  const warn = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
  const capture = new ComfyDesktopCapture({
    execution: {
      sessions: { read: () => ({ open: [context], active: context, loading: false }) },
      results: { read: () => ({ outputs: [] }) },
    },
    documents: { capture: () => entry.document },
    bindings: { binding: () => binding },
    outputs: {
      capture: async () => {
        if (failing) throw new DesktopSessionRefusal('unavailable', 'A final could not be read.');
        return entry.outputs;
      },
    },
    identities: { inventory: () => entry.sources },
    submissions: { read: () => entry.submissions },
    canvas: { capture: () => entry.view },
    presentation: { capture: () => ({ canvas: true, masonry: false, editorWidth: 0 }) },
    input: new Map(),
    assets: new Map(shared.checkpoint.assets.map((asset) => [asset.digest, asset])),
    scope: shared.checkpoint.scope,
    versions: async () => ({ frontend: '1', sugarcubes: '1' }),
    now: () => 1,
    previousId: () => null,
    assertCurrent: () => {},
  } as unknown as ConstructorParameters<typeof ComfyDesktopCapture>[0]);
  const first = await capture.capture(shared.checkpoint.id, 5, new DesktopSessionCancellation());
  failing = true;
  const second = await capture.capture(shared.checkpoint.id, 6, new DesktopSessionCancellation());
  expect(second.entries).toEqual([first.entries[0]]);
  expect(warn).toHaveBeenCalledTimes(1);
  warn.mockRestore();
});
