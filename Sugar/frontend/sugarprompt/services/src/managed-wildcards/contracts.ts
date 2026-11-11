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
/** Managed wildcard storage and library contracts expose relative identities, never desktop root paths. */
import type { WildcardCatalog, WildcardCatalogInput } from '../wildcards/contracts.js';

/** One managed TXT/CSV file identity; candidate content/semantics are owned by existing document parsers. */
export interface ManagedWildcardFile {
  readonly path: string;
  readonly identifier: string;
  readonly kind: 'text' | 'csv';
}
/** A ready library snapshot is also its genuine immutable revision ticket for file commands. */
export type ManagedWildcardSnapshot =
  | {
      readonly status: 'ready';
      readonly revision: string;
      readonly files: readonly ManagedWildcardFile[];
      readonly catalog: WildcardCatalog;
    }
  | {
      readonly status: 'idle' | 'loading' | 'stale' | 'unavailable' | 'failed' | 'disposed';
    };
/** Expected external availability and optimistic concurrency failures preserve original storage bytes. */
export interface ManagedWildcardStorageFailure {
  readonly status: 'unavailable' | 'denied' | 'missing' | 'conflict' | 'stale';
}
/** Host list and decoded catalog metadata share one storage revision, including other approved catalog roots. */
export type ManagedWildcardListResult =
  | {
      readonly status: 'ready';
      readonly revision: string;
      readonly paths: readonly string[];
      readonly catalog: WildcardCatalogInput;
    }
  | ManagedWildcardStorageFailure;
/** Exact decoded source preserves CR/LF/CRLF and all authored TXT/CSV spelling. */
export type ManagedWildcardReadResult =
  | {
      readonly status: 'read';
      readonly revision: string;
      readonly text: string;
    }
  | ManagedWildcardStorageFailure;
/** A confirmed mutation returns its committed revision before metadata refresh. */
export type ManagedWildcardChangeResult =
  | { readonly status: 'changed'; readonly revision: string }
  | ManagedWildcardStorageFailure;
/** Host owns its managed root, symlink/drive containment, UTF8/CSV decoding, atomic revision checks and persistence.
 * Aborted requests settle and prevent subsequent persistence; unexpected failures reject with their original cause.
 * An unavailable/denied/conflict/stale mutation preserves original bytes. Each successful mutation advances revision.
 */
export interface ManagedWildcardStorage {
  /** Enumerate only managed TXT/CSV paths and already-decoded catalog metadata without reading source for display. */
  list(signal: AbortSignal): Promise<ManagedWildcardListResult>;
  /** Read exact source after atomically checking the supplied storage revision. */
  read(path: string, revision: string, signal: AbortSignal): Promise<ManagedWildcardReadResult>;
  /** Create or replace exact source, atomically checking revision and honoring cancellation before persistence. */
  write(
    path: string,
    text: string,
    revision: string,
    signal: AbortSignal,
  ): Promise<ManagedWildcardChangeResult>;
  /** Rename within the managed root, atomically checking revision; destination collisions return conflict. */
  rename(
    from: string,
    to: string,
    revision: string,
    signal: AbortSignal,
  ): Promise<ManagedWildcardChangeResult>;
  /** Delete within the managed root after atomically checking revision; absent files may return missing. */
  delete(path: string, revision: string, signal: AbortSignal): Promise<ManagedWildcardChangeResult>;
  /** Synchronously invalidate host process metadata/candidate caches; no filesystem/network work or user-data deletion. */
  invalidate(): void;
}
/** Optional folder capability owns the OS root and opening behavior; intent contains no guessed path. */
export interface ManagedWildcardFolderAdapter {
  /** Handle one explicit folder action with finite cancellation and typed availability. */
  open(
    intent: { readonly kind: 'open-managed-wildcards' },
    signal: AbortSignal,
  ): Promise<{ readonly status: 'opened' | 'unavailable' | 'denied' }>;
}
/** Command admission and lifetime outcomes never claim a file mutation or a current catalog. */
export type ManagedWildcardFailure =
  | ManagedWildcardStorageFailure
  | { readonly status: 'cancelled' | 'disposed' | 'busy' }
  | { readonly status: 'invalid'; readonly reason: 'path' | 'content' };
/** Library refresh publishes only a genuine current complete snapshot. */
export type ManagedWildcardRefreshOutcome =
  | {
      readonly status: 'ready';
      readonly library: Extract<ManagedWildcardSnapshot, { status: 'ready' }>;
    }
  | ManagedWildcardFailure;
/** Source reads keep authoring contents separate from library metadata and generation output. */
export type ManagedWildcardReadOutcome =
  | {
      readonly status: 'read';
      readonly path: string;
      readonly revision: string;
      readonly text: string;
    }
  | ManagedWildcardFailure;
/** A mutation remains observably committed even when subsequent metadata refresh is unavailable. */
export type ManagedWildcardChangeOutcome =
  | {
      readonly status: 'changed';
      readonly revision: string;
      readonly library: ManagedWildcardSnapshot;
    }
  | ManagedWildcardFailure;
/** Folder actions have their own latest-wins finite intent lifetime and never mutate a library revision. */
export interface ManagedWildcardFolderOutcome {
  readonly status: 'opened' | 'unavailable' | 'denied' | 'cancelled' | 'disposed';
}
