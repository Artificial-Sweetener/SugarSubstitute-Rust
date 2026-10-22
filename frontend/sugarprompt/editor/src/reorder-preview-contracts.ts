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
/** Immutable reorder paint publication shared by native marks and derived fragments. */
import type { ReorderOutput, ReorderState, SceneDocument } from '../../core/src/index.js';
import type { NativeReorderCapture } from './reorder-command.js';

/** Native publication reports current paint, rejected currentness, or unavailable canonical output; Session owns retirement. */
export type ReorderPublicationOutcome = 'published' | 'stale' | 'unavailable';

/** A session publishes one Core operand/output scoped to its original admitted value. */
export interface ReorderPreview {
  readonly token: object;
  readonly capture: NativeReorderCapture;
  /** Original chip identity remains owned by the session while absent from a hidden preview. */
  readonly activeChip: number;
  readonly order: ReorderState;
  readonly output: ReorderOutput;
  /** Sole Core classification of actual proposed decoded UTF-16 source, prepared before publication. */
  readonly scenes: SceneDocument | null;
  /** Collision paint is measured separately; pointer proposals retain the invisible held footprint. */
  readonly presentation: 'complete' | 'collision' | 'pointer';
}

/** One primary hold freezes only derived CM paint; Session/Core retain the proposed order. */
export interface ReorderPointerPaint {
  readonly hold: object;
  readonly anchor: ReorderPreview;
  /** A refused complete presentation returns to CM for the remainder of this exact hold. */
  readonly mode: 'stage' | 'fallback';
}
