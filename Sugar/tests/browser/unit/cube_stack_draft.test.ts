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
/** Prove cart staging preserves existing identities and remains reversible until Apply. */
import { expect, test } from '@jest/globals';
import { CubeStackDraft } from '../../../frontend/comfyui/ui/picker/domain/CubeStackDraft.js';

test('opens with exact current instances and stages repeated definitions as distinct additions', () => {
  const draft = setup();
  expect(draft.changed).toBe(false);
  const first = draft.add('catalog-key');
  const second = draft.add('catalog-key');
  expect(first).not.toBe(second);
  expect(draft.entries).toEqual([
    { source: 'existing', draftId: 'existing:first', instanceId: 'first' },
    { source: 'existing', draftId: 'existing:second', instanceId: 'second' },
    { source: 'new', draftId: first, catalogKey: 'catalog-key' },
    { source: 'new', draftId: second, catalogKey: 'catalog-key' },
  ]);
  expect(draft.changed).toBe(true);
});

test('moves and removes references while Reset restores the exact original objects', () => {
  const draft = setup();
  const original = draft.entries;
  const added = draft.add('catalog-key', 'existing:second');
  draft.move('existing:second', 'existing:first');
  draft.remove('existing:first');
  expect(draft.entries.map((entry) => entry.draftId)).toEqual(['existing:second', added]);
  draft.reset();
  expect(draft.entries).toBe(original);
  expect(draft.changed).toBe(false);
});

test('moving back to the original order clears the change flag', () => {
  const draft = setup();
  draft.move('existing:first', null);
  expect(draft.changed).toBe(true);
  draft.move('existing:first', 'existing:second');
  expect(draft.changed).toBe(false);
});

test('rejects stale destinations and malformed permutations without altering the draft', () => {
  const draft = setup();
  const original = draft.entries;
  expect(() => draft.add('catalog-key', 'missing')).toThrow('unavailable');
  expect(() => draft.move('existing:first', 'missing')).toThrow('unavailable');
  expect(() => draft.remove('missing')).toThrow('unavailable');
  expect(() => draft.reorder(['existing:first', 'existing:first'])).toThrow(
    'exact draft permutation',
  );
  expect(draft.entries).toBe(original);
});

test('holds immutable entries and detects identity provider collisions', () => {
  const draft = new CubeStackDraft(['first'], () => 'collision');
  draft.add('catalog-key');
  expect(() => draft.add('catalog-key')).toThrow('duplicate');
  expect(Object.isFrozen(draft.entries)).toBe(true);
  expect(draft.entries.every(Object.isFrozen)).toBe(true);
});

test('rejects ambiguous initial instances and empty exact artifact identities', () => {
  expect(() => new CubeStackDraft(['first', 'first'], () => 'id')).toThrow('duplicate');
  expect(() => new CubeStackDraft([''], () => 'id')).toThrow('empty');
  expect(() => setup().add('')).toThrow('empty');
});

/** Supply deterministic unique IDs without exposing browser UUID generation to domain logic. */
function setup(): CubeStackDraft {
  let next = 0;
  return new CubeStackDraft(['first', 'second'], () => String(++next));
}
