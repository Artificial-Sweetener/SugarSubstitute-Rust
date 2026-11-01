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
/** Exercise native viewport intent within the existing presentation/revision lifetime. */
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import { NativeCanvasPresentation } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasPresentation.js';
import type { NativeCanvasPresentationStatus } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasStatus.js';
import type {
  CanvasSurfaceCommand,
  CanvasSurfaceHost,
  CanvasSurfaceResult,
  CanvasSurfaceViewportState,
} from '../../../integrations/contracts/CanvasSurface.js';
import type { CanvasCancellationSource } from '../../../integrations/contracts/CanvasHost.js';
import type {
  CanvasCommandScope,
  CanvasSurfaceGeometry,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import { canvasNavigationScope, canvasSnapshot } from '../support/CanvasPortFixture.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';

const geometry: CanvasSurfaceGeometry = {
  coordinateSpace: 'client',
  units: 'css-pixels',
  origin: { x: 0, y: 0 },
  viewport: { width: 500, height: 400 },
  clip: { x: 0, y: 0, width: 500, height: 400 },
  physicalPixelsPerCssPixel: 1.5,
  visible: true,
  focused: true,
};
const fitted: CanvasSurfaceViewportState = {
  mode: 'fit',
  physicalScale: 0.75,
  dimensions: { width: 1024, height: 768 },
};
const native: CanvasSurfaceViewportState = {
  mode: 'native',
  physicalScale: 1,
  dimensions: { width: 1024, height: 768 },
};

/** Drain only actual promise continuations; the fake boundary owns every native acknowledgement. */
async function settle(): Promise<void> {
  for (let turn = 0; turn < 12; turn++) await Promise.resolve();
}

/** Hold native completions after cancellation to verify that credit and revision remain serialized. */
function fixture(
  commands: CanvasSurfaceHost['capabilities']['viewportCommands'] = [
    'fit',
    'native',
    'zoom',
    'pan',
  ],
) {
  const catalog = canvasCatalogFixture();
  const calls: CanvasSurfaceCommand[] = [];
  const pending: {
    resolve(result: CanvasSurfaceResult): void;
    reject(error: Error): void;
    cancelled: boolean;
  }[] = [];
  const states: (CanvasSurfaceViewportState | null)[] = [];
  const errors: unknown[] = [];
  const statuses: NativeCanvasPresentationStatus[] = [];
  const visible: boolean[] = [];
  const host: CanvasSurfaceHost = {
    capabilities: {
      schema: 1,
      imageFormats: ['png'],
      viewportCommands: commands,
      viewportState: true,
      previewFormats: [],
      comparison: false,
      overview: false,
      input: false,
    },
    execute(command: CanvasSurfaceCommand, cancellation?: CanvasCancellationSource) {
      calls.push(command);
      return new Promise<CanvasSurfaceResult>((resolve, reject) => {
        const record = {
          reject,
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
    address: canvasSnapshot('viewport').address,
    host: () => host,
    accepts: () => true,
    acceptsPreparation: () => false,
    prepared: () => {},
    visible: (value) => visible.push(value),
    status: (value) => statuses.push(value),
    viewport: (value) => states.push(value),
    report: (error) => errors.push(error),
  });
  return {
    presentation,
    calls,
    pending,
    states,
    errors,
    statuses,
    visible,
    first: catalog.entries[0]!.items[0]!,
    second: catalog.entries[1]!.items[0]!,
    respond(index: number, result: CanvasSurfaceResult) {
      pending[index]!.resolve(result);
    },
  };
}

/** Establish selected native pixels and then obtain their actual camera through the same stream. */
async function prepare(f: ReturnType<typeof fixture>): Promise<void> {
  f.presentation.update(geometry, f.first);
  for (let index = 0; index < 4; index++) {
    f.respond(index, { status: index === 0 ? 'ready' : 'applied', message: '' });
    await settle();
  }
  expect(f.calls[4]).toMatchObject({
    type: 'surface.viewport.request',
    revision: 5,
    command: { type: 'read' },
  });
  expect(f.states.at(-1)).toBeNull();
  f.respond(4, { status: 'applied', message: '', viewport: fitted });
  await settle();
  expect(f.states.at(-1)).toEqual(fitted);
}

/** Preserve the shared command scope while exposing its cancellation to the test. */
function scope(): { scope: CanvasCommandScope; cancel(): void } {
  const controller = new AbortController();
  return {
    scope: {
      ...canvasNavigationScope(),
      signal: controller.signal,
      current: () => !controller.signal.aborted,
    },
    cancel: () => controller.abort(),
  };
}

test('reads authoritative native state and applies all four commands in one increasing revision stream', async () => {
  const f = fixture();
  await prepare(f);
  for (const command of [
    { type: 'fit' },
    { type: 'native' },
    { type: 'zoom', factor: 1.25 },
    { type: 'pan', x: 0.08, y: -0.1, units: 'viewport' },
  ] as const) {
    const index = f.calls.length;
    const pending = f.presentation.viewport(command, scope().scope);
    expect(f.calls[index]).toMatchObject({
      type: 'surface.viewport.request',
      revision: index + 1,
      command,
    });
    f.respond(index, { status: 'applied', message: '', viewport: native });
    expect(await pending).toBe(true);
    await settle();
    expect(f.states.at(-1)).toEqual(native);
  }
  expect(f.errors).toEqual([]);
});

test('refuses unsupported commands without sending a native operation', async () => {
  const f = fixture(['fit']);
  await prepare(f);
  expect(await f.presentation.viewport({ type: 'zoom', factor: 1.25 }, scope().scope)).toBe(false);
  expect(f.calls).toHaveLength(5);
});

test('awaits cancelled native acknowledgement before forwarding the newest bounded viewport intent', async () => {
  const f = fixture();
  await prepare(f);
  const firstScope = scope();
  const first = f.presentation.viewport({ type: 'fit' }, firstScope.scope);
  let firstDone = false;
  void first.then(() => {
    firstDone = true;
  });
  const second = f.presentation.viewport({ type: 'native' }, scope().scope);
  expect(f.pending[5]!.cancelled).toBe(true);
  await settle();
  expect(firstDone).toBe(false);
  expect(f.calls).toHaveLength(6);
  f.respond(5, { status: 'cancelled', message: '' });
  await settle();
  expect(await first).toBe(false);
  expect(f.calls[6]).toMatchObject({
    type: 'surface.viewport.request',
    command: { type: 'native' },
    revision: 7,
  });
  f.respond(6, { status: 'applied', message: '', viewport: native });
  expect(await second).toBe(true);
  expect(f.states.at(-1)).toEqual(native);
});

test('scope cancellation waits for the actual terminal outcome and never publishes its late camera', async () => {
  const f = fixture();
  await prepare(f);
  const admitted = scope();
  const operation = f.presentation.viewport({ type: 'native' }, admitted.scope);
  admitted.cancel();
  expect(f.pending[5]!.cancelled).toBe(true);
  f.respond(5, { status: 'applied', message: '', viewport: native });
  expect(await operation).toBe(false);
  await settle();
  expect(f.states.at(-1)).toBeNull();
  expect(f.calls[6]).toMatchObject({ type: 'surface.viewport.request', command: { type: 'read' } });
  const actual = { ...native, mode: 'custom' as const, physicalScale: 0.92 };
  f.respond(6, { status: 'applied', message: '', viewport: actual });
  await settle();
  expect(f.states.at(-1)).toEqual(actual);
});

test('source replacement cancels pending navigation and reads only the replacement camera', async () => {
  const f = fixture();
  await prepare(f);
  const operation = f.presentation.viewport({ type: 'native' }, scope().scope);
  f.presentation.update(geometry, f.second);
  expect(f.states.at(-1)).toBeNull();
  expect(f.pending[5]!.cancelled).toBe(true);
  f.respond(5, { status: 'applied', message: '', viewport: native });
  await settle();
  expect(await operation).toBe(false);
  expect(f.states).not.toContainEqual(native);
  expect(f.calls[6]).toMatchObject({ type: 'surface.select', itemId: f.second.id });
  f.respond(6, { status: 'applied', message: '' });
  await settle();
  expect(f.calls[7]).toMatchObject({ type: 'surface.viewport.request', command: { type: 'read' } });
  const replacement = { ...fitted, dimensions: { width: 640, height: 480 } };
  f.respond(7, { status: 'applied', message: '', viewport: replacement });
  await settle();
  expect(f.states.at(-1)).toEqual(replacement);
});

test('geometry changes discard the old camera until acknowledged geometry and its state read', async () => {
  const f = fixture();
  await prepare(f);
  f.presentation.update(
    { ...geometry, viewport: { width: 700, height: 400 }, clip: { ...geometry.clip, width: 700 } },
    f.first,
  );
  expect(f.states.at(-1)).toBeNull();
  expect(f.calls[5]?.type).toBe('surface.geometry');
  f.respond(5, { status: 'applied', message: '' });
  await settle();
  expect(f.calls[6]).toMatchObject({ type: 'surface.viewport.request', command: { type: 'read' } });
  f.respond(6, { status: 'applied', message: '', viewport: native });
  await settle();
  expect(f.states.at(-1)).toEqual(native);
});

test('menu focus changes preserve admitted navigation and serialize the latest focus after its acknowledgement', async () => {
  const f = fixture();
  await prepare(f);
  const operation = f.presentation.viewport({ type: 'native' }, scope().scope);
  f.presentation.update({ ...geometry, focused: false }, f.first);
  expect(f.pending[5]!.cancelled).toBe(false);
  expect(f.calls).toHaveLength(6);
  f.respond(5, { status: 'applied', message: '', viewport: native });
  expect(await operation).toBe(true);
  await settle();
  expect(f.calls[6]).toMatchObject({ type: 'surface.geometry', geometry: { focused: false } });
  expect(f.states.at(-1)).toEqual(native);
  f.presentation.update(geometry, f.first);
  expect(f.pending[6]!.cancelled).toBe(false);
  f.respond(6, { status: 'applied', message: '' });
  await settle();
  expect(f.calls[7]).toMatchObject({ type: 'surface.geometry', geometry: { focused: true } });
  f.respond(7, { status: 'applied', message: '' });
  await settle();
  expect(f.calls).toHaveLength(8);
  expect(f.states.at(-1)).toEqual(native);
  expect(f.visible).toEqual([true]);
});

test('a physical resize still cancels navigation after an intervening focus change', async () => {
  const f = fixture();
  await prepare(f);
  const operation = f.presentation.viewport({ type: 'native' }, scope().scope);
  f.presentation.update({ ...geometry, focused: false }, f.first);
  f.presentation.update({ ...geometry, viewport: { width: 700, height: 400 } }, f.first);
  expect(f.pending[5]!.cancelled).toBe(true);
  f.respond(5, { status: 'cancelled', message: '' });
  expect(await operation).toBe(false);
  await settle();
  expect(f.calls[6]).toMatchObject({
    type: 'surface.geometry',
    geometry: { viewport: { width: 700 } },
  });
  expect(f.states.at(-1)).toBeNull();
});

test('a superseded viewport that actually applies cannot publish or settle the newer intent', async () => {
  const f = fixture();
  await prepare(f);
  const old = f.presentation.viewport({ type: 'native' }, scope().scope);
  const current = f.presentation.viewport({ type: 'fit' }, scope().scope);
  f.respond(5, { status: 'applied', message: '', viewport: native });
  await settle();
  expect(await old).toBe(false);
  expect(f.states).not.toContainEqual(native);
  expect(f.calls[6]).toMatchObject({ type: 'surface.viewport.request', command: { type: 'fit' } });
  f.respond(6, { status: 'applied', message: '', viewport: fitted });
  expect(await current).toBe(true);
});

test('failed initial commit settles unsent viewport intent without creating another host request', async () => {
  const f = fixture();
  f.presentation.update(geometry, f.first);
  for (let index = 0; index < 3; index++) {
    f.respond(index, { status: index === 0 ? 'ready' : 'applied', message: '' });
    await settle();
  }
  expect(f.calls[3]?.type).toBe('surface.commit');
  const intent = f.presentation.viewport({ type: 'native' }, scope().scope);
  f.respond(3, { status: 'unavailable', message: '' });
  await settle();
  expect(await intent).toBe(false);
  expect(f.calls).toHaveLength(4);
  expect(f.states.at(-1)).toBeNull();
});

test('retirement cancels native navigation and discards late state before release completes', async () => {
  const f = fixture();
  await prepare(f);
  const operation = f.presentation.viewport({ type: 'native' }, scope().scope);
  const retired = f.presentation.dispose();
  expect(f.pending[5]!.cancelled).toBe(true);
  expect(f.calls[6]?.type).toBe('surface.release');
  f.respond(5, { status: 'applied', message: '', viewport: native });
  await settle();
  expect(await operation).toBe(false);
  expect(f.states.at(-1)).toBeNull();
  f.respond(6, { status: 'applied', message: '' });
  await retired;
});

test('commit transport failure settles unsent navigation without retry and preserves the original cause', async () => {
  const f = fixture();
  f.presentation.update(geometry, f.first);
  for (let index = 0; index < 3; index++) {
    f.respond(index, { status: index === 0 ? 'ready' : 'applied', message: '' });
    await settle();
  }
  const intent = f.presentation.viewport({ type: 'native' }, scope().scope);
  const failure = new Error('Native admission failed');
  f.pending[3]!.reject(failure);
  await settle();
  expect(await intent).toBe(false);
  expect(f.calls).toHaveLength(4);
  expect(f.errors).toEqual([failure]);
});

test('a stale navigation error preserves its cause while the newer intent still progresses', async () => {
  const f = fixture();
  await prepare(f);
  const previous = f.presentation.viewport({ type: 'fit' }, scope().scope);
  const current = f.presentation.viewport({ type: 'native' }, scope().scope);
  const failure = new Error('Old viewport failed');
  f.pending[5]!.reject(failure);
  await settle();
  expect(await previous).toBe(false);
  expect(f.errors).toEqual([failure]);
  expect(f.calls[6]).toMatchObject({
    type: 'surface.viewport.request',
    command: { type: 'native' },
  });
  f.respond(6, { status: 'applied', message: '', viewport: native });
  expect(await current).toBe(true);
});

test('bounds pending viewport intent to the newest command while the old native operation retires', async () => {
  const f = fixture();
  await prepare(f);
  const first = f.presentation.viewport({ type: 'fit' }, scope().scope);
  const middle = f.presentation.viewport({ type: 'native' }, scope().scope);
  const latest = f.presentation.viewport({ type: 'zoom', factor: 0.8 }, scope().scope);
  expect(await middle).toBe(false);
  expect(f.calls).toHaveLength(6);
  f.respond(5, { status: 'cancelled', message: '' });
  await settle();
  expect(await first).toBe(false);
  expect(f.calls[6]).toMatchObject({
    type: 'surface.viewport.request',
    command: { type: 'zoom', factor: 0.8 },
  });
  f.respond(6, { status: 'applied', message: '', viewport: native });
  expect(await latest).toBe(true);
});

test('cancels unsent viewport intent locally without cancelling required preparation', async () => {
  const f = fixture();
  f.presentation.update(geometry, f.first);
  const admitted = scope();
  const operation = f.presentation.viewport({ type: 'native' }, admitted.scope);
  admitted.cancel();
  expect(await operation).toBe(false);
  expect(f.pending[0]!.cancelled).toBe(false);
  expect(f.calls).toHaveLength(1);
});

test('an unavailable viewport mutation preserves accepted pixels and recovers with exactly one read', async () => {
  const f = fixture();
  await prepare(f);
  const operation = f.presentation.viewport({ type: 'native' }, scope().scope);
  f.respond(5, { status: 'unavailable', message: '' });
  expect(await operation).toBe(false);
  await settle();
  expect(f.calls[6]).toMatchObject({ type: 'surface.viewport.request', command: { type: 'read' } });
  expect(f.states.at(-1)).toBeNull();
  expect(f.statuses.at(-1)?.state).toBe('ready');
  expect(f.visible).toEqual([true]);
  f.respond(6, { status: 'applied', message: '', viewport: fitted });
  await settle();
  expect(f.states.at(-1)).toEqual(fitted);
  expect(f.calls).toHaveLength(7);
  expect(f.errors).toEqual([]);
});

for (const status of ['unavailable', 'error', 'stale'] as const) {
  test(`a ${status} recovery read fails closed without retrying the mutation or read`, async () => {
    const f = fixture();
    await prepare(f);
    const operation = f.presentation.viewport({ type: 'native' }, scope().scope);
    f.respond(5, { status: 'unavailable', message: '' });
    expect(await operation).toBe(false);
    await settle();
    f.respond(6, { status, message: '' });
    await settle();
    await settle();
    expect(f.calls).toHaveLength(7);
    expect(f.states.at(-1)).toBeNull();
    expect(f.statuses.at(-1)?.state).toBe(status === 'error' ? 'error' : 'unavailable');
  });
}

test('a stale viewport mutation remains fail-closed and does not start a speculative read', async () => {
  const f = fixture();
  await prepare(f);
  const operation = f.presentation.viewport({ type: 'native' }, scope().scope);
  f.respond(5, { status: 'stale', message: '' });
  expect(await operation).toBe(false);
  await settle();
  expect(f.calls).toHaveLength(6);
  expect(f.states.at(-1)).toBeNull();
  expect(f.statuses.at(-1)?.state).toBe('unavailable');
});
