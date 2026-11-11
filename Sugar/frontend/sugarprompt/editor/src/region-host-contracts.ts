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
/** Host synchronization carries literal ordinal names and source-free same-editor targets. */
import type { RegionName, PromptDocumentKind } from '../../core/src/index.js';

/** Only the native regional owner can create an executable inspection target. */
export const regionNameTargetBrand: unique symbol = Symbol();
/** A finite current-source/context handle is consumed once by the editor that issued it. */
export interface RegionNameTarget {
  readonly kind: 'region-name-target';
  readonly [regionNameTargetBrand]: true;
}
/** Ordinary source authoring publishes trimmed changes; baseline and peer synchronization do not echo authored events. */
export interface RegionNameChange {
  readonly sourceRevision: number;
  readonly changes: readonly RegionName[];
  readonly names: readonly RegionName[];
}
/** Explicit inspection reuses prepared names and captures current source/context without changing selection or history. */
export type RegionNameInspection =
  | {
      readonly status: 'ready';
      readonly sourceRevision: number;
      readonly names: readonly RegionName[];
      readonly target: RegionNameTarget;
    }
  | { readonly status: 'composing' }
  | {
      readonly status: 'unsupported';
      readonly documentKind: Exclude<PromptDocumentKind, 'prompt'>;
    };
/** Peer updates are host source loading, including read-only fields; stale, consumed and foreign targets cannot mutate source. */
export type RegionNameSyncOutcome =
  | { readonly status: 'applied' | 'unchanged' }
  | {
      readonly status: 'rejected';
      readonly reason:
        | 'foreign-command'
        | 'consumed-command'
        | 'stale-source'
        | 'stale-context'
        | 'composing'
        | 'disposed';
    };
