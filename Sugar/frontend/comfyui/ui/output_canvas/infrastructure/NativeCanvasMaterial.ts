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
/** Project submitted native geometry and acknowledged frame facts into material readiness. */
import type { CanvasSurfaceGeometry } from '../domain/CanvasViewPort.js';
import type { NativeCanvasBinding } from './NativeCanvasBinding.js';
import type { NativeCanvasDesired } from './NativeCanvasDesired.js';
import { nativeCanvasBackdrop } from './NativeCanvasCapabilities.js';

/** Read only material-relevant acknowledgement facts after the presenter verifies ownership. */
export type NativeCanvasMaterialBinding = Readonly<
  Pick<NativeCanvasBinding, 'host' | 'committed' | 'lastGeometry'>
>;

/** Require accepted display allocation/material, independently of newer input-routing requests. */
export function nativeMaterialReady(
  binding: NativeCanvasMaterialBinding | null,
  shown: boolean,
  geometry: CanvasSurfaceGeometry | null,
): boolean {
  return (
    binding !== null &&
    nativeCanvasBackdrop(binding.host) &&
    binding.committed &&
    shown &&
    geometry?.visible === true &&
    !!geometry.backdrops?.length &&
    materialIdentity(binding.lastGeometry) === materialIdentity(geometry)
  );
}

/** Preserve drawable allocation while withholding visibility for a prepared empty presentation. */
export function nativeMaterialGeometry(
  binding: Readonly<Pick<NativeCanvasBinding, 'prepared' | 'lastGeometry'>>,
  desired: Readonly<Pick<NativeCanvasDesired, 'geometry' | 'shown' | 'previewable'>>,
  observingPreview: boolean,
): CanvasSurfaceGeometry | null {
  const geometry = desired.geometry ?? binding.lastGeometry;
  if (!binding.prepared || !geometry) return null;
  return desired.shown || (desired.previewable && observingPreview)
    ? geometry
    : { ...geometry, visible: false, clip: { x: 0, y: 0, width: 0, height: 0 } };
}

/** Preserve acknowledged pixels through source replacement and same-content allocation changes. */
export function nativeMaterialRetention(options: {
  readonly binding: Readonly<
    Pick<NativeCanvasBinding, 'committed' | 'lastGeometry' | 'selectionRevision'>
  > | null;
  readonly previousContent: string | undefined;
  readonly desired: Readonly<
    Pick<NativeCanvasDesired, 'geometry' | 'content' | 'presentable' | 'shown' | 'previewable'>
  >;
  readonly observing: boolean;
  readonly shown: boolean;
}): { readonly retainedFinal: boolean; readonly resized: boolean; readonly retainPixels: boolean } {
  const { binding, desired, observing, shown, previousContent } = options;
  const geometryMatches =
    binding?.lastGeometry != null &&
    desired.geometry !== null &&
    materialIdentity(binding.lastGeometry) === materialIdentity(desired.geometry);
  const presentsPixels = desired.shown || (desired.previewable && observing);
  return {
    retainedFinal:
      desired.previewable && observing && binding?.selectionRevision !== null && geometryMatches,
    // The host re-presents committed content at a new allocation without losing its shown frame.
    resized:
      shown &&
      previousContent === desired.content &&
      binding?.committed === true &&
      desired.presentable &&
      presentsPixels,
    retainPixels: presentsPixels && binding?.committed === true && geometryMatches,
  };
}

/** Identify the pixels and rounded material of an allocation, excluding gesture-routing state. */
function materialIdentity(geometry: CanvasSurfaceGeometry | null): string | null {
  if (!geometry) return null;
  return JSON.stringify([
    geometry.coordinateSpace,
    geometry.units,
    geometry.origin,
    geometry.viewport,
    geometry.clip,
    geometry.physicalPixelsPerCssPixel,
    geometry.visible,
    geometry.backdrops ?? [],
  ]);
}
