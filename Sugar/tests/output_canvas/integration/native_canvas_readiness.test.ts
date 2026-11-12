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
/** Prove native readiness is event-driven: a slow host delays the canvas, only a real failure reports one. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { CanvasPrimarySurface } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasPrimarySurface.js';
import { NativeCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { CanvasSurfaceGeometryObserver } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasSurfaceGeometryObserver.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import type { CanvasSessionCoordinator } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSessionCoordinator.js';
import {
  windowId,
  type CanvasViewId,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import {
  nativePreviewHost,
  previewGeometry,
  finalViewport,
  settleNativePreview as settle,
} from '../support/NativeCanvasPreviewFixture.js';
import type { CanvasSurfaceCommand } from '../../../integrations/contracts/CanvasSurface.js';

const disposals: (() => void)[] = [];
afterEach(() => {
  for (const dispose of disposals.splice(0)) dispose();
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

type CommandType = CanvasSurfaceCommand['type'];

/** Compose the real primary surface and attachments over a fake native host and a fake clock. */
function fixture(options: { host?: boolean; discovering?: boolean } = {}) {
  jest.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: 500,
    bottom: 400,
    width: 500,
    height: 400,
    toJSON: () => ({}),
  });
  // jsdom cannot measure visibility; keep the observed allocation drawable.
  jest.spyOn(CanvasSurfaceGeometryObserver.prototype, 'read').mockReturnValue(previewGeometry);
  const catalog = canvasCatalogFixture();
  let sequence = 0;
  const views = new CanvasViewRegistry(() => String(++sequence));
  const first = views.create(catalog.sessionId).viewId;
  const second = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 1 });
  selection.reconcile(first, catalog);
  selection.reconcile(second, catalog);
  let active: CanvasViewId = first;
  let renderer = 'native';
  const hostState = { present: options.host ?? true, discovering: options.discovering ?? false };
  const coordinator = {
    active: () => active,
    catalog: () => catalog,
    subscribe: () => () => undefined,
  } as unknown as CanvasSessionCoordinator;
  const element = document.createElement('div');
  document.body.append(element);
  const clock = new WorkflowTestClock();
  const native = nativePreviewHost(false);
  const errors: unknown[] = [];
  const attachments: NativeCanvasAttachment[] = [];
  const primary = new CanvasPrimarySurface({
    document,
    window: windowId('native-readiness'),
    views,
    coordinator,
    element: () => element,
    renderer: () => renderer,
    report: (error) => errors.push(error),
    mount: (root, address, _catalog, accepts, acceptsPreparation) => {
      const attachment = new NativeCanvasAttachment(root, {
        address,
        clock,
        views,
        selection,
        frames: new CanvasFrameFixture(),
        inspections: new CanvasInspectionStore(),
        catalog: () => catalog,
        accepts,
        acceptsPreparation,
        nativeSurface: () => (hostState.present ? native.host : null),
        surfacePending: () => hostState.discovering,
        connected: () => true,
        report: (error) => errors.push(error),
      });
      attachments.push(attachment);
      return attachment;
    },
  });
  disposals.push(() => primary.dispose());
  const held = new Set<CommandType>();
  return {
    ...native,
    primary,
    attachments,
    clock,
    errors,
    element,
    held,
    hostState,
    /** Acknowledge every ticket except the operation types the test holds back as a slow host would. */
    async drain(): Promise<void> {
      for (let guard = 0; guard < 48; guard++) {
        await settle();
        const index = native.pending.findIndex(
          (record, position) => !record.settled && !held.has(native.calls[position]!.type),
        );
        if (index < 0) return;
        const call = native.calls[index]!;
        native.respond(
          index,
          call.type === 'surface.prepare' ? 'ready' : 'applied',
          call.type === 'surface.viewport.request' ? finalViewport : undefined,
        );
      }
      throw new Error('Native operations did not converge.');
    },
    /** Let a long stretch of wall-clock time pass while the host stays as slow as it was. */
    async waitMinutes(minutes: number): Promise<void> {
      for (let minute = 0; minute < minutes; minute++) {
        clock.advance(60_000);
        await settle();
      }
    },
    prepares: () => native.calls.filter(({ type }) => type === 'surface.prepare'),
    unsettled: (type: CommandType) =>
      native.calls
        .map((call, index) => ({ call, index }))
        .filter(({ call, index }) => call.type === type && !native.pending[index]!.settled),
    /** The most recent root element, which stays hidden while a replacement is only staged. */
    stagedRoot: () => element.lastElementChild as HTMLElement,
    committedCommands: () => native.calls.filter(({ type }) => type === 'surface.commit'),
    /**
     * Answer each preparation the attachment asks for with a failure until it shows that failure;
     * an answer to a superseded request is ignored, so the next request is answered in turn.
     */
    async answerPreparation(
      failure: 'unavailable' | 'error' | Error,
      attachment: NativeCanvasAttachment,
    ): Promise<void> {
      for (let turn = 0; turn < 4; turn++) {
        const asked = native.calls
          .map((call, index) => ({ call, index }))
          .filter(
            ({ call, index }) => call.type === 'surface.prepare' && !native.pending[index]!.settled,
          );
        if (asked.length === 0) break;
        const { index } = asked.at(-1)!;
        if (typeof failure === 'string') native.respond(index, failure);
        else native.fail(index, failure);
        await settle();
        clock.advance(1000);
        await settle();
        if (typeof failure === 'string' && attachment.port.read().resource.state === failure) break;
        if (typeof failure !== 'string' && errors.length > 0) break;
      }
    },
    switchTo(view: CanvasViewId): void {
      active = view;
      primary.reconcile();
    },
    discover(next: string): void {
      renderer = next;
      primary.reconcile();
    },
    first,
    second,
    select: (item: (typeof catalog.entries)[number]['items'][number]) =>
      selection.apply(first, catalog, {
        type: 'select-source',
        selection: { sourceId: item.provenance.sourceId, batchIndex: item.batchIndex },
      }),
    other: () => {
      const selected = views.read(first).selection;
      return catalog.entries.find((entry) => entry.source.id !== selected?.sourceId)!.items[0]!;
    },
  };
}

