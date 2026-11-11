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
/** Negotiate committed-change observation independently of coherent native capture. */
export type CanvasObservationStatus = 'observing' | 'unsupported' | 'unavailable';

/** Retire an observation request immediately, including while its acknowledgement is pending. */
export interface CanvasChangeSubscription {
  readonly ready: Promise<CanvasObservationStatus>;
  stop(): void;
}

/** Publish metadata changes only after the host explicitly accepts observation. */
export interface CanvasChangeObservation<Change> {
  subscribe(listener: (change: Change) => void): CanvasChangeSubscription;
}
