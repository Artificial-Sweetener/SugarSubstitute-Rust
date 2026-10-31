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
/** Derive the native presentation requested by the view owners, independent of host progress. */

import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { CanvasSurfaceGeometry } from '../domain/CanvasViewPort.js';
import type { CanvasComparison } from '../domain/CanvasViewState.js';
import type {
  CanvasSurfaceOverviewCell,
  CanvasSurfaceViewportCommand,
} from '../../../../../integrations/contracts/CanvasSurface.js';
import type { NativeCanvasOperation } from './NativeCanvasBinding.js';

/** Pair the resolved B image with the divider chosen by the selection owner. */
export interface NativeCanvasComparison {
  readonly item: WorkflowOutputItem;
  readonly orientation: CanvasComparison['orientation'];
  readonly split: number;
}

/** Describe one presentation identity; mutable members update it without replacing that identity. */
export interface NativeCanvasDesired {
  readonly empty: boolean;
  readonly previewable: boolean;
  /** Whether a final image or a nonempty overview grid is to be presented. */
  readonly shown: boolean;
  /** Focus updates retain this presentation identity and follow its current operation. */
  geometry: CanvasSurfaceGeometry | null;
  readonly item: WorkflowOutputItem | null;
  /** Divider updates retain this identity; only the B image participates in its key. */
  comparison: NativeCanvasComparison | null;
  /**
   * Natively painted overview cells, or null outside overview. Scroll and layout updates retain
   * this identity; only entering, leaving or emptying the grid participates in its key.
   */
  overview: readonly CanvasSurfaceOverviewCell[] | null;
  readonly key: string;
}

/**
 * Qualify the selected output, optional B image or overview grid against drawable geometry.
 *
 * A comparison is presented only with an eligible final image A and an image B, so a host that
 * receives the pair can always place the divider. Overview replaces the selected image; with no
 * paintable cell or drawable geometry it presents nothing rather than a retained final.
 * Preparation permits hidden staged geometry.
 */
export function nativeCanvasDesired(
  geometry: CanvasSurfaceGeometry | null,
  item: WorkflowOutputItem | null,
  comparison: NativeCanvasComparison | null,
  overview: readonly CanvasSurfaceOverviewCell[] | null,
  accepts: boolean,
  preparing: boolean,
): NativeCanvasDesired {
  const drawable =
    !!geometry &&
    geometry.viewport.width > 0 &&
    geometry.viewport.height > 0 &&
    (preparing || (geometry.visible && geometry.clip.width > 0 && geometry.clip.height > 0));
  const eligible = drawable && !overview && item?.mediaKind === 'image' ? item : null;
  const grid = overview && (drawable ? overview : []);
  const pair = eligible && comparison?.item.mediaKind === 'image' ? comparison : null;
  return {
    geometry,
    item: eligible,
    comparison: pair,
    overview: grid,
    key: JSON.stringify([
      geometry && { ...geometry, focused: false },
      item && [item.id, item.file, item.mediaKind],
      accepts,
      ...(pair ? [[pair.item.id, pair.item.file]] : []),
      ...(grid ? [['overview', grid.length > 0]] : []),
    ]),
    empty: grid ? grid.length === 0 : item?.mediaKind !== 'image',
    previewable: !grid && item === null,
    shown: eligible !== null || (grid?.length ?? 0) > 0,
  };
}

/** Identify the painted overview layout independently of the identity that entered overview. */
export function nativeCanvasOverviewKey(
  overview: readonly CanvasSurfaceOverviewCell[] | null,
): string | null {
  return overview && JSON.stringify(overview);
}

/** Identify divider placement independently of the pair that the select operation fixed. */
export function nativeCanvasWipeKey(comparison: NativeCanvasComparison | null): string | null {
  return comparison && JSON.stringify([comparison.orientation, comparison.split]);
}

/**
 * Select A and the optional B together so the host applies both under one revision, or select
 * the overview grid in their place.
 *
 * An overview is the displayed selection while shown: its key never equals an image key, so a
 * later image selection is always sent, and each newer layout is sent behind the one in flight.
 */
export function nativeCanvasSelection(desired: NativeCanvasDesired): {
  readonly key: string;
  readonly operation: Extract<
    NativeCanvasOperation,
    { readonly type: 'surface.select' | 'surface.overview' }
  >;
} | null {
  const { item, comparison, overview } = desired;
  if (overview?.length)
    return {
      key: JSON.stringify(['overview', overview]),
      operation: { type: 'surface.overview', cells: overview },
    };
  if (!item) return null;
  return {
    key: JSON.stringify(
      comparison
        ? [item.id, item.file, comparison.item.id, comparison.item.file]
        : [item.id, item.file],
    ),
    operation: {
      type: 'surface.select',
      itemId: item.id,
      file: item.file,
      ...(comparison
        ? { comparison: { itemId: comparison.item.id, file: comparison.item.file } }
        : {}),
    },
  };
}

/** Express the newest divider for a presented pair as the negotiated viewport command. */
export function nativeCanvasWipe(desired: NativeCanvasDesired): {
  readonly key: string;
  readonly command: Extract<CanvasSurfaceViewportCommand, { readonly type: 'wipe' }>;
} | null {
  const { item, comparison } = desired;
  const key = nativeCanvasWipeKey(comparison);
  return item && comparison && key
    ? {
        key,
        command: { type: 'wipe', orientation: comparison.orientation, split: comparison.split },
      }
    : null;
}
