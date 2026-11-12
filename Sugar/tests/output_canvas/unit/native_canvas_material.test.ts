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
/** Qualify native material from acknowledged binding facts and submitted geometry. */
import type { CanvasSurfaceGeometry } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import {
  nativeMaterialGeometry,
  nativeMaterialReady,
  nativeMaterialRetention,
  type NativeCanvasMaterialBinding,
} from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasMaterial.js';
import { nativePreviewHost, previewGeometry } from '../support/NativeCanvasPreviewFixture.js';
import { nativeCanvasDiagnostics } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasDiagnostics.js';
import { createNativeCanvasBinding } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasBinding.js';

const geometry: CanvasSurfaceGeometry = {
  ...previewGeometry,
  backdrops: [{ x: 8, y: 350, width: 200, height: 42, cornerRadius: 8, sigma: 10 }],
};

/** Project an already-owned native binding without creating a second presentation lifecycle. */
function binding(): NativeCanvasMaterialBinding {
  const host = nativePreviewHost(false).host;
  return {
    host: {
      ...host,
      capabilities: { ...host.capabilities, schema: 3, backdrop: { schema: 1 } },
    },
    committed: true,
    lastGeometry: geometry,
  };
}

test('material requires the exact visible committed geometry and negotiated support', () => {
  const current = binding();
  expect(nativeMaterialReady(current, true, geometry)).toBe(true);
  expect(nativeMaterialReady(null, true, geometry)).toBe(false);
  expect(nativeMaterialReady(current, false, geometry)).toBe(false);
  expect(nativeMaterialReady({ ...current, committed: false }, true, geometry)).toBe(false);
  expect(nativeMaterialReady({ ...current, lastGeometry: null }, true, geometry)).toBe(false);
  expect(nativeMaterialReady(current, true, null)).toBe(false);
  expect(nativeMaterialReady(current, true, previewGeometry)).toBe(false);
  expect(nativeMaterialReady(current, true, { ...geometry, visible: false })).toBe(false);
  expect(nativeMaterialReady(current, true, { ...geometry, focused: !geometry.focused })).toBe(
    true,
  );
  expect(
    nativeMaterialReady({ ...current, host: nativePreviewHost(false).host }, true, geometry),
  ).toBe(false);
});

test('preparation and an allocation precede any submitted material geometry', () => {
  const desired = { geometry, shown: true, previewable: false };
  expect(
    nativeMaterialGeometry({ prepared: false, lastGeometry: null }, desired, false),
  ).toBeNull();
  expect(
    nativeMaterialGeometry(
      { prepared: true, lastGeometry: null },
      { ...desired, geometry: null },
      false,
    ),
  ).toBeNull();
});

test('final and observed preview presentations preserve the actual visible geometry', () => {
  const current = { prepared: true, lastGeometry: null };
  expect(
    nativeMaterialGeometry(current, { geometry, shown: true, previewable: false }, false),
  ).toBe(geometry);
  expect(nativeMaterialGeometry(current, { geometry, shown: false, previewable: true }, true)).toBe(
    geometry,
  );
  expect(
    nativeMaterialGeometry(
      { prepared: true, lastGeometry: geometry },
      { geometry: null, shown: true, previewable: false },
      false,
    ),
  ).toBe(geometry);
});

test('empty and unobserved preview presentations retain allocation with a hidden empty clip', () => {
  const current = { prepared: true, lastGeometry: geometry };
  const desired = { geometry: null, shown: false, previewable: true };
  const hidden = nativeMaterialGeometry(current, desired, false);
  expect(hidden).toEqual({
    ...geometry,
    visible: false,
    clip: { x: 0, y: 0, width: 0, height: 0 },
  });
  expect(geometry.visible).toBe(true);
  expect(hidden?.backdrops).toBe(geometry.backdrops);
});

test('same-content resize retains committed pixels while replacement requires acknowledged allocation', () => {
  const desired = {
    geometry,
    content: 'image:1',
    presentable: true,
    shown: true,
    previewable: false,
  };
  const options = {
    binding: { committed: true, lastGeometry: geometry, selectionRevision: 3 },
    previousContent: desired.content,
    desired,
    observing: false,
    shown: true,
  };
  expect(nativeMaterialRetention(options)).toEqual({
    retainedFinal: false,
    resized: true,
    retainPixels: true,
  });
  const resized = { ...desired, geometry: { ...geometry, physicalPixelsPerCssPixel: 2 } };
  expect(nativeMaterialRetention({ ...options, desired: resized })).toEqual({
    retainedFinal: false,
    resized: true,
    retainPixels: false,
  });
  expect(nativeMaterialRetention({ ...options, previousContent: 'image:2' }).resized).toBe(false);
  expect(nativeMaterialRetention({ ...options, binding: null }).retainPixels).toBe(false);
  expect(
    nativeMaterialRetention({ ...options, binding: { ...options.binding, committed: false } })
      .resized,
  ).toBe(false);
});

