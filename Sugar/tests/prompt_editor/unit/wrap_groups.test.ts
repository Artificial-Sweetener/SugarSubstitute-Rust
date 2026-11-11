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
/** Preserve Substitute's tag threshold, decoration attachments and full-width fallback policy. */
import { ChangeSet, Text } from '@codemirror/state';
import { Decoration } from '@codemirror/view';
import {
  SourceToken,
  EmphasisWidget,
} from '../../../frontend/sugarprompt/editor/src/projection-widgets.js';
import {
  preparePromptWrapGroups,
  fittingPromptWrapGroups,
  mapPromptWrapRange,
} from '../../../frontend/sugarprompt/editor/src/prompt-wrap-groups.js';

test('protect one to three word comma tags, include the comma, and respect hard lines', () => {
  const text =
    '  best quality , greco roman clothes, one two three four, final tag\nplain prose\nlast,\n';
  const groups = preparePromptWrapGroups(text, Decoration.none);
  expect(
    groups.filter((group) => group.kind === 'tag').map((group) => text.slice(group.from, group.to)),
  ).toEqual(['best quality ,', 'greco roman clothes,', 'final tag', 'last,']);
  expect(
    preparePromptWrapGroups('unseparated two words', Decoration.none).filter(
      (group) => group.kind === 'tag',
    ),
  ).toEqual([]);
});

test('never force an unmeasured group onto one line', () => {
  expect(
    fittingPromptWrapGroups(
      preparePromptWrapGroups('a very long word, tail', Decoration.none),
      new Map(),
      100,
    ),
  ).toEqual([]);
});

test('derive bounded edge words from existing widgets, including the closing comma', () => {
  const text = 'alpha, (holding large wooden staff:1.60), tail';
  const token = new SourceToken();
  const prefix = text.indexOf('('),
    suffix = text.indexOf(':1.60');
  const decorations = Decoration.set([
    Decoration.replace({ widget: new EmphasisWidget(token, 'prefix') }).range(prefix, prefix + 1),
    Decoration.replace({ widget: new EmphasisWidget(token, 'suffix', '1.60') }).range(
      suffix,
      suffix + ':1.60)'.length,
    ),
  ]);
  const groups = preparePromptWrapGroups(text, decorations);
  expect(
    groups
      .filter((group) => group.kind === 'leading' || group.kind === 'trailing')
      .map((group) => text.slice(group.from, group.to)),
  ).toEqual(['(holding', 'staff:1.60),']);
});

test('prefer the whole decorated short tag and fall back to fitting edge groups if it is oversized', () => {
  const text = 'alpha, (best quality:1.20), tail';
  const token = new SourceToken(),
    prefix = text.indexOf('('),
    suffix = text.indexOf(':1.20');
  const groups = preparePromptWrapGroups(
    text,
    Decoration.set([
      Decoration.replace({ widget: new EmphasisWidget(token, 'prefix') }).range(prefix, prefix + 1),
      Decoration.replace({ widget: new EmphasisWidget(token, 'suffix', '1.20') }).range(
        suffix,
        suffix + ':1.20)'.length,
      ),
    ]),
  );
  const widths = new Map(
    groups.map(
      (group) => [group, group.kind === 'tag' && group.from === prefix ? 140 : 40] as const,
    ),
  );
  expect(fittingPromptWrapGroups(groups, widths, 200).map((group) => group.kind)).toEqual([
    'tag',
    'tag',
    'tag',
  ]);
  const fallback = fittingPromptWrapGroups(groups, widths, 100);
  expect(fallback.map((group) => group.kind)).toEqual(['tag', 'leading', 'trailing', 'tag']);
  expect(fittingPromptWrapGroups(groups, widths, 20)).toEqual([]);
});

test('never consume text outside a decorated single-word body or a hard line', () => {
  const text = '(wind:2.00), next\nline';
  const token = new SourceToken();
  const groups = preparePromptWrapGroups(
    text,
    Decoration.set([
      Decoration.replace({ widget: new EmphasisWidget(token, 'prefix') }).range(0, 1),
      Decoration.replace({ widget: new EmphasisWidget(token, 'suffix', '2.00') }).range(5, 11),
    ]),
  );
  expect(
    groups
      .filter((group) => group.kind === 'leading' || group.kind === 'trailing')
      .map((group) => text.slice(group.from, group.to)),
  ).toEqual(['(wind', 'wind:2.00),']);
  expect(groups.every((group) => !text.slice(group.from, group.to).includes('\n'))).toBe(true);
});

