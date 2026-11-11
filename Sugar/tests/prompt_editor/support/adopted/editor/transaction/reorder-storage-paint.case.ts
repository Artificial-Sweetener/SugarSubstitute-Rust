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
/** Genuine Core/native captures compare stored preview literals against exact committed storage, without certifying browser layout. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  dropReorderChip,
  hideReorderChip,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import {
  prepareReorderStoragePaint,
  prepareCapturedReorderStorage,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-storage-paint.ts';
import { prepareReorderCommit } from '../../../../../../frontend/sugarprompt/editor/src/reorder-command.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { captureReorder, reorderState } from './reorder-fixture.ts';

for (const specimen of [
  {
    source: 'name,prompt\r\nrow,"a""b"", c"  \r\nlast,end',
    value: 'c, a""b""',
    position: 18,
  },
  {
    source: 'name,prompt\r\nrow,"(a""b""😀\r\nline:1.25), c"  \r\nlast,end',
    value: 'c, (a""b""😀\r\nline:1.25)',
    position: 19,
  },
]) {
  test('ordinary CSV preview literal paint retains canonical stored quote spelling, padding and exact commit history boundary', () => {
    const { state } = reorderState(
        specimen.source,
        specimen.position,
        specimen.position,
        'wildcard-csv',
      ),
      capture = captureReorder(state),
      hidden = hideReorderChip(capture.prepared, capture.prepared.initial, capture.active)!;
    assert.ok(hidden);
    const moved = dropReorderChip(capture.prepared, hidden, capture.active, {
      kind: 'row',
      row: 0,
      insertion: 1,
    })!;
    assert.ok(moved);
    const preview = prepareReorderPreview(
      { token: {}, capture, activeChip: capture.active, order: moved },
      'complete',
    )!;
    assert.ok(preview);
    const paint = prepareReorderStoragePaint(preview);
    assert.equal(prepareReorderStoragePaint(preview), paint);
    assert.equal(paint.literal({ from: 0, to: preview.output.text.length }), specimen.value);
    assert.deepEqual(paint.replacement, capture.target.entry.semanticMapping!.logicalRange);
    assert.equal(paint.prefix, '');
    assert.equal(paint.suffix, '');
    const committed = prepareReorderCommit(state, capture, moved);
    assert.equal(committed.status, 'ready');
    if (committed.status !== 'ready') throw new Error('reorder-storage-test-commit');
    const final = committed.edit.transactions.at(-1)!.state;
    assert.equal(readExactSource(state), specimen.source);
    const expected = 'name,prompt\r\nrow,"' + specimen.value + '"  \r\nlast,end';
    assert.equal(readExactSource(final), expected);
  });
}

test('JSON-looking prompt escape spellings remain literal source in prepared storage paint', () => {
  const source = String.raw`{"value":"tag\n\u0041"}, b`,
    { state } = reorderState(source, 2),
    capture = captureReorder(state),
    hidden = hideReorderChip(capture.prepared, capture.prepared.initial, capture.active)!,
    moved = dropReorderChip(capture.prepared, hidden, capture.active, {
      kind: 'row',
      row: 0,
      insertion: 1,
    })!,
    preview = prepareReorderPreview(
      { token: {}, capture, activeChip: capture.active, order: moved },
      'complete',
    )!;
  assert.ok(preview);
  const paint = prepareReorderStoragePaint(preview);
  assert.equal(
    paint.literal({ from: 0, to: preview.output.text.length }),
    String.raw`b, {"value":"tag\n\u0041"}`,
  );
  assert.equal(readExactSource(state), source);
});

test('many same-hard-line targets retain one actual captured encoding and dense map across overlapping origins', () => {
  const contents = Array.from({ length: 80 }, (_, at) => `"tag${at}"`).join(', '),
    source = 'name,prompt\nrow,"' + contents.replaceAll('"', '""') + '"',
    position = source.indexOf('""tag0'),
    { state } = reorderState(source, position, position, 'wildcard-csv'),
    capture = captureReorder(state),
    prepared = prepareCapturedReorderStorage(capture)!;
  assert.ok(prepared);
  const map = prepared.mapping,
    decodedBoundaries = map.decodedToSource,
    sourceBoundaries = map.sourceToDecoded,
    hidden = hideReorderChip(capture.prepared, capture.prepared.initial, capture.active)!;
  assert.equal(capture.prepared.chips.length, 80);
  for (const insertion of [1, 40, 79, 1]) {
    const moved = dropReorderChip(capture.prepared, hidden, capture.active, {
        kind: 'row',
        row: 0,
        insertion,
      })!,
      preview = prepareReorderPreview(
        { token: {}, capture, activeChip: capture.active, order: moved },
        'complete',
      )!;
    assert.ok(preview);
    const paint = prepareReorderStoragePaint(preview);
    assert.equal(prepareCapturedReorderStorage(capture), prepared);
    assert.equal(prepareCapturedReorderStorage(capture)!.mapping, map);
    assert.equal(
      prepareCapturedReorderStorage(capture)!.mapping.decodedToSource,
      decodedBoundaries,
    );
    assert.equal(prepareCapturedReorderStorage(capture)!.mapping.sourceToDecoded, sourceBoundaries);
    assert.equal(
      paint.literal({ from: 0, to: preview.output.text.length }),
      preview.output.text.replaceAll('"', '""'),
    );
  }
  assert.equal(readExactSource(state), source);
});
