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
/** Public theme output owns one solid fill, ready wash and above-media ring with exact donor alpha. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { loraTheme } from '../../../../../../frontend/sugarprompt/editor/src/lora-theme.ts';
/** Consume emitted public CSS, preserving runtime property order and selectors. */
function rule(selector: string): string {
  const rows = EditorState.create({ extensions: [loraTheme] })
    .facet(EditorView.styleModule)
    .flatMap((module) => module.getRules().split('\n'));
  const matches = rows.filter(
    (row) => row.slice(0, row.indexOf(' {')).replace(/^\.\S+ /u, '') === selector,
  );
  assert.equal(matches.length, 1);
  return matches[0]!;
}
test('one fallback fill and above-media inactive or active ring use exact solid alpha', () => {
  assert.match(rule('.sp-lora-backing'), /background: var\(--sugarprompt-lora-surface, Canvas\);/u);
  const fill = rule('.sp-lora-backing::before'),
    ring = rule('.sp-lora-backing::after');
  assert.match(fill, /z-index: 1;/u);
  assert.match(
    fill,
    /light-dark\(rgb\(from var\(--sp-lora-accent\) r g b \/ calc\(32 \/ 255\)\), rgb\(from var\(--sp-lora-accent\) r g b \/ calc\(46 \/ 255\)\)\)/u,
  );
  assert.match(ring, /z-index: 2;/u);
  assert.match(ring, /rgb\(from var\(--sp-lora-accent\) r g b \/ calc\(104 \/ 255\)\)/u);
  assert.match(ring, /polygon\(evenodd,/u);
  assert.match(ring, /inset: 0;/u);
  assert.match(rule('.sp-lora-active .sp-lora-backing::after'), /calc\(156 \/ 255\)/u);
  assert.match(
    rule('.sp-lora-chip[data-media=ready] .sp-lora-backing'),
    /background: transparent;/u,
  );
  assert.match(
    rule('.sp-lora-chip[data-media=ready] .sp-lora-backing::before'),
    /background: rgba\(0, 0, 0, calc\(64 \/ 255\)\);/u,
  );
  assert.match(rule('.sp-lora-chip'), /text-shadow: none;/u);
  const all = EditorState.create({ extensions: [loraTheme] })
    .facet(EditorView.styleModule)
    .flatMap((module) => module.getRules().split('\n'))
    .join('\n');
  assert.match(all, /--sp-lora-accent: var\(--sugarprompt-error-foreground, #b42318\);/u);
  assert.match(rule('.sp-lora-chip'), /--sp-lora-accent: rgb\(107, 114, 128\);/u);
  assert.match(
    rule('.sp-lora-chip[data-resolution=found]'),
    /--sp-lora-accent: rgb\(0, 120, 212\);/u,
  );
  assert.doesNotMatch(all, /196, 43, 28/u);
  assert.doesNotMatch(
    all,
    /linear-gradient|--sp-lora-fallback-fill|\.sp-lora-chip::before|\.41|\.12/u,
  );
});
