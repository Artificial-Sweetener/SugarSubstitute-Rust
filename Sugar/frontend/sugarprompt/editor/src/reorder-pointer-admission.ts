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
/** Cold admission borrows the sole Measurement press and schedules its source-neutral publication outside native input/update delivery. */
import type { ReorderGrabPaint } from './reorder-grab-contracts.js';
import type { ReorderMeasureInput, ReorderMeasureOwner } from './reorder-measurement-contracts.js';

/** Measurement alone owns captured press/hotspot/paint and its latest threshold/release intent. */
export interface ReorderPointerPress {
  readonly input: ReorderMeasureInput;
  readonly x: number;
  readonly y: number;
  paint: ReorderGrabPaint | null;
  threshold: boolean;
  release: boolean;
  admission: object | null;
}

/** Exact native lifetime, failure boundary and successful feedback handoff remain with the mounted Measurement owner. */
export interface ReorderPointerAdmissionPorts {
  readonly owner: ReorderMeasureOwner;
  /** Pure captured epoch/press identity check performs no geometry or publication. */
  current(): boolean;
  /** Delegate native failures to the existing all-attempt Measurement resource boundary. */
  observe<T>(action: () => T): T;
  /** Register one keyed supported measurement job; source publication waits for its deferred WRITE callback. */
  schedule(request: {
    readonly key: object;
    /** Certify this exact primed press without publishing or sampling target geometry. */
    read(): boolean;
    /** Queue the admitted publication after the enclosing native measurement completes. */
    write(ready: boolean): void;
  }): void;
  /** Adopt the consumed original paint and latest coordinates after Session's genuine collision publication. */
  start(
    paint: ReorderGrabPaint,
    coordinates: { readonly x: number; readonly y: number },
    release: boolean,
  ): void;
}

/** One exact request coalesces the existing press; a released unavailable drop retires, while ordinary unavailability retries only on real input. */
export function requestReorderPointerAdmission(
  press: ReorderPointerPress,
  ports: ReorderPointerAdmissionPorts,
): void {
  const admission = {},
    paint = press.paint;
  press.admission = admission;
  /** Callback retirement cannot authorize a replaced request or clear a newer refresh/retry. */
  const owned = (): boolean =>
    press.admission === admission && ports.current() && press.admission === admission;
  /** Recheck exact paint and native identities after Session delegates currentness. */
  const current = (): boolean =>
    owned() && ports.owner.current(press.input) && owned() && press.paint === paint;
  /** Only this still-owned unsuccessful request relinquishes its retry reservation. */
  const reset = (): void => {
    if (owned()) press.admission = null;
  };
  ports.schedule({
    key: press,
    read: () => ports.observe(() => current() && press.threshold && !!paint),
    write: (ready) =>
      queueMicrotask(() => {
        if (!ready) {
          reset();
          return;
        }
        ports.observe(() => {
          try {
            if (!current() || !paint) return;
            if (!ports.owner.collision(press.input)) {
              if (current() && press.release) ports.owner.failed();
              return;
            }
            if (!current() || !press.input.coordinates) return;
            const coordinates = press.input.coordinates;
            press.paint = null;
            ports.start(paint, coordinates, press.release);
          } finally {
            reset();
          }
        });
      }),
  });
}
