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
/** Present A/B comparison through the native revision stream owned by the presentation. */
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import { NativeCanvasPresentation } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasPresentation.js';
import type { NativeCanvasComparison } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasDesired.js';
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
  CanvasSurfaceHost,
  CanvasSurfaceResult,
  CanvasSurfaceViewportState,
} from '../../../integrations/contracts/CanvasSurface.js';
import type { CanvasCancellationSource } from '../../../integrations/contracts/CanvasHost.js';
import type { WorkflowOutputItem } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import type { CanvasSurfaceGeometry } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import type { CanvasMessage } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { connectCanvasSurface } from '../../editor_integrations/support/canvas_surface.js';
import { canvasSnapshot } from '../support/CanvasPortFixture.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';

const geometry: CanvasSurfaceGeometry = {
  coordinateSpace: 'client',
  units: 'css-pixels',
  origin: { x: 0, y: 0 },
  viewport: { width: 500, height: 400 },
  clip: { x: 0, y: 0, width: 500, height: 400 },
  physicalPixelsPerCssPixel: 1,
  visible: true,
  focused: false,
};
const comparing: CanvasSurfaceCapabilities = {
  schema: 1,
  imageFormats: ['png'],
  viewportCommands: ['fit', 'native', 'zoom', 'pan', 'wipe'],
  viewportState: true,
  previewFormats: [],
  comparison: true,
  overview: false,
};
const pairViewport: CanvasSurfaceViewportState = {
  mode: 'fit',
  physicalScale: 0.25,
  dimensions: { width: 3840, height: 2160 },
  secondaryDimensions: { width: 1920, height: 1080 },
};

/** Drain only actual promise continuations; the fake boundary owns every native acknowledgement. */
async function settle(): Promise<void> {
  for (let turn = 0; turn < 12; turn++) await Promise.resolve();
}

/** Hold each native completion so in-flight and pending divider work remain observable. */
function fixture(capabilities: CanvasSurfaceCapabilities = comparing, transport?: CanvasClient) {
  const catalog = canvasCatalogFixture();
  const calls: CanvasSurfaceCommand[] = [];
  const pending: { resolve(result: CanvasSurfaceResult): void; cancelled: boolean }[] = [];
  const states: (CanvasSurfaceViewportState | null)[] = [];
  const statuses: string[] = [];
  const errors: unknown[] = [];
  const host: CanvasSurfaceHost = {
    capabilities,
    execute(command: CanvasSurfaceCommand, cancellation?: CanvasCancellationSource) {
      calls.push(command);
      return new Promise<CanvasSurfaceResult>((resolve) => {
        const record = {
          resolve: (result: CanvasSurfaceResult) => {
            stop?.();
            resolve(result);
          },
          cancelled: false,
        };
        const stop = cancellation?.subscribe(() => {
          record.cancelled = true;
        });
        pending.push(record);
      });
    },
  };
  const presentation = new NativeCanvasPresentation({
    clock: new WorkflowTestClock(),
    preview: () => {},
    address: canvasSnapshot('comparison').address,
    host: () => transport?.surface ?? host,
    accepts: () => true,
    acceptsPreparation: () => false,
    prepared: () => {},
    visible: () => {},
    status: (value) => statuses.push(value.state),
    viewport: (value) => states.push(value),
    report: (error) => errors.push(error),
  });
  const [a, b, c] = catalog.entries.map((entry) => entry.items[0]!);
  return {
    presentation,
    calls,
    pending,
    states,
    statuses,
    errors,
    a: a!,
    b: b!,
    c: c!,
    respond(index: number, result: CanvasSurfaceResult) {
      pending[index]!.resolve(result);
    },
  };
}

/** Describe B and a divider exactly as the attachment derives them from the view state. */
function pair(
  item: WorkflowOutputItem,
  split = 0.5,
  orientation: NativeCanvasComparison['orientation'] = 'vertical',
): NativeCanvasComparison {
  return { item, orientation, split };
}

/** Expect the protocol's atomic A/B selection for two catalog outputs. */
function selectPair(primary: WorkflowOutputItem, secondary: WorkflowOutputItem) {
  return {
    type: 'surface.select',
    itemId: primary.id,
    file: primary.file,
    comparison: { itemId: secondary.id, file: secondary.file },
  };
}

/** Expect the divider placement carried by the existing viewport request. */
function wipe(split: number, orientation: NativeCanvasComparison['orientation'] = 'vertical') {
  return { type: 'surface.viewport.request', command: { type: 'wipe', orientation, split } };
}

