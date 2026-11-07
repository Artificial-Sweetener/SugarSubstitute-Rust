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
/** Snapshot existing native presentation facts without observing DOM or granting host access. */
import type { CanvasSurfaceGeometry } from '../domain/CanvasViewPort.js';
import type { NativeCanvasBinding } from './NativeCanvasBinding.js';
import type { NativeCanvasDesired } from './NativeCanvasDesired.js';
import { nativeMaterialReady } from './NativeCanvasMaterial.js';
import {
  IDLE_NATIVE_CANVAS_PREVIEW_DIAGNOSTICS,
  type NativeCanvasPreview,
} from './NativeCanvasPreview.js';

/** Copy only the bounded material allocation; routing and cancellation objects stay private. */
export function nativeMaterialGeometrySnapshot(geometry: CanvasSurfaceGeometry | null) {
  return (
    geometry &&
    Object.freeze({
      coordinateSpace: geometry.coordinateSpace,
      units: geometry.units,
      origin: Object.freeze({ ...geometry.origin }),
      viewport: Object.freeze({ ...geometry.viewport }),
      clip: Object.freeze({ ...geometry.clip }),
      physicalPixelsPerCssPixel: geometry.physicalPixelsPerCssPixel,
      visible: geometry.visible,
      ...(geometry.backdrops
        ? {
            backdrops: Object.freeze(
              geometry.backdrops.map((region) => Object.freeze({ ...region })),
            ),
          }
        : {}),
    })
  );
}

/** Preserve immutable preview counters and cached material reasons without changing presentation. */
export function nativeCanvasDiagnostics(
  preview: NativeCanvasPreview | null,
  binding: NativeCanvasBinding | null,
  desired: Pick<NativeCanvasDesired, 'geometry'> | null,
  shown: boolean,
) {
  const capabilities = binding?.host.capabilities;
  return Object.freeze({
    ...(preview?.diagnostics() ?? IDLE_NATIVE_CANVAS_PREVIEW_DIAGNOSTICS),
    material: Object.freeze({
      bound: binding !== null,
      bindingCapabilities: capabilities
        ? Object.freeze({
            ...capabilities,
            imageFormats: Object.freeze([...capabilities.imageFormats]),
            viewportCommands: Object.freeze([...capabilities.viewportCommands]),
            previewFormats: Object.freeze([...capabilities.previewFormats]),
            ...(capabilities.backdrop
              ? { backdrop: Object.freeze({ ...capabilities.backdrop }) }
              : {}),
          })
        : null,
      prepared: binding?.prepared ?? false,
      committed: binding?.committed ?? false,
      shown,
      desired: desired !== null,
      currentGeometry: nativeMaterialGeometrySnapshot(desired?.geometry ?? null),
      acknowledgedGeometry: nativeMaterialGeometrySnapshot(binding?.lastGeometry ?? null),
      activeOperation: binding?.active?.type ?? null,
      selectionRevision: binding?.selectionRevision ?? null,
      conditionsReady: nativeMaterialReady(binding, shown, desired?.geometry ?? null),
    }),
  });
}
