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
/** Bounded command publication adapts native metadata before the decoded edit is issued. */
import { requireValue } from '../../core/src/required.js';
import {
  Transaction,
  type StateEffect,
  type Annotation,
  type EditorState,
} from '@codemirror/state';
import {
  pasteImportAnnotations,
  pasteImportUserEvent,
  type PasteImportHistoryIntent,
} from './paste-import-history.js';
import { normalizationJoin } from './input-descriptor.js';
import { authoredNesting } from './normalization-events.js';
import type { DocumentValueNormalizationProvenance } from './document-value-ingestion.js';
import { installAuthorship } from './authorship.js';
import type { DocumentValueReorderProvenance } from './reorder-provenance.js';
import {
  generatedShells,
  literalIntents,
  type Authorship,
  type GeneratedShell,
  type LiteralIntent,
} from './authorship-ranges.js';
/** Existing command owners supply a purpose and one actual prepared authorship effect. */
export type DocumentValueEditPublication = {
  readonly authorship?: StateEffect<Authorship>;
  readonly normalization?: DocumentValueNormalizationProvenance;
  readonly reorder?: DocumentValueReorderProvenance;
  readonly pasteHistory?: PasteImportHistoryIntent;
} & (
  | {
      readonly purpose:
        | 'completion'
        | 'weight'
        | 'region'
        | 'trigger'
        | 'paste'
        | 'reorder'
        | 'duplicate'
        | 'spelling';
    }
  | {
      readonly purpose: 'typed';
      readonly input: {
        readonly time: number;
        readonly userEvent: string;
        readonly authoredImplicitDepth: number | null;
      };
    }
);
/** Source-independent metadata is fully resolved before the actual transaction is constructed. */
interface PreparedDocumentValuePublication {
  readonly effects: readonly StateEffect<Authorship>[];
  readonly userEvent: string | undefined;
  readonly scrollIntoView: boolean;
  readonly annotations?: readonly Annotation<unknown>[];
}
/** Only supported command metadata enters the same actual transaction as its encoded source. */
export function prepareDocumentValuePublication(
  publication?: DocumentValueEditPublication,
  state?: EditorState,
): PreparedDocumentValuePublication {
  const effect = publication?.authorship;
  if (effect && !effect.is(installAuthorship))
    throw new Error('decoded-edit-authorship-effect-mismatch');
  if (publication?.pasteHistory) {
    if (publication.purpose !== 'paste' || !state) throw new Error('decoded-edit-paste-history');
    return {
      effects: effect ? [effect] : [],
      userEvent: pasteImportUserEvent(publication.pasteHistory),
      scrollIntoView: false,
      annotations: pasteImportAnnotations(state, publication.pasteHistory),
    };
  }
  if (publication?.purpose === 'typed') {
    const input = publication.input,
      normalization = publication.normalization?.normalization;
    if (
      !normalization ||
      !Number.isFinite(input.time) ||
      !/^(?:input\.type|delete)(?:\.[A-Za-z]+)*$/u.test(input.userEvent) ||
      (input.authoredImplicitDepth !== null &&
        (!Number.isSafeInteger(input.authoredImplicitDepth) || input.authoredImplicitDepth < 2))
    )
      throw new Error('decoded-edit-typed-publication');
    const depth = authoredNesting(normalization);
    if (input.authoredImplicitDepth !== null && depth[0]?.value !== input.authoredImplicitDepth)
      throw new Error('decoded-edit-typed-depth');
    return {
      effects: effect ? [effect] : [],
      userEvent: input.userEvent,
      scrollIntoView: false,
      annotations: [
        normalizationJoin.of(true),
        Transaction.time.of(input.time),
        ...(input.authoredImplicitDepth === null ? [] : depth),
      ],
    };
  }
  return {
    effects: effect ? [effect] : [],
    userEvent:
      publication?.purpose === 'spelling'
        ? 'input.spelling'
        : publication?.purpose === 'duplicate'
          ? 'input.duplicate'
          : publication?.purpose === 'reorder'
            ? 'input.reorder'
            : publication?.purpose === 'completion'
              ? 'input.complete'
              : publication?.purpose === 'weight'
                ? 'input.weight'
                : publication?.purpose === 'region'
                  ? 'input.region'
                  : publication?.purpose === 'paste'
                    ? 'input.paste'
                    : 'input.exact',
    scrollIntoView:
      publication?.purpose === 'completion' ||
      publication?.purpose === 'reorder' ||
      publication?.purpose === 'trigger',
  };
}
/** Complete supported payload observations do not confer issuance or retain another mutable metadata owner. */
export type CapturedDocumentValueEffect =
  | {
      readonly kind: 'authorship';
      readonly generated: readonly GeneratedShell[];
      readonly literals: readonly LiteralIntent[];
    }
  | {
      readonly kind: 'array';
      readonly values: readonly unknown[];
    }
  | {
      readonly kind: 'identity';
      readonly value: unknown;
    };
/** Capture public metadata values using their existing owner rather than a generic object serializer. */
export function captureDocumentValueEffect(
  effect: StateEffect<unknown>,
): CapturedDocumentValueEffect {
  if (effect.is(installAuthorship))
    return Object.freeze({
      kind: 'authorship',
      generated: generatedShells(effect.value),
      literals: literalIntents(effect.value),
    });
  return Array.isArray(effect.value)
    ? Object.freeze({ kind: 'array', values: Object.freeze([...effect.value]) })
    : Object.freeze({ kind: 'identity', value: effect.value });
}
/** Native payload identity is separately fenced; compare every supported public value without coercion. */
export function documentValueEffectMatches(
  effect: StateEffect<unknown>,
  captured: CapturedDocumentValueEffect,
): boolean {
  if (captured.kind === 'identity') return Object.is(effect.value, captured.value);
  if (captured.kind === 'array')
    return (
      Array.isArray(effect.value) &&
      effect.value.length === captured.values.length &&
      captured.values.every(
        (value, index) =>
          Object.hasOwn(effect.value as unknown[], index) &&
          Object.is((effect.value as unknown[])[index], value),
      )
    );
  if (!effect.is(installAuthorship)) return false;
  const generated = generatedShells(effect.value),
    literals = literalIntents(effect.value);
  return (
    generated.length === captured.generated.length &&
    generated.every((shell, index) => {
      const expected = requireValue(captured.generated[index], 'document-value-edit-publication');
      return (
        Object.is(shell.from, expected.from) &&
        Object.is(shell.to, expected.to) &&
        Object.is(shell.contentFrom, expected.contentFrom) &&
        Object.is(shell.contentTo, expected.contentTo) &&
        Object.is(shell.nestingDepth, expected.nestingDepth)
      );
    }) &&
    literals.length === captured.literals.length &&
    literals.every(
      (intent, index) =>
        Object.is(
          intent.from,
          requireValue(captured.literals[index], 'document-value-edit-publication').from,
        ) &&
        Object.is(
          intent.to,
          requireValue(captured.literals[index], 'document-value-edit-publication').to,
        ),
    )
  );
}