/** Commit an A/B pair and acknowledge its first divider placement. */
async function presentPair(f: ReturnType<typeof fixture>): Promise<void> {
  f.presentation.update(geometry, f.a, null, pair(f.b));
  for (let index = 0; index < 4; index++) {
    f.respond(index, { status: index === 0 ? 'ready' : 'applied', message: '' });
    await settle();
  }
  expect(f.calls[2]).toMatchObject({ ...selectPair(f.a, f.b), revision: 3 });
  expect(f.calls[4]).toMatchObject({ ...wipe(0.5), revision: 5 });
  f.respond(4, { status: 'applied', message: '', viewport: pairViewport });
  await settle();
  expect(f.states.at(-1)).toEqual(pairViewport);
  expect(f.calls).toHaveLength(5);
}

test('selects A and B under one revision and places the divider without a separate read', async () => {
  const f = fixture();
  await presentPair(f);
  expect(f.calls.map(({ type }) => type)).toEqual([
    'surface.prepare',
    'surface.geometry',
    'surface.select',
    'surface.commit',
    'surface.viewport.request',
  ]);
  expect(f.errors).toEqual([]);
});

test('coalesces divider drags to one placement in flight and only the newest pending value', async () => {
  const f = fixture();
  await presentPair(f);
  f.presentation.update(geometry, f.a, null, pair(f.b, 0.2));
  expect(f.calls[5]).toMatchObject(wipe(0.2));
  f.presentation.update(geometry, f.a, null, pair(f.b, 0.3));
  f.presentation.update(geometry, f.a, null, pair(f.b, 0.4, 'horizontal'));
  await settle();
  expect(f.calls).toHaveLength(6);
  expect(f.pending[5]!.cancelled).toBe(false);
  f.respond(5, { status: 'applied', message: '', viewport: pairViewport });
  await settle();
  expect(f.calls).toHaveLength(7);
  expect(f.calls[6]).toMatchObject({ ...wipe(0.4, 'horizontal'), revision: 7 });
  f.respond(6, { status: 'applied', message: '', viewport: pairViewport });
  await settle();
  expect(f.calls).toHaveLength(7);
  f.presentation.update(geometry, f.a, null, pair(f.b, 0.4, 'horizontal'));
  await settle();
  expect(f.calls).toHaveLength(7);
  expect(f.errors).toEqual([]);
});

test('never publishes a divider acknowledgement for a pair that was replaced in flight', async () => {
  const f = fixture();
  await presentPair(f);
  f.presentation.update(geometry, f.a, null, pair(f.b, 0.7));
  expect(f.calls[5]).toMatchObject(wipe(0.7));
  f.presentation.update(geometry, f.a, null, pair(f.c, 0.7));
  expect(f.pending[5]!.cancelled).toBe(true);
  expect(f.states.at(-1)).toBeNull();
  const late = { ...pairViewport, physicalScale: 3 };
  f.respond(5, { status: 'applied', message: '', viewport: late });
  await settle();
  expect(f.states).not.toContainEqual(late);
  expect(f.calls[6]).toMatchObject(selectPair(f.a, f.c));
  f.respond(6, { status: 'applied', message: '' });
  await settle();
  expect(f.calls[7]).toMatchObject(wipe(0.7));
  f.respond(7, { status: 'applied', message: '', viewport: pairViewport });
  await settle();
  expect(f.states.at(-1)).toEqual(pairViewport);
});

test('resends the atomic selection when A and B swap or B changes', async () => {
  const f = fixture();
  await presentPair(f);
  f.presentation.update(geometry, f.b, null, pair(f.a));
  await settle();
  expect(f.calls[5]).toMatchObject(selectPair(f.b, f.a));
  f.respond(5, { status: 'applied', message: '' });
  await settle();
  expect(f.calls[6]).toMatchObject(wipe(0.5));
  f.respond(6, { status: 'applied', message: '', viewport: pairViewport });
  await settle();
  f.presentation.update(geometry, f.b, null, pair(f.c));
  await settle();
  expect(f.calls[7]).toMatchObject(selectPair(f.b, f.c));
  f.respond(7, { status: 'applied', message: '' });
  await settle();
  expect(f.calls[8]).toMatchObject(wipe(0.5));
});

test('leaving comparison selects A alone and reads its camera without a divider', async () => {
  const f = fixture();
  await presentPair(f);
  f.presentation.update(geometry, f.a);
  await settle();
  expect(f.calls[5]).toEqual(
    expect.objectContaining({ type: 'surface.select', itemId: f.a.id, file: f.a.file }),
  );
  expect(f.calls[5]).not.toHaveProperty('comparison');
  f.respond(5, { status: 'applied', message: '' });
  await settle();
  expect(f.calls[6]).toMatchObject({ type: 'surface.viewport.request', command: { type: 'read' } });
});

