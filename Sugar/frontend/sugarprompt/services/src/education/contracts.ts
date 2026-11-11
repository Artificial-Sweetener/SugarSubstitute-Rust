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
/** Host-owned one-time education acknowledgement, separate from feature preferences and source history. */

/** The host atomically claims one previously unseen tip or reports its existing acknowledgement. */
export interface EducationClaim {
  readonly status: 'claimed' | 'seen' | 'unavailable';
}

/** Persist the single nested-parenthesis seen flag in the host's schema and sharing scope. */
export interface ParenthesisEducationAdapter {
  /** Atomically read/mark seen across editor instances; cancellation must prevent later persistence. */
  claim(signal: AbortSignal): Promise<EducationClaim>;
}

/** Prepared education contains the conversion's depth and exact multiplier, never prompt content. */
export interface NestedParenthesisTip {
  readonly nestingDepth: number;
  readonly weight: string;
}

/** Observable availability and presentation state; a visible tip has already been acknowledged by the host. */
export type EducationSnapshot =
  | { readonly status: 'visible'; readonly tip: NestedParenthesisTip }
  | {
      readonly status: 'idle' | 'claiming' | 'seen' | 'unavailable' | 'failed' | 'disposed';
      readonly tip: null;
    };

/** Expected suppression, cancellation and storage unavailability remain explicit outcomes. */
export interface EducationOutcome {
  readonly status: 'shown' | 'suppressed' | 'unavailable' | 'cancelled' | 'disposed';
}
