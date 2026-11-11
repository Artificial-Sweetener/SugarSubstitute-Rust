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
/** Shared native measurement and borrowed calibration contracts are dependency leaves, without presentation or allocation ownership. */
import type { ReadonlyMediaPlan } from './readonly-widget-media.js';
import type { EditorView } from '@codemirror/view';
import type { ReorderMeasureInput } from './reorder-measurement-contracts.js';
import type { ReorderRect } from './reorder-geometry.js';
import type { ReorderPreview } from './reorder-preview-contracts.js';
import type { ReorderBaseContext } from './reorder-base-context.js';
import type { ReorderMotionScene } from './reorder-motion-scene.js';
/** The existing measurement owner retains all scheduling, source/session and unexpected-failure authority. */
export interface ReorderMotionPorts {
  current(input: ReorderMeasureInput): boolean;
  rects(): readonly ReorderRect[];
  /** Geometry invalidation renews calibration; admitted endpoint publication schedules only its previously unavailable marker READ. */
  refresh(input: ReorderMeasureInput, reason: 'geometry' | 'endpoint'): void;
  observe<T>(action: () => T): T;
  schedule<T>(request: {
    read(view: EditorView): T;
    write?(measured: T, view: EditorView): void;
    key?: unknown;
  }): void;
}
/** Native wrapper declarations await WRITE normalization; descendants consume the single canonical widget renderer. */
export interface PreparedPassiveMotion {
  /** Exact primary pointer projection requests complete, extent-qualified native line coverage. */
  readonly whole?: true;
  readonly preview: ReorderPreview;
  readonly context: ReorderBaseContext;
  /** Resolved native declarations are normalized once by the slot WRITE owner. */
  readonly contentCss: string;
  readonly contentClasses: string;
  readonly appearance: readonly (readonly [string, string])[];
  readonly lineCss: string;
  readonly scrollerCss: string;
  readonly scrollerClasses: string;
  readonly held: number | null;
  /** Only native READ may compare captured scroll/viewport/device scale values; these getters can force layout. */
  readonly readViewportCurrent: () => boolean;
}
/** Actual layout provides placement first and complete admitted passive paint afterward. */
export type PassiveMotionRead =
  | { readonly kind: 'layer' }
  | {
      readonly kind: 'placement';
      readonly left: number;
      readonly top: number;
      readonly scaleX: number;
      readonly scaleY: number;
    }
  | { readonly kind: 'media'; readonly plan: ReadonlyMediaPlan }
  | { readonly kind: 'carriers'; readonly operation: 'create' | 'discard' }
  | { readonly kind: 'ready'; readonly scene: ReorderMotionScene }
  | { readonly kind: 'unavailable' };

/** The coordinate registry borrows only idempotent child retirement; no canonical painter or preparation type is imported back. */
export interface PassiveMotionChild {
  /** The exact proposal generation owns currentness; retained display is a separate resource lease. */
  current(): boolean;
  /** Exact proposal identity can be checked while its reserved canonical paint is detached. */
  publicationCurrent(): boolean;
  destroy(): void;
}
/** Canonical child paint borrows native calibration, placement and exact-child release without importing its owning stage. */
export interface PassiveMotionStage {
  readonly node: HTMLElement;
  /** READ certifies the native marker draw and original scroll extents. */
  readLayer(): 'waiting' | 'ready' | 'unavailable';
  current(): boolean;
  measured(): { readonly scaleX: number; readonly scaleY: number } | null;
  readPlacement(
    prepared: PreparedPassiveMotion,
  ): Extract<PassiveMotionRead, { kind: 'placement' }> | null;
  place(
    prepared: PreparedPassiveMotion,
    read: Extract<PassiveMotionRead, { kind: 'placement' }>,
  ): void;
  release(child: PassiveMotionChild): void;
}
