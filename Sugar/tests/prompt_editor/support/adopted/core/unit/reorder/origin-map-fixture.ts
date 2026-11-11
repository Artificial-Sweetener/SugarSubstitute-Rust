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
/** Genuine canonical scope setup and shared exact-output observations for origin-map proof. */
import assert from 'node:assert/strict';
import { prepareDocumentValues } from '../../../../../../../frontend/sugarprompt/core/src/document/values.ts';
import { prepareDocumentPromptScopes } from '../../../../../../../frontend/sugarprompt/core/src/document/prompt-scopes.ts';
import { prepareValueReorder } from '../../../../../../../frontend/sugarprompt/core/src/reorder/derivation.ts';
import { isReorderBoundary } from '../../../../../../../frontend/sugarprompt/core/src/reorder/source-pieces.ts';
import type {
  PreparedValueReorder,
  ReorderOutput,
} from '../../../../../../../frontend/sugarprompt/core/src/reorder/contracts.ts';

/** Obtain only actual canonical scope operands from their sole preparation owner. */
export function prepare(source: string): PreparedValueReorder {
  const scopes = prepareDocumentPromptScopes(prepareDocumentValues(source, 'prompt'), {
    style: 'curly',
  });
  return prepareValueReorder(scopes, scopes.scopes[0]!)!;
}

/** Independently verify dense output coverage and exact source slices, including scalar and CRLF endpoints. */
export function coverage(prepared: PreparedValueReorder, output: ReorderOutput): void {
  let end = 0;
  assert.equal(output.origin.scope, prepared.scope);
  assert.equal(Object.isFrozen(output.origin.pieces), true);
  for (const piece of output.origin.pieces) {
    assert.equal(piece.outputRange.from, end);
    assert.ok(piece.outputRange.to > piece.outputRange.from);
    assert.equal(isReorderBoundary(output.text, piece.outputRange.from), true);
    assert.equal(isReorderBoundary(output.text, piece.outputRange.to), true);
    assert.equal(Object.isFrozen(piece), true);
    if (piece.kind === 'composed') {
      assert.equal(piece.outputRange.to - piece.outputRange.from, 2);
      assert.equal(
        piece.components
          .map((component) =>
            component.kind === 'derived'
              ? component.text
              : prepared.scope.document.sourceText.slice(
                  component.sourceRange.from,
                  component.sourceRange.to,
                ),
          )
          .join(''),
        output.text.slice(piece.outputRange.from, piece.outputRange.to),
      );
      for (const component of piece.components)
        if (component.kind !== 'derived') {
          assert.equal(
            isReorderBoundary(prepared.scope.document.sourceText, component.sourceRange.from),
            true,
          );
          assert.equal(
            isReorderBoundary(prepared.scope.document.sourceText, component.sourceRange.to),
            true,
          );
        }
    } else if (piece.kind !== 'derived') {
      assert.equal(
        output.text.slice(piece.outputRange.from, piece.outputRange.to),
        prepared.scope.document.sourceText.slice(piece.sourceRange.from, piece.sourceRange.to),
      );
      assert.equal(
        piece.kind === 'unchanged',
        piece.sourceRange.from === piece.outputRange.from &&
          piece.sourceRange.to === piece.outputRange.to,
      );
    }
    end = piece.outputRange.to;
  }
  assert.equal(end, output.text.length);
}

/** Select one actual source-origin interval, retaining all output images rather than inventing a dense monotonic map. */
export function images(
  output: ReorderOutput,
  from: number,
  to: number,
): readonly { from: number; to: number }[] {
  return output.origin.pieces.flatMap((piece) =>
    piece.kind !== 'derived' &&
    piece.kind !== 'composed' &&
    piece.sourceRange.from <= from &&
    to <= piece.sourceRange.to
      ? [
          {
            from: piece.outputRange.from + from - piece.sourceRange.from,
            to: piece.outputRange.from + to - piece.sourceRange.from,
          },
        ]
      : [],
  );
}
