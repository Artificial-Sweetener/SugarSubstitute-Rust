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
/** Stateless cold calibration borrows the sole latest Placement intent and the actual CollisionBase geometry owner. */
import type { EditorView } from '@codemirror/view';
import type { ReorderMeasureInput, ReorderMeasureOwner } from './reorder-measurement-contracts.js';
import type { ReorderBaseContext } from './reorder-base-context.js';
import type { ReorderCollisionBase } from './reorder-collision-base.js';

/** Placement owns latest release and immutable collision identities; a context marks its actual in-flight cold pipeline. */
export interface ReorderPointerIntent {
  readonly input: ReorderMeasureInput;
  readonly epoch: number;
  readonly hidden: ReorderMeasureInput['hidden'];
  readonly held: number;
  readonly capture: ReorderMeasureInput['capture'];
  readonly pointer: number | null;
  release: boolean;
  calibration: ReorderBaseContext | null;
}
/** Exact Placement/retarget epoch and Session currentness are borrowed, never reproduced as another state owner. */
export interface ReorderCalibrationAdmission {
  readonly owner: ReorderMeasureOwner;
  /** Pure identity check reads no geometry, styles or source. */
  owned(): boolean;
}
/** Native requests and failures retain the existing mounted resource owner. */
export interface ReorderCalibrationPorts extends ReorderCalibrationAdmission {
  /** Preserve raw unexpected failures through the existing all-attempt boundary. */
  observe<T>(action: () => T): T;
  /** Queue only supported CM requests after the enclosing native WRITE has returned. */
  schedule<T>(request: {
    readonly key: object;
    read(view: EditorView): T;
    write(value: T, view: EditorView): void;
  }): void;
  /** Completion resumes the normal target READ with the latest actual coordinates and release. */
  ready(): void;
  /** A still-held invalidated native context renews through the existing keyed target READ. */
  refresh(): void;
}
/** Immutable collision operands distinguish a current phase from a mutated or transferred hold. */
export function matchesReorderPointerIntent(intent: ReorderPointerIntent): boolean {
  const input = intent.input;
  return (
    input.hidden === intent.hidden &&
    input.held === intent.held &&
    input.capture === intent.capture &&
    input.pointer === intent.pointer
  );
}
/** Coordinate-only motion may retain this exact hidden operand/context, but no callback can admit a replaced issue. */
export function currentReorderPointerCalibration(
  intent: ReorderPointerIntent,
  ports: ReorderCalibrationAdmission,
): boolean {
  const context = intent.calibration,
    input = intent.input;
  if (!context) return false;
  /** Immutable operands and exact context are rechecked after every delegated native boundary. */
  const matches = (): boolean =>
    intent.calibration === context && matchesReorderPointerIntent(intent);
  return (
    ports.owned() &&
    matches() &&
    ports.owner.current(input) &&
    ports.owned() &&
    matches() &&
    context.current() &&
    ports.owned() &&
    matches()
  );
}
/** Mount/calibration follow-ups use finite guarded microtasks, preventing recursive geometry READs in one CM measure loop. */
export function requestReorderPointerCalibration(
  intent: ReorderPointerIntent,
  base: ReorderCollisionBase,
  ports: ReorderCalibrationPorts,
): void {
  const context = intent.calibration;
  if (!context) return;
  /** Separate superseded identity from a still-owned native context that can no longer supply geometry. */
  const status = (): 'retired' | 'unavailable' | 'stale' | 'current' => {
    const owned = (): boolean =>
      ports.owned() && intent.calibration === context && matchesReorderPointerIntent(intent);
    if (!owned()) return 'retired';
    const session = ports.owner.current(intent.input);
    if (!owned()) return 'retired';
    if (!session) return 'unavailable';
    const native = context.current();
    if (!owned()) return 'retired';
    return native ? 'current' : 'stale';
  };
  /** Geometry refusal retains its provenance; only stale geometry may renew an exact unreleased gesture. */
  const finish = (outcome: 'ready' | 'stale' | 'unavailable'): void =>
    queueMicrotask(() =>
      ports.observe(() => {
        const state = status();
        if (state === 'retired') return;
        if (state === 'unavailable' || outcome === 'unavailable') ports.owner.failed();
        else if (state === 'stale' || outcome === 'stale') {
          if (intent.release) ports.owner.failed();
          else ports.refresh();
        } else if (state === 'current') ports.ready();
        else ports.owner.failed();
      }),
    );
  queueMicrotask(() =>
    ports.observe(() => {
      const state = status();
      if (state === 'retired') return;
      if (state === 'unavailable') {
        ports.owner.failed();
        return;
      }
      if (state === 'stale') {
        finish('stale');
        return;
      }
      ports.schedule({
        key: intent,
        read: () =>
          ports.observe(() => {
            const state = status();
            return state === 'retired'
              ? null
              : state !== 'current'
                ? ({ kind: state } as const)
                : base.measure(intent.input, context);
          }),
        write: (read) =>
          ports.observe(() => {
            const state = status();
            if (state === 'retired' || !read) return;
            if (state === 'unavailable' || read.kind === 'unavailable') {
              finish('unavailable');
              return;
            }
            if (state === 'stale' || read.kind === 'stale') {
              finish('stale');
              return;
            }
            if (read.kind === 'calibration') {
              const aligned = base.align(read);
              const state = status();
              if (state === 'retired') return;
              if (!aligned) finish('unavailable');
              else if (state === 'current') requestReorderPointerCalibration(intent, base, ports);
              else finish(state === 'stale' ? 'stale' : 'unavailable');
            } else finish('ready');
          }),
      });
    }),
  );
}
