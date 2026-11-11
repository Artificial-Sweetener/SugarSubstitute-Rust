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
/** Fence translucent chrome to actual native geometry and commit acknowledgements. */
import { jest } from '@jest/globals';
import type { CanvasSurfaceHost } from '../../../integrations/contracts/CanvasSurface.js';
import type { CanvasSurfaceGeometry } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import { NativeCanvasPresentation } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasPresentation.js';
import {
  nativePreviewHost,
  previewGeometry,
  finalViewport,
} from '../support/NativeCanvasPreviewFixture.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import { canvasSnapshot } from '../support/CanvasPortFixture.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';

const geometry: CanvasSurfaceGeometry = {
  ...previewGeometry,
  backdrops: [{ x: 8, y: 350, width: 200, height: 42, cornerRadius: 8, sigma: 10 }],
};

/** Drain promise continuations while the external boundary owns native completion. */
async function settle(): Promise<void> {
  for (let step = 0; step < 16; step++) await Promise.resolve();
}

/** Compose the real presentation owner with controllable native replies. */
function fixture(backdrop = true, comparison = false) {
  const native = nativePreviewHost(false);
  const host: CanvasSurfaceHost = {
    ...native.host,
    capabilities: {
      ...native.host.capabilities,
      schema: 3,
      viewportState: comparison,
      comparison,
      viewportCommands: comparison
        ? [...native.host.capabilities.viewportCommands, 'wipe']
        : native.host.capabilities.viewportCommands,
      ...(backdrop ? { backdrop: { schema: 1 } } : {}),
    },
  };
  const changed = jest.fn();
  const report = jest.fn();
  const presentation = new NativeCanvasPresentation({
    address: canvasSnapshot('acrylic').address,
    clock: new WorkflowTestClock(),
    host: () => host,
    accepts: () => true,
    acceptsPreparation: () => false,
    prepared: () => {},
    visible: () => {},
    status: () => {},
    viewport: () => {},
    preview: () => {},
    backdropChanged: changed,
    report,
  });
  return {
    ...native,
    presentation,
    changed,
    report,
    item: canvasCatalogFixture().entries[0]!.items[0]!,
  };
}

/** Establish the exact geometry, selection and commit in their native revision order. */
async function commit(f: ReturnType<typeof fixture>, current = geometry): Promise<void> {
  f.presentation.update(current, f.item);
  for (let index = 0; index < 4; index++) {
    expect(f.presentation.backdropReady(current)).toBe(false);
    f.respond(index, index === 0 ? 'ready' : 'applied');
    await settle();
  }
}

test('configuration acknowledgement cannot enable native transparency before committed pixels', async () => {
  const f = fixture();
  await commit(f);
  expect(f.calls.map((call) => call.type)).toEqual([
    'surface.prepare',
    'surface.geometry',
    'surface.select',
    'surface.commit',
  ]);
  expect(f.presentation.backdropReady(geometry)).toBe(true);
  expect(f.changed).toHaveBeenCalled();
  const release = f.presentation.dispose();
  expect(f.presentation.backdropReady(geometry)).toBe(false);
  f.respond(4);
  await release;
});

test('new bar bounds stay opaque through an older geometry reply and only the latest ACK restores transparency', async () => {
  const f = fixture();
  await commit(f);
  const first: CanvasSurfaceGeometry = {
    ...geometry,
    backdrops: [{ ...geometry.backdrops![0]!, width: 220 }],
  };
  const latest: CanvasSurfaceGeometry = {
    ...geometry,
    backdrops: [{ ...geometry.backdrops![0]!, width: 280 }],
  };
  f.presentation.update(first, f.item);
  expect(f.presentation.backdropReady(first)).toBe(false);
  f.presentation.update(latest, f.item);
  f.respond(4);
  await settle();
  expect(f.presentation.backdropReady(latest)).toBe(false);
  expect(f.calls[5]).toMatchObject({ type: 'surface.geometry', geometry: latest });
  f.respond(5);
  await settle();
  expect(f.presentation.backdropReady(latest)).toBe(true);
  expect(f.presentation.backdropReady(first)).toBe(false);
  expect(f.calls.filter((call) => call.type === 'surface.select')).toHaveLength(1);
  const release = f.presentation.dispose();
  f.respond(6);
  await release;
});

test('a cancelled material geometry preserves the queued same-key routing wake and restores acrylic after its ACK', async () => {
  const f = fixture();
  await commit(f);
  const allocation = { ...geometry, backdrops: [{ ...geometry.backdrops![0]!, width: 220 }] };
  const latest = {
    ...allocation,
    focused: true,
    occlusions: [{ x: 8, y: 350, width: 220, height: 42 }],
  };
  try {
    f.presentation.update(allocation, f.item);
    expect(f.calls[4]).toMatchObject({ type: 'surface.geometry', geometry: allocation });
    f.presentation.update(latest, f.item);
    expect(f.calls).toHaveLength(5);
    expect(f.pending[4]?.cancelled).toBe(false);
    f.respond(4, 'cancelled');
    await settle();
    expect(f.calls[5]).toMatchObject({ type: 'surface.geometry', geometry: latest });
    expect(f.presentation.backdropReady(latest)).toBe(false);
    f.respond(5);
    await settle();
    expect(f.presentation.backdropReady(latest)).toBe(true);
    expect(f.presentation.diagnostics().material).toMatchObject({
      conditionsReady: true,
      activeOperation: null,
      acknowledgedGeometry: { backdrops: [{ width: 220 }] },
    });
    expect(f.calls).toHaveLength(6);
    expect(f.calls.filter((call) => call.type === 'surface.select')).toHaveLength(1);
  } finally {
    const release = f.presentation.dispose();
    f.respond(f.calls.length - 1);
    await release;
  }
});

