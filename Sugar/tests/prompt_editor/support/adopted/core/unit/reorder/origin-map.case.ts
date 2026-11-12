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
/** Independent exact source intervals prove reorder provenance without native metadata or another grammar owner. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { prepareDocumentValues } from '../../../../../../../frontend/sugarprompt/core/src/document/values.ts';
import { prepareDocumentPromptScopes } from '../../../../../../../frontend/sugarprompt/core/src/document/prompt-scopes.ts';
import { parsePrompt } from '../../../../../../../frontend/sugarprompt/core/src/prompt/parser.ts';
import { prepareValueReorder } from '../../../../../../../frontend/sugarprompt/core/src/reorder/derivation.ts';
import {
  hideReorderChip,
  dropReorderChip,
  moveReorderChip,
} from '../../../../../../../frontend/sugarprompt/core/src/reorder/movement.ts';
import {
  serializeValueReorder,
  serializeReorderPreview,
  reorderChipText,
} from '../../../../../../../frontend/sugarprompt/core/src/reorder/serialization.ts';
import {
  captureReorderSelection,
  mapReorderSelection,
} from '../../../../../../../frontend/sugarprompt/core/src/reorder/selection.ts';
import {
  reorderOriginBoundary,
  atomicOriginPieces,
} from '../../../../../../../frontend/sugarprompt/core/src/reorder/source-pieces.ts';
import { prepare, coverage, images } from './origin-map-fixture.ts';

test('unchanged hard-ending comma carriers conserve every exact byte and identity source fragment', () => {
  for (const source of [
    'a\n, b',
    'a\r, b',
    'a\r\n, b',
    'a\n \t, b',
    '(a\n, b:1), tail',
    '(a\r, b:1), tail',
    '(a\r\n \t, b:1), tail',
  ]) {
    const prepared = prepare(source),
      output = serializeValueReorder(prepared)!;
    assert.equal(output.text, source);
    coverage(prepared, output);
    assert.equal(
      output.origin.pieces.every((piece) => piece.kind === 'unchanged'),
      true,
    );
    const moved = moveReorderChip(prepared, prepared.initial, 1, {
      kind: 'row',
      row: 0,
      insertion: 0,
    })!;
    const changed = serializeValueReorder(prepared, moved)!;
    coverage(prepared, changed);
    assert.equal(
      changed.text.includes(source.includes('\r\n') ? '\r\n' : source.includes('\r') ? '\r' : '\n'),
      true,
    );
  }
});

test('equal-looking moved chips and copied separator styles retain distinct original intervals', () => {
  const prepared = prepare('same, same, 🦊');
  const state = moveReorderChip(prepared, prepared.initial, 1, {
    kind: 'row',
    row: 0,
    insertion: 0,
  })!;
  const output = serializeValueReorder(prepared, state)!;
  assert.equal(output.text, 'same, same, 🦊');
  assert.deepEqual(state.order, [1, 0, 2]);
  assert.deepEqual(images(output, 6, 10), [{ from: 0, to: 4 }]);
  assert.deepEqual(images(output, 0, 4), [{ from: 6, to: 10 }]);
  assert.deepEqual(images(output, 4, 6), [
    { from: 4, to: 6 },
    { from: 10, to: 12 },
  ]);
  assert.deepEqual(images(output, 10, 12), []);
  coverage(prepared, output);
});

test('split generated-shell source identity pairs every reopened exact outer and content occurrence', () => {
  const prepared = prepare('(a,b:1), x');
  const state = moveReorderChip(prepared, prepared.initial, 1, {
    kind: 'row',
    row: 0,
    insertion: 2,
  })!;
  const output = serializeValueReorder(prepared, state)!;
  assert.equal(output.text, '(a:1), x, (b:1)');
  const span = prepared.scope.document.emphasisSpans[0]!;
  assert.deepEqual(
    output.origin.envelopes.map((item) => ({
      outer: item.outerRange,
      content: item.contentRange,
    })),
    [
      { outer: { from: 0, to: 5 }, content: { from: 1, to: 2 } },
      { outer: { from: 10, to: 15 }, content: { from: 11, to: 12 } },
    ],
  );
  assert.equal(
    output.origin.envelopes.every((item) => item.span === span),
    true,
  );
  assert.deepEqual(images(output, 0, 1), [
    { from: 0, to: 1 },
    { from: 10, to: 11 },
  ]);
  assert.deepEqual(images(output, 4, 7), [
    { from: 2, to: 5 },
    { from: 12, to: 15 },
  ]);
  assert.equal(parsePrompt(output.text).emphasisSpans.length, 2);
  coverage(prepared, output);
});

test('nested shell reopen occurrences preserve both original depths and independent equal-weight identities', () => {
  const prepared = prepare('((a,b:1),c:2), tail');
  const state = moveReorderChip(prepared, prepared.initial, 1, {
    kind: 'row',
    row: 0,
    insertion: 3,
  })!;
  const output = serializeValueReorder(prepared, state)!;
  assert.equal(output.text, '((a:1),c:2), tail,((b:1):2)');
  for (const span of prepared.scope.document.emphasisSpans)
    assert.equal(output.origin.envelopes.filter((item) => item.span === span).length, 2);
  for (const occurrence of output.origin.envelopes) {
    assert.equal(
      output.text.slice(occurrence.outerRange.from, occurrence.contentRange.from),
      prepared.scope.document.sourceText.slice(
        occurrence.span.outerRange.from,
        occurrence.span.contentRange.from,
      ),
    );
    assert.equal(
      output.text.slice(occurrence.contentRange.to, occurrence.outerRange.to),
      prepared.scope.document.sourceText.slice(
        occurrence.span.contentRange.to,
        occurrence.span.outerRange.to,
      ),
    );
  }
  const independent = prepare('(a,b:1), (c,d:1)');
  const split = serializeValueReorder(
    independent,
    moveReorderChip(independent, independent.initial, 2, {
      kind: 'row',
      row: 0,
      insertion: 0,
    })!,
  )!;
  assert.deepEqual(
    independent.scope.document.emphasisSpans.map(
      (span) => split.origin.envelopes.filter((item) => item.span === span).length,
    ),
    [1, 2],
  );
  coverage(prepared, output);
  coverage(independent, split);
});

test('literal parentheses, Unicode interiors and intact nested generated shells move as exact original slices', () => {
  const prepared = prepare('\\(🦊\\), keep');
  const output = serializeValueReorder(
    prepared,
    moveReorderChip(prepared, prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 1,
    })!,
  )!;
  assert.equal(output.text, 'keep, \\(🦊\\)');
  assert.deepEqual(images(output, 0, 6), [{ from: 6, to: 12 }]);
  assert.deepEqual(images(output, 2, 4), [{ from: 8, to: 10 }]);
  const nested = prepare('(a:1)word, tail');
  const compiled = serializeValueReorder(
    nested,
    moveReorderChip(nested, nested.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 1,
    })!,
  )!;
  assert.equal(compiled.text, 'tail, (a:1)word');
  assert.equal(compiled.origin.envelopes.length, 0);
  assert.deepEqual(images(compiled, 0, 5), [{ from: 6, to: 11 }]);
  coverage(prepared, output);
  coverage(nested, compiled);
});

test('blank-gap movement distinguishes copied authored CRLF from a derived inserted comma', () => {
  const prepared = prepare('a\r\n\r\nb, c'),
    hidden = hideReorderChip(prepared, prepared.initial, 2)!;
  const state = dropReorderChip(prepared, hidden, 2, {
    kind: 'gap',
    gap: 0,
    blank: 0,
  })!;
  const output = serializeValueReorder(prepared, state)!;
  assert.equal(output.text, 'a\r\nc,\r\nb');
  assert.deepEqual(images(output, 1, 3), [{ from: 1, to: 3 }]);
  assert.deepEqual(images(output, 3, 5), [{ from: 5, to: 7 }]);
  assert.deepEqual(
    output.origin.pieces.filter((piece) => piece.kind === 'derived'),
    [
      {
        kind: 'derived',
        outputRange: { from: 4, to: 5 },
        reason: 'blank-line-comma',
      },
    ],
  );
  const empty = prepare('[SEP|A]\r\nbody\r\n[SEP|B]\r\n[SEP|C]\r\n');
  const moved = dropReorderChip(empty, hideReorderChip(empty, empty.initial, 0)!, 0, {
    kind: 'partition',
    partition: 2,
  })!;
  const compiled = serializeValueReorder(empty, moved)!;
  assert.equal(
    compiled.origin.pieces.some(
      (piece) =>
        piece.kind === 'derived' &&
        piece.reason === 'empty-partition-ending' &&
        compiled.text.slice(piece.outputRange.from, piece.outputRange.to) === '\r\n',
    ),
    true,
  );
  coverage(prepared, output);
  coverage(empty, compiled);
});

test('complete source validation refuses foreign carriers, mismatched text, sparse fragments and partial scalar or CRLF intervals', () => {
  const prepared = prepare('🦊, a\r\nb'),
    initial = prepared.initial;
  const carrier = initial.slots[0]!;
  for (const prefix of [
    { ...initial.prefix, scope: prepare('🦊, a\r\nb').scope },
    { ...initial.prefix, text: 'unowned' },
    {
      ...initial.prefix,
      text: 'unowned',
      pieces: [
        {
          kind: 'derived' as const,
          text: 'unowned',
          reason: 'row-separator' as const,
        },
      ],
    },
    {
      ...initial.prefix,
      text: prepared.scope.document.sourceText.slice(0, 1),
      pieces: [{ kind: 'source' as const, sourceRange: { from: 0, to: 1 } }],
    },
    {
      ...initial.prefix,
      text: '\r',
      pieces: [{ kind: 'source' as const, sourceRange: { from: 5, to: 6 } }],
    },
  ])
    assert.equal(serializeValueReorder(prepared, { ...initial, prefix }), null);
  const sparse = [...carrier.pieces];
  // eslint-disable-next-line @typescript-eslint/no-array-delete -- Deliberately supply a sparse array to prove fail-closed input validation.
  delete sparse[0];
  assert.equal(
    serializeValueReorder(prepared, {
      ...initial,
      slots: [{ ...carrier, pieces: sparse }, ...initial.slots.slice(1)],
    }),
    null,
  );
  assert.equal(
    serializeValueReorder(prepared, {
      ...initial,
      slots: [{ ...carrier, text: carrier.text + ' ' }, ...initial.slots.slice(1)],
    }),
    null,
  );
});

test('directional ranges within separate moved chips map exactly while cross-chip and repeated-shell origins remain explicit multiple images', () => {
  const prepared = prepare('🦊first, second');
  const output = serializeValueReorder(
    prepared,
    moveReorderChip(prepared, prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 1,
    })!,
  )!;
  assert.equal(output.text, 'second, 🦊first');
  const first = captureReorderSelection(prepared, 7, 2)!,
    second = captureReorderSelection(prepared, 9, 15)!;
  assert.deepEqual(mapReorderSelection(prepared, output, first), {
    anchor: 15,
    head: 10,
  });
  assert.deepEqual(mapReorderSelection(prepared, output, second), {
    anchor: 0,
    head: 6,
  });
  assert.deepEqual(images(output, 0, 15), []);
  assert.equal(captureReorderSelection(prepared, 1, 7), null);
  const shell = prepare('(a,b:1), x');
  const split = serializeValueReorder(
    shell,
    moveReorderChip(shell, shell.initial, 1, {
      kind: 'row',
      row: 0,
      insertion: 2,
    })!,
  )!;
  assert.equal(images(split, 0, 1).length, 2);
  coverage(prepared, output);
});

test('preview has dense owned output coverage but omits the held chip and cannot become complete editable output', () => {
  const prepared = prepare('first, second\r\n'),
    hidden = hideReorderChip(prepared, prepared.initial, 1)!;
  const preview = serializeReorderPreview(prepared, hidden)!;
  assert.equal(preview.text, 'first\r\n');
  assert.deepEqual(images(preview, 7, 13), []);
  assert.equal(serializeValueReorder(prepared, hidden), null);
  coverage(prepared, preview);
  assert.equal(
    prepareValueReorder(
      prepareDocumentPromptScopes(prepareDocumentValues('other', 'prompt'), {
        style: 'curly',
      }),
      prepared.scope,
      prepared,
    ),
    null,
  );
});

test('nested source-owned padding retains its actual enclosing position in output and chip geometry', () => {
  for (const source of [' ( ( a,b:1), c :2), tail', '((a,b:1) , c:2), tail', ' (a,b :1) , tail']) {
    const prepared = prepare(source),
      output = serializeValueReorder(prepared)!;
    assert.equal(output.text, source);
    assert.equal(
      output.origin.pieces.every((piece) => piece.kind === 'unchanged'),
      true,
    );
    coverage(prepared, output);
    if (source === ' ( ( a,b:1), c :2), tail')
      assert.equal(reorderChipText(prepared.chips[0]!), ' ( ( a:1):2)');
    for (const chip of prepared.chips) {
      const proxy = reorderChipText(chip);
      assert.equal(proxy.includes(chip.text), true);
      assert.equal(parsePrompt(proxy).emphasisSpans.length, chip.envelopes.length);
    }
    const moved = moveReorderChip(prepared, prepared.initial, 1, {
      kind: 'row',
      row: 0,
      insertion: 2,
    })!;
    coverage(prepared, serializeValueReorder(prepared, moved)!);
  }
});

test('genuine hard-line movement joins standalone CR and LF with atomic provenance and typed interior ambiguity', () => {
  const prepared = prepare('alpha\r\nbeta\rgamma\n'),
    initial = serializeValueReorder(prepared)!;
  assert.deepEqual(images(initial, 11, 12), [{ from: 11, to: 12 }]);
  assert.deepEqual(images(initial, 17, 18), [{ from: 17, to: 18 }]);
  const state = moveReorderChip(prepared, prepared.initial, 2, {
    kind: 'row',
    row: 0,
    insertion: 0,
  })!;
  const output = serializeValueReorder(prepared, state)!;
  assert.equal(output.text, 'gamma, alpha\r\nbeta\r\n');
  assert.deepEqual(
    output.origin.pieces.filter((piece) => piece.kind === 'composed'),
    [
      {
        kind: 'composed',
        outputRange: { from: 18, to: 20 },
        components: [
          { kind: 'copied', sourceRange: { from: 11, to: 12 } },
          { kind: 'copied', sourceRange: { from: 17, to: 18 } },
        ],
      },
    ],
  );
  coverage(prepared, output);
  assert.deepEqual(reorderOriginBoundary(output, 12, -1), {
    status: 'observed',
    images: [{ kind: 'interior', unitRange: { from: 18, to: 20 } }],
  });
  assert.deepEqual(reorderOriginBoundary(output, 11, 1), {
    status: 'observed',
    images: [{ kind: 'boundary', position: 18 }],
  });
  assert.deepEqual(reorderOriginBoundary(output, 18, -1), {
    status: 'observed',
    images: [{ kind: 'boundary', position: 20 }],
  });
  assert.deepEqual(reorderOriginBoundary(output, 6, 1), {
    status: 'rejected',
    reason: 'invalid-source-boundary',
  });
  assert.deepEqual(
    atomicOriginPieces('😊', [
      {
        kind: 'unchanged',
        sourceRange: { from: 0, to: 1 },
        outputRange: { from: 0, to: 1 },
      },
      {
        kind: 'copied',
        sourceRange: { from: 4, to: 5 },
        outputRange: { from: 1, to: 2 },
      },
    ]),
    [
      {
        kind: 'composed',
        outputRange: { from: 0, to: 2 },
        components: [
          { kind: 'unchanged', sourceRange: { from: 0, to: 1 } },
          { kind: 'copied', sourceRange: { from: 4, to: 5 } },
        ],
      },
    ],
  );
});
