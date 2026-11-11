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
/** Decoded host list management contracts; persistence format and containment remain host-owned. */
import type { BundledTag, TagCompletionCatalog } from '../completion/contracts.js';

/** One host-identified line-based list; names and identifiers are opaque to service storage. */
export interface AutocompleteList {
  readonly id: string;
  readonly name: string;
  readonly kind: 'custom' | 'censored';
  readonly enabled: boolean;
  readonly text: string;
}

/** An atomic host revision contains every managed list in host-defined catalog order. */
export interface AutocompleteListSnapshot {
  readonly revision: string;
  readonly lists: readonly AutocompleteList[];
}

/** Mutations preserve exact authored text; rename may replace a host path-derived identifier. */
export type AutocompleteListCommand =
  | {
      readonly kind: 'create';
      readonly name: string;
      readonly listKind: AutocompleteList['kind'];
      readonly text: string;
    }
  | { readonly kind: 'edit'; readonly id: string; readonly text: string }
  | { readonly kind: 'rename'; readonly id: string; readonly name: string }
  | { readonly kind: 'delete'; readonly id: string }
  | { readonly kind: 'enable'; readonly id: string; readonly enabled: boolean };

/** Expected host rejection is distinct from malformed output and unexpected thrown causes. */
export type AutocompleteListHostResult =
  | {
      readonly status: 'ready';
      readonly snapshot: AutocompleteListSnapshot;
      readonly affectedId: string | null;
    }
  | {
      readonly status: 'unavailable' | 'invalid' | 'conflict' | 'not-found' | 'name-conflict';
    };

/** One host owns containment, existing storage and atomic compare-and-swap revision admission. */
export interface AutocompleteListStorage {
  /** Return an atomic full revision, with affectedId null. Abort may retire acknowledgement before persistence settles. */
  read(signal: AbortSignal): Promise<unknown>;
  /** Apply exactly one semantically admitted command against expectedRevision; ready returns the resulting full revision and affected identity. */
  mutate(
    command: AutocompleteListCommand,
    expectedRevision: string,
    signal: AbortSignal,
  ): Promise<unknown>;
}

/** One immutable publication gives consumers completion and its matching exact tag knowledge together. */
export interface PreparedAutocompleteLists extends AutocompleteListSnapshot {
  readonly catalog: TagCompletionCatalog;
}

/** Prepared readiness and validated unpublished writes are distinct; expected rejection alone never certifies persistence. */
export type AutocompleteListOutcome =
  | { readonly status: 'ready'; readonly value: PreparedAutocompleteLists }
  | {
      readonly status: 'committed';
      readonly value: AutocompleteListSnapshot;
      readonly retirement: 'cancelled' | 'disposed';
    }
  | {
      readonly status:
        | 'disposed'
        | 'cancelled'
        | 'busy'
        | 'stale'
        | 'invalid'
        | 'unavailable'
        | 'conflict'
        | 'not-found'
        | 'name-conflict';
    };

/** Actual host transport settlement is observable independently of retired acknowledgement; settled does not certify commit. */
export type AutocompleteListSettlement =
  | { readonly status: 'settled' }
  | { readonly status: 'failed'; readonly cause: unknown };

/** Bundled rows are fixed for one service lifetime; a replacement dataset starts a new lifetime. */
export interface AutocompleteListOptions {
  readonly storage: AutocompleteListStorage;
  readonly bundled: readonly BundledTag[];
  /** Observe prepared atomic publications; authenticated raw write receipts survive later preparation or observer failure. */
  readonly observe?: (value: PreparedAutocompleteLists) => void;
}