test('retain pinned Unicode words and joined punctuation without keeping oversized words', () => {
  const text = 'alpha, one two long-sleeved 三色毛 hair extra, tail';
  const groups = preparePromptWrapGroups(text, Decoration.none);
  expect(
    groups
      .filter((group) => group.kind === 'word')
      .map((group) => text.slice(group.from, group.to)),
  ).toEqual(['alpha', 'one', 'two', 'long-sleeved', '三色毛', 'hair', 'extra', 'tail']);
  const widths = new Map(
    groups.map(
      (group) => [group, text.slice(group.from, group.to) === 'long-sleeved' ? 150 : 30] as const,
    ),
  );
  expect(
    fittingPromptWrapGroups(groups, widths, 100).some(
      (group) => text.slice(group.from, group.to) === 'long-sleeved',
    ),
  ).toBe(false);
  const separated = 'alpha, one\u0085two\u0085three\u0085four, omega';
  expect(
    preparePromptWrapGroups(separated, Decoration.none)
      .filter((group) => group.kind === 'tag')
      .map((group) => separated.slice(group.from, group.to)),
  ).toEqual(['alpha,', 'omega']);
});

test('exclude words covered by nested concealed ranges without losing later visible words', () => {
  const text = 'alpha hidden nested visible omega';
  const groups = preparePromptWrapGroups(
    text,
    Decoration.set([
      Decoration.replace({ widget: new EmphasisWidget(new SourceToken(), 'prefix') }).range(6, 19),
      Decoration.replace({ widget: new EmphasisWidget(new SourceToken(), 'prefix') }).range(13, 19),
    ]),
  );
  expect(groups.map((group) => text.slice(group.from, group.to))).toEqual([
    'alpha',
    'visible',
    'omega',
  ]);
});

test('carry a kept tag through a weight step that rewrites its whole emphasis', () => {
  const text = 'hand on headwear, (oversized brim:-1.05),';
  const tag = { from: text.indexOf('('), to: text.length };
  const emphasis = { from: tag.from, to: text.indexOf(')') + 1 };
  const changes = ChangeSet.of(
    [{ from: emphasis.from, to: emphasis.to, insert: '(oversized brim:-1.00)' }],
    text.length,
  );
  const mapped = mapPromptWrapRange(tag, changes);
  const after = changes.apply(Text.of([text])).toString();
  expect(mapped && after.slice(mapped.from, mapped.to)).toBe('(oversized brim:-1.00),');
  const interior = ChangeSet.of(
    [{ from: text.indexOf('05'), to: text.indexOf('05') + 2, insert: '1' }],
    text.length,
  );
  const narrowed = mapPromptWrapRange(tag, interior);
  expect(narrowed && interior.apply(Text.of([text])).sliceString(narrowed.from, narrowed.to)).toBe(
    '(oversized brim:-1.1),',
  );
});

test('leave edge insertions outside a kept group and drop groups an edit crosses or splits', () => {
  const text = 'alpha, best quality, tail';
  const group = { from: text.indexOf('best'), to: text.indexOf(' tail') };
  const length = text.length;
  expect(
    mapPromptWrapRange(group, ChangeSet.of([{ from: group.from, insert: 'x' }], length)),
  ).toEqual({
    from: group.from + 1,
    to: group.to + 1,
  });
  expect(
    mapPromptWrapRange(group, ChangeSet.of([{ from: group.to, insert: 'x' }], length)),
  ).toEqual(group);
  expect(
    mapPromptWrapRange(group, ChangeSet.of([{ from: group.from - 2, to: group.from + 2 }], length)),
  ).toBeNull();
  expect(
    mapPromptWrapRange(group, ChangeSet.of([{ from: group.from + 4, insert: '\n' }], length)),
  ).toBeNull();
  expect(
    mapPromptWrapRange(group, ChangeSet.of([{ from: group.from, to: group.to }], length)),
  ).toBeNull();
});
