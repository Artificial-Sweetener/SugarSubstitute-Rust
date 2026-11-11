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
/** Prepared exact edits bind to one native editor lifetime and source/context identity. */
import type { EditorState, TransactionSpec } from '@codemirror/state';
import { exactReplacement, readExactSource } from './exact-source.js';
import { editorFreshness, freshnessRejection, type EditorFreshness } from './freshness.js';

/** Opaque native command identity; serializing or copying its visible shape does not recreate an executable command. */
const sourceEditBrand: unique symbol = Symbol();

/** A source-free, immutable handle to one prepared exact-source edit; use only with the editor that prepared it. */
export interface PreparedSourceEdit {
  readonly kind: 'source-edit';
  readonly [sourceEditBrand]: true;
}

/** A source-free range reservation for host work whose replacement arrives later. */
export interface ReservedSourceEdit {
  readonly kind: 'source-reservation';
  readonly [sourceEditBrand]: true;
}

/** A reservation has the same preparation restrictions as an ordinary prepared edit. */
export type SourceEditReservation =
  | { readonly status: 'ready'; readonly command: ReservedSourceEdit }
  | Exclude<SourceEditPreparation, { readonly status: 'ready' }>;

/** Preparation rejects expected editing restrictions before retaining any command payload. */
export type SourceEditPreparation =
  | { readonly status: 'ready'; readonly command: PreparedSourceEdit }
  | {
      readonly status: 'blocked';
      readonly reason: 'read-only' | 'composing' | 'disposed';
    };

/** Rejection is source-free; an invalid or retired command never edits the text now occupying its old range. */
export type SourceEditOutcome =
  | { readonly status: 'applied' | 'unchanged' }
  | {
      readonly status: 'rejected';
      readonly reason:
        | 'foreign-command'
        | 'consumed-command'
        | 'stale-source'
        | 'stale-context'
        | 'read-only'
        | 'composing'
        | 'disposed';
    };

/** An accepted command contributes exactly one native transaction; no-op commands preserve caret and history. */
type TakenEdit =
  | { readonly status: 'ready'; readonly transaction: TransactionSpec }
  | Exclude<SourceEditOutcome, { readonly status: 'applied' | 'unchanged' }>
  | { readonly status: 'unchanged' };

/** Only a caller-held opaque handle retains its prepared payload; no strong command registry or source snapshot is kept. */
interface CapturedEdit {
  readonly freshness: EditorFreshness;
  readonly replacement:
    | {
        readonly kind: 'prepared';
        readonly transaction: TransactionSpec;
        readonly unchanged: boolean;
      }
    | { readonly kind: 'reserved'; readonly from: number; readonly to: number };
}

/** One finite editor owner validates and consumes prepared exact edits; CodeMirror remains the transaction/history owner. */
export class PreparedSourceEdits {
  private commands = new WeakMap<PreparedSourceEdit | ReservedSourceEdit, CapturedEdit>();
  private consumed = new WeakSet<PreparedSourceEdit | ReservedSourceEdit>();
  private disposed = false;

  /** Prepare an exact-source UTF-16 range, including CRLF boundary validation; this does not move selection or publish state. */
  prepare(
    state: EditorState,
    composing: boolean,
    from: number,
    to: number,
    source: string,
  ): SourceEditPreparation {
    if (this.disposed) return { status: 'blocked', reason: 'disposed' };
    if (composing) return { status: 'blocked', reason: 'composing' };
    if (state.readOnly) return { status: 'blocked', reason: 'read-only' };
    const transaction = exactReplacement(state, from, to, source);
    const command: PreparedSourceEdit = Object.freeze({
      kind: 'source-edit',
      [sourceEditBrand]: true as const,
    });
    this.commands.set(command, {
      freshness: state.field(editorFreshness),
      replacement: {
        kind: 'prepared',
        transaction,
        unchanged: readExactSource(state).slice(from, to) === source,
      },
    });
    return { status: 'ready', command };
  }

  /** Validate exact-source UTF-16 boundaries now; no source payload is retained while the host works. */
  reserve(state: EditorState, composing: boolean, from: number, to: number): SourceEditReservation {
    if (this.disposed) return { status: 'blocked', reason: 'disposed' };
    if (composing) return { status: 'blocked', reason: 'composing' };
    if (state.readOnly) return { status: 'blocked', reason: 'read-only' };
    exactReplacement(state, from, to, '');
    const command: ReservedSourceEdit = Object.freeze({
      kind: 'source-reservation',
      [sourceEditBrand]: true as const,
    });
    this.commands.set(command, {
      freshness: state.field(editorFreshness),
      replacement: { kind: 'reserved', from, to },
    });
    return { status: 'ready', command };
  }

  /** Resolve host text only after the shared command owner has accepted this reservation's source/context identity. */
  takeReserved(
    state: EditorState,
    composing: boolean,
    command: ReservedSourceEdit,
    source: string,
  ): TakenEdit {
    const captured = this.claim(state, composing, command);
    if ('status' in captured) return captured;
    if (captured.replacement.kind !== 'reserved')
      throw new TypeError('Expected source reservation');
    const { from, to } = captured.replacement;
    return readExactSource(state).slice(from, to) === source
      ? { status: 'unchanged' }
      : {
          status: 'ready',
          transaction: exactReplacement(state, from, to, source),
        };
  }

  /** Restrictions leave the command available; terminal freshness rejection and successful acceptance consume it once. */
  take(state: EditorState, composing: boolean, command: PreparedSourceEdit): TakenEdit {
    const captured = this.claim(state, composing, command);
    if ('status' in captured) return captured;
    if (captured.replacement.kind !== 'prepared')
      throw new TypeError('Expected prepared source edit');
    return captured.replacement.unchanged
      ? { status: 'unchanged' }
      : { status: 'ready', transaction: captured.replacement.transaction };
  }

  /** Both eager edits and deferred host replacements share restrictions, consumption and freshness ownership. */
  private claim(
    state: EditorState,
    composing: boolean,
    command: PreparedSourceEdit | ReservedSourceEdit,
  ): CapturedEdit | Exclude<SourceEditOutcome, { readonly status: 'applied' | 'unchanged' }> {
    if (this.disposed) return { status: 'rejected', reason: 'disposed' };
    const captured = this.commands.get(command);
    if (!captured)
      return {
        status: 'rejected',
        reason: this.consumed.has(command) ? 'consumed-command' : 'foreign-command',
      };
    if (composing) return { status: 'rejected', reason: 'composing' };
    if (state.readOnly) return { status: 'rejected', reason: 'read-only' };
    this.commands.delete(command);
    this.consumed.add(command);
    const current = state.field(editorFreshness);
    const failure = freshnessRejection(current, captured.freshness);
    if (failure) return { status: 'rejected', reason: failure };
    return captured;
  }

  /** Release all retained prepared payloads immediately and prevent future acceptance after teardown. */
  dispose(): void {
    this.disposed = true;
    this.commands = new WeakMap();
    this.consumed = new WeakSet();
  }
}