test.each<CommandType>(['surface.prepare', 'surface.select'])(
  'a host that is slow to answer %s delays the canvas without an error and then attaches',
  async (slow) => {
    const f = fixture();
    f.held.add(slow);
    f.primary.setup();
    await f.drain();
    const [candidate] = f.attachments;
    expect(f.unsettled(slow)).toHaveLength(1);
    // Hours of wall-clock time cannot turn a live, slow host into a failed one.
    await f.waitMinutes(180);
    await f.drain();
    expect(f.errors).toEqual([]);
    expect(f.prepares()).toHaveLength(1);
    // Nothing else is shown, so the attachment is committed at once and its loading state shows.
    expect(f.primary.diagnostics().address).toEqual(candidate!.port.read().address);
    expect(candidate!.port.read().resource.state).toBe('loading');
    expect(f.stagedRoot().style.visibility).toBe('');
    expect(f.committedCommands()).toHaveLength(0);
    expect(candidate!.viewport.classList.contains('sc-output-native')).toBe(false);
    f.held.delete(slow);
    await f.drain();
    expect(f.primary.diagnostics().address).toEqual(candidate!.port.read().address);
    expect(candidate!.port.read().resource.state).toBe('ready');
    expect(candidate!.viewport.classList.contains('sc-output-native')).toBe(true);
    expect(f.committedCommands()).toHaveLength(1);
    expect(f.errors).toEqual([]);
    expect(f.prepares()).toHaveLength(1);
    expect(f.calls.map(({ type }) => type)).not.toContain('surface.release');
  },
);

test('a slow replacement keeps the committed attachment shown and never reports an error', async () => {
  const f = fixture();
  f.primary.setup();
  await f.drain();
  const [committed] = f.attachments;
  expect(f.primary.diagnostics().address).toEqual(committed!.port.read().address);
  // Discovery replaces the renderer; the staged replacement waits on a busy host.
  f.held.add('surface.prepare');
  f.discover('native:discovered');
  await f.drain();
  await f.waitMinutes(60);
  await f.drain();
  expect(f.errors).toEqual([]);
  expect(f.prepares()).toHaveLength(2);
  expect(f.primary.diagnostics().address).toEqual(committed!.port.read().address);
  expect(f.stagedRoot().style.visibility).toBe('hidden');
  f.held.delete('surface.prepare');
  await f.drain();
  const replacement = f.attachments[1]!;
  expect(f.primary.diagnostics().address).toEqual(replacement.port.read().address);
  expect(replacement.port.read().resource.state).toBe('ready');
  expect(f.errors).toEqual([]);
});

test.each<['unavailable' | 'error', string]>([
  ['unavailable', 'The native canvas is unavailable.'],
  ['error', 'Native canvas presentation failed.'],
])(
  'a host that answers %s fails the preparation, and no retry follows however long it waits',
  async (status, message) => {
    const f = fixture();
    f.primary.setup();
    await settle();
    const [attachment] = f.attachments;
    // The attachment commits while its first preparation is in flight, which supersedes that
    // request; the host's answer to the superseded one is ignored and the current one is asked.
    await f.answerPreparation(status, attachment!);
    expect(attachment!.port.read().resource).toMatchObject({ state: status, message });
    expect(f.errors.length).toBeGreaterThan(0);
    expect(f.errors.map(String).join('\n')).not.toContain('timed out');
    const attempts = f.prepares().length;
    const reported = f.errors.length;
    await f.waitMinutes(60);
    expect(f.prepares()).toHaveLength(attempts);
    expect(f.errors).toHaveLength(reported);
  },
);