test('observed preview preserves final camera only for the acknowledged selection allocation', () => {
  const options = {
    binding: { committed: true, lastGeometry: geometry, selectionRevision: 3 },
    previousContent: 'source:1',
    desired: {
      geometry,
      content: 'source:1',
      presentable: true,
      shown: false,
      previewable: true,
    },
    observing: true,
    shown: true,
  };
  expect(nativeMaterialRetention(options).retainedFinal).toBe(true);
  expect(nativeMaterialRetention({ ...options, observing: false })).toEqual({
    retainedFinal: false,
    resized: false,
    retainPixels: false,
  });
  expect(
    nativeMaterialRetention({
      ...options,
      binding: { ...options.binding, selectionRevision: null },
    }).retainedFinal,
  ).toBe(false);
  expect(
    nativeMaterialRetention({ ...options, binding: { ...options.binding, lastGeometry: null } })
      .retainedFinal,
  ).toBe(false);
});

test('routing updates retain the last accepted material while the host owns its replacement reply', () => {
  const current = binding();
  const routing = {
    ...geometry,
    focused: !geometry.focused,
    modal: true,
    occlusions: [{ x: 0, y: 0, width: 20, height: 20 }],
  };
  expect(nativeMaterialReady(current, true, routing)).toBe(true);
  expect(nativeMaterialReady({ ...current, lastGeometry: null }, true, routing)).toBe(false);
  for (const changed of [
    { ...routing, physicalPixelsPerCssPixel: 2 },
    { ...routing, origin: { x: geometry.origin.x + 1, y: geometry.origin.y } },
    { ...routing, backdrops: [{ ...geometry.backdrops![0]!, width: 240 }] },
    { ...routing, clip: { ...geometry.clip, width: geometry.clip.width - 1 } },
  ])
    expect(nativeMaterialReady(current, true, changed)).toBe(false);
});

test('a one-pixel change to B withdraws the whole comparison material until its new allocation is acknowledged', () => {
  const first = geometry.backdrops![0]!;
  const second = { ...first, x: 450, width: 44 };
  const pair = { ...geometry, backdrops: [first, second] };
  const current = { ...binding(), lastGeometry: pair };
  expect(nativeMaterialReady(current, true, pair)).toBe(true);
  const changed = { ...pair, backdrops: [first, { ...second, x: 449, width: 45 }] };
  expect(changed.backdrops[0]).toBe(pair.backdrops[0]);
  expect(nativeMaterialReady(current, true, changed)).toBe(false);
  expect(
    nativeMaterialRetention({
      binding: { ...current, selectionRevision: 3 },
      previousContent: 'pair:A:B',
      desired: {
        geometry: changed,
        content: 'pair:A:A',
        presentable: true,
        shown: true,
        previewable: false,
      },
      observing: false,
      shown: true,
    }).retainPixels,
  ).toBe(false);
  expect(nativeMaterialReady({ ...current, lastGeometry: changed }, false, changed)).toBe(false);
  expect(nativeMaterialReady({ ...current, lastGeometry: changed }, true, changed)).toBe(true);
});

test('native diagnostics distinguish actual binding and acknowledged material without exposing mutable geometry', () => {
  const current = createNativeCanvasBinding(binding().host);
  current.prepared = true;
  current.committed = true;
  current.lastGeometry = geometry;
  const diagnostics = nativeCanvasDiagnostics(null, current, { geometry }, true);
  expect(diagnostics.material).toMatchObject({
    bound: true,
    prepared: true,
    committed: true,
    shown: true,
    desired: true,
    conditionsReady: true,
    bindingCapabilities: { schema: 3, backdrop: { schema: 1 } },
  });
  expect(diagnostics.material.currentGeometry).not.toBe(geometry);
  expect(diagnostics.material.acknowledgedGeometry?.backdrops).not.toBe(geometry.backdrops);
  expect(Object.isFrozen(diagnostics.material.currentGeometry?.origin)).toBe(true);
  expect(Object.isFrozen(diagnostics.material.bindingCapabilities?.imageFormats)).toBe(true);
  expect(nativeCanvasDiagnostics(null, current, null, false).material).toMatchObject({
    desired: false,
    shown: false,
    conditionsReady: false,
  });
});