test('a cancelled geometry with no newer wake stays failed rather than retrying without intent', async () => {
  const f = fixture();
  await commit(f);
  const allocation = { ...geometry, backdrops: [{ ...geometry.backdrops![0]!, width: 220 }] };
  f.presentation.update(allocation, f.item);
  f.respond(4, 'cancelled');
  await settle();
  expect(f.calls).toHaveLength(5);
  expect(f.presentation.backdropReady(allocation)).toBe(false);
  const release = f.presentation.dispose();
  f.respond(5);
  await release;
});

test('a failed geometry or retired attachment cannot retain a ready material state', async () => {
  const f = fixture();
  await commit(f);
  const latest = { ...geometry, physicalPixelsPerCssPixel: 2 };
  f.presentation.update(latest, f.item);
  f.respond(4, 'error');
  await settle();
  expect(f.presentation.backdropReady(latest)).toBe(false);
  const release = f.presentation.dispose();
  expect(f.presentation.backdropReady(geometry)).toBe(false);
  f.respond(5);
  await release;
});

test('an applied hidden geometry configuration cannot expose native material', async () => {
  const f = fixture();
  await commit(f);
  const hidden = { ...geometry, visible: false, clip: { x: 0, y: 0, width: 0, height: 0 } };
  f.presentation.update(hidden, f.item);
  expect(f.presentation.backdropReady(hidden)).toBe(false);
  f.respond(4);
  await settle();
  expect(f.presentation.backdropReady(hidden)).toBe(false);
  expect(f.presentation.backdropReady(geometry)).toBe(false);
  const release = f.presentation.dispose();
  f.respond(5);
  await release;
});

test.each([false, true])(
  'hosts with empty material geometry never enable native transparency (negotiated=%s)',
  async (backdrop) => {
    const f = fixture(backdrop);
    await commit(f, previewGeometry);
    expect(f.presentation.backdropReady(previewGeometry)).toBe(false);
    const release = f.presentation.dispose();
    f.respond(4);
    await release;
  },
);

test('accepted acrylic never flashes pending across source, comparison-pair and divider replacements', async () => {
  const f = fixture(true, true);
  await commit(f);
  f.respond(4, 'applied', finalViewport);
  await settle();
  expect(f.presentation.backdropReady(geometry)).toBe(true);
  const routing = {
    ...geometry,
    focused: true,
    modal: false,
    occlusions: [{ x: 8, y: 350, width: 200, height: 42 }],
  };
  const outputs = canvasCatalogFixture().entries.map((entry) => entry.items[0]!);
  const second = outputs[1]!;
  const third = outputs[2]!;
  /** Observe every in-flight native boundary, not only the eventual settled frame. */
  const finish = async (): Promise<void> => {
    for (let step = 0; step < 12; step++) {
      expect(f.presentation.backdropReady(routing)).toBe(true);
      const index = f.pending.findIndex((call) => !call.settled);
      if (index < 0) return;
      const call = f.calls[index]!;
      f.respond(
        index,
        'applied',
        call.type === 'surface.viewport.request' ? finalViewport : undefined,
      );
      await settle();
    }
    throw new Error('Native presentation did not settle its bounded operations.');
  };
  f.presentation.update(routing, second);
  expect(f.calls.at(-1)?.type).toBe('surface.geometry');
  expect(f.presentation.backdropReady(routing)).toBe(true);
  await finish();
  f.presentation.update(routing, second, null, {
    item: f.item,
    orientation: 'vertical',
    split: 0.5,
  });
  expect(f.calls.at(-1)?.type).toBe('surface.select');
  await finish();
  f.presentation.update(routing, second, null, {
    item: f.item,
    orientation: 'vertical',
    split: 0.25,
  });
  f.presentation.update(routing, second, null, {
    item: f.item,
    orientation: 'vertical',
    split: 0.75,
  });
  await finish();
  expect(f.calls.at(-1)).toMatchObject({
    type: 'surface.viewport.request',
    command: { type: 'wipe', split: 0.75 },
  });
  f.presentation.update(routing, second, null, {
    item: third,
    orientation: 'vertical',
    split: 0.75,
  });
  await finish();
  expect(f.presentation.backdropReady(routing)).toBe(true);
  const release = f.presentation.dispose();
  f.respond(f.calls.length - 1);
  await release;
});