test('a host without negotiated comparison never receives a pair or a divider', async () => {
  const f = fixture({ ...comparing, comparison: false, viewportCommands: ['fit'] });
  f.presentation.update(geometry, f.a, null, pair(f.b));
  for (let index = 0; index < 4; index++) {
    f.respond(index, { status: index === 0 ? 'ready' : 'applied', message: '' });
    await settle();
  }
  expect(f.calls[2]).toEqual({
    type: 'surface.select',
    address: canvasSnapshot('comparison').address,
    revision: 3,
    itemId: f.a.id,
    file: f.a.file,
  });
  expect(f.calls[4]).toMatchObject({ type: 'surface.viewport.request', command: { type: 'read' } });
  f.presentation.update(geometry, f.a, null, pair(f.b, 0.9));
  await settle();
  expect(f.calls).toHaveLength(5);
});

test('a refused divider reports the host outcome instead of retrying it', async () => {
  const f = fixture();
  f.presentation.update(geometry, f.a, null, pair(f.b));
  for (let index = 0; index < 4; index++) {
    f.respond(index, { status: index === 0 ? 'ready' : 'applied', message: '' });
    await settle();
  }
  f.respond(4, { status: 'unavailable', message: 'No comparison is active.' });
  await settle();
  expect(f.statuses.at(-1)).toBe('unavailable');
  expect(f.calls).toHaveLength(5);
});

type SurfaceRequest = Extract<CanvasMessage, { readonly address: unknown }>;

/** Answer one exact surface request with a status-only reply. */
function statusReply(request: SurfaceRequest, status: 'ready' | 'applied' | 'cancelled') {
  return {
    protocol: request.protocol,
    version: request.version,
    session: request.session,
    requestId: request.requestId,
    type: 'surface.reply',
    address: request.address,
    revision: request.revision,
    status,
    message: '',
  } as const;
}

/** Answer one exact viewport request with an applied camera observation. */
function viewportReply(request: SurfaceRequest, viewport: CanvasSurfaceViewportState) {
  return {
    ...statusReply(request, 'applied'),
    type: 'surface.viewport.reply',
    viewport,
  } as const;
}

test('ignores foreign and post-cancellation divider replies on the real canvas transport', async () => {
  const sent: CanvasMessage[] = [];
  const reports: unknown[] = [];
  const client = new CanvasClient('native-comparison-transport', {
    send: (message) => sent.push(message),
    later: () => () => {},
    report: (error) => reports.push(error),
  });
  await connectCanvasSurface(client, sent, comparing);
  const f = fixture(comparing, client);
  const outstanding = (): SurfaceRequest => {
    const request = sent.at(-1);
    if (!request || !('address' in request)) throw new Error('Missing surface request.');
    return request;
  };
  f.presentation.update(geometry, f.a, null, pair(f.b, 0.6));
  for (const status of ['ready', 'applied', 'applied', 'applied'] as const) {
    expect(client.receive(statusReply(outstanding(), status))).toBe(true);
    await settle();
  }
  const placement = outstanding();
  expect(placement).toMatchObject(wipe(0.6));
  const foreign = { ...pairViewport, physicalScale: 9 };
  const unknownRequest = { ...placement, requestId: placement.requestId + 50 };
  expect(client.receive(viewportReply(unknownRequest, foreign))).toBe(false);
  await settle();
  expect(f.states).not.toContainEqual(foreign);
  expect(sent.at(-1)).toBe(placement);
  expect(client.receive(viewportReply(placement, pairViewport))).toBe(true);
  await settle();
  expect(f.states.at(-1)).toEqual(pairViewport);

  f.presentation.update(geometry, f.a, null, pair(f.b, 0.1));
  const superseded = outstanding();
  expect(superseded).toMatchObject(wipe(0.1));
  f.presentation.update(geometry, f.a, null, pair(f.c, 0.1));
  expect(sent.at(-1)).toMatchObject({ type: 'cancel', requestId: superseded.requestId });
  expect(client.receive(statusReply(superseded, 'cancelled'))).toBe(true);
  await settle();
  const late = { ...pairViewport, physicalScale: 7 };
  expect(client.receive(viewportReply(superseded, late))).toBe(false);
  await settle();
  expect(f.states).not.toContainEqual(late);
  expect(outstanding()).toMatchObject(selectPair(f.a, f.c));
  expect(reports).toEqual([]);
  expect(f.errors).toEqual([]);
  client.dispose();
});