test('a transport that fails the preparation reports that failure and does not retry', async () => {
  const f = fixture();
  f.primary.setup();
  await settle();
  const [attachment] = f.attachments;
  await f.answerPreparation(new Error('Canvas connection closed.'), attachment!);
  expect(f.errors.map(String).join('\n')).toContain('Canvas connection closed.');
  const attempts = f.prepares().length;
  const reported = f.errors.length;
  await f.waitMinutes(60);
  expect(f.prepares()).toHaveLength(attempts);
  expect(f.errors).toHaveLength(reported);
});

test('readiness arriving after a tab switch is ignored and never reaches the new workflow', async () => {
  const f = fixture();
  f.held.add('surface.prepare');
  f.primary.setup();
  await f.drain();
  const [stale] = f.attachments;
  const [pendingPrepare] = f.unsettled('surface.prepare');
  // The user switches workflows while the first workflow's preparation is still in flight.
  f.switchTo(f.second);
  await f.drain();
  const release = f.calls.findIndex(
    (call) => call.type === 'surface.release' && call.address.viewId === f.first,
  );
  expect(release).toBeGreaterThanOrEqual(0);
  // The host answers the abandoned preparation late; the second workflow waits for its release.
  f.respond(pendingPrepare!.index, 'ready');
  await settle();
  f.respond(release, 'applied');
  f.held.delete('surface.prepare');
  await f.drain();
  const calls = f.calls.filter((call) => call.address.viewId === f.first).map(({ type }) => type);
  expect(calls).toEqual(['surface.prepare', 'surface.release']);
  expect(f.prepares().map((call) => call.address.viewId)).toEqual([f.first, f.second]);
  const fresh = f.attachments[1]!;
  expect(f.primary.diagnostics().address).toEqual(fresh.port.read().address);
  expect(fresh.port.read().address.viewId).toBe(f.second);
  expect(fresh.port.read().resource.state).toBe('ready');
  expect(stale!.port.read().resource.state).not.toBe('ready');
  expect(f.errors).toEqual([]);
});

test('an outdated selection acknowledgement does not make the attachment ready', async () => {
  const f = fixture();
  f.held.add('surface.select');
  f.primary.setup();
  await f.drain();
  const [attachment] = f.attachments;
  const [outdated] = f.unsettled('surface.select');
  // The selection changes while the host is still presenting the previous one.
  const next = f.other();
  f.select(next);
  await settle();
  // The host acknowledges the outdated revision as applied: it must not complete readiness.
  f.respond(outdated!.index, 'applied');
  await settle();
  expect(f.committedCommands()).toHaveLength(0);
  expect(attachment!.port.read().resource.state).toBe('loading');
  expect(f.errors).toEqual([]);
  f.held.delete('surface.select');
  await f.drain();
  const selects = f.calls.filter(({ type }) => type === 'surface.select');
  expect(selects.length).toBeGreaterThanOrEqual(2);
  expect(selects.at(-1)).toMatchObject({ itemId: next.id });
  expect(f.committedCommands()).toHaveLength(1);
  expect(attachment!.port.read().resource.state).toBe('ready');
  expect(f.errors).toEqual([]);
});

test('a canvas mounted before the native host exists loads silently and attaches when it appears', async () => {
  const f = fixture({ host: false, discovering: true });
  f.primary.setup();
  await f.drain();
  await f.waitMinutes(30);
  expect(f.calls).toEqual([]);
  expect(f.errors).toEqual([]);
  expect(f.attachments[0]!.port.read().resource.state).toBe('loading');
  // Discovery concludes with a native surface; the renderer identity changes with it.
  f.hostState.present = true;
  f.hostState.discovering = false;
  f.discover('native:discovered');
  await f.drain();
  expect(f.errors).toEqual([]);
  expect(f.prepares()).toHaveLength(1);
  const attached = f.attachments.at(-1)!;
  expect(f.primary.diagnostics().address).toEqual(attached.port.read().address);
  expect(attached.port.read().resource.state).toBe('ready');
  expect(attached.viewport.classList.contains('sc-output-native')).toBe(true);
});

test('a canvas whose discovery concluded without a native host reports it is unavailable', async () => {
  const f = fixture({ host: false, discovering: false });
  f.primary.setup();
  await f.drain();
  f.clock.advance(1000);
  await settle();
  expect(f.attachments[0]!.port.read().resource).toMatchObject({ state: 'unavailable' });
  expect(f.errors.length).toBeGreaterThan(0);
  expect(f.calls).toEqual([]);
});
