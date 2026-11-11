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
/** Aggregate hosts position their own menu using finite viewport CSS pixels; the rectangle never supplies a caret. */
export interface FieldActionContext {
  readonly left: number;
  readonly right: number;
  readonly top: number;
  readonly bottom: number;
}
/** Accepted means delegated intent, rather than completion of an external operation or a guaranteed source mutation. */
export type FieldActionOutcome =
  | { readonly status: 'accepted' }
  | {
      readonly status: 'refused';
      readonly reason: 'disabled' | 'stale' | 'composing' | 'disposed';
    };
/** Literal finite presentation contains only issued callbacks, never foreign editable commands. */
export type FieldActionEntry =
  | { readonly kind: 'heading'; readonly label: string }
  | { readonly kind: 'separator' }
  | {
      readonly kind: 'submenu';
      readonly label: string;
      readonly children: readonly FieldActionEntry[];
    }
  | {
      readonly kind: 'action';
      readonly label: string;
      readonly title?: string;
      readonly enabled: boolean;
      readonly checked?: boolean;
      /** Invoke the original issued capability against current native admission. */
      activate(): Promise<FieldActionOutcome>;
    };
/** Release invalidates only this invocation; hosts release on menu dismissal or replacement. */
export interface FieldActionContribution {
  readonly entries: readonly FieldActionEntry[];
  release(): void;
}
/** Finite native preparation settles on supersession and teardown; composition offers no executable contribution. */
export type FieldActionPreparation =
  | { readonly status: 'ready'; readonly contribution: FieldActionContribution }
  | { readonly status: 'unavailable'; readonly reason: 'composing' }
  | { readonly status: 'cancelled' | 'disposed' };
