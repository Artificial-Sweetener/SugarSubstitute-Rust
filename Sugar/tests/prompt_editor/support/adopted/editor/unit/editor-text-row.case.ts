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
/** Actual CodeMirror theme composition retains the independently declared native text-row contract. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, EditorSelection, EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { history, undoDepth } from '@codemirror/commands';
import { editorTheme } from '../../../../../../frontend/sugarprompt/editor/src/editor-theme.ts';

/** Match the exact selector after its emitted theme scope, retaining grouped rules and excluding qualified descendants. */
function rule(state: EditorState, selector: string): string {
  const rules = state
    .facet(EditorView.styleModule)
    .flatMap((module) => module.getRules().split('\n'));
  const matches = rules.filter((value) => {
    const opening = value.indexOf(' {');
    return value
      .slice(0, opening)
      .split(', ')
      .some((scoped) => scoped.slice(scoped.indexOf(' ') + 1) === selector);
  });
  assert.equal(matches.length, 1);
  return matches[0]!;
}

test('default Segoe UI 14 text has one content inset and the actual donor 19px row', () => {
  const state = EditorState.create({ extensions: [editorTheme] });
  const content = rule(state, '.cm-content');
  assert.match(content, /line-height: 19px;/u);
  assert.match(content, /padding: 6px 7px 6px 12px;/u);
  assert.match(content, /padding-left: calc\(12px \+ var\(--sugarprompt-content-inset, 0px\)\);/u);
  assert.match(rule(state, '.cm-line'), /padding: 0;/u);
  assert.match(rule(state, '.sp-source-line::before'), /right: -7px;/u);
  const root = state
    .facet(EditorView.styleModule)
    .map((module) => module.getRules())
    .join('\n');
  assert.match(root, /font-size: 14px; font-family: 'Segoe UI', sans-serif;/u);
});

test('emphasis reserves its small raised grid inside the text row and keeps native value typography', () => {
  const state = EditorState.create({ extensions: [editorTheme] });
  const weight = rule(state, '.sp-emphasis-weight');
  assert.match(weight, /display: inline-grid;/u);
  assert.match(weight, /vertical-align: top;/u);
  assert.match(weight, /font-size: max\(6px, calc\(1em - 4px\)\);/u);
  assert.match(weight, /font-weight: 600;/u);
  assert.match(weight, /line-height: normal;/u);
  assert.match(
    rule(state, '.sp-emphasis-parenthesis'),
    /vertical-align: 1px; line-height: normal;/u,
  );
  assert.match(rule(state, '.sp-weight-value'), /grid-area: 1 \/ 1; padding: 0 .5px;/u);
});

test('native theme reconfiguration preserves directional selection, source and existing history', () => {
  const appearance = new Compartment();
  const initial = EditorState.create({
    doc: 'ab',
    selection: EditorSelection.single(2, 0),
    extensions: [history(), appearance.of([])],
  });
  const edited = initial.update({
    changes: { from: 2, insert: 'c' },
    selection: EditorSelection.single(3, 1),
  }).state;
  const changed = edited.update({
    effects: appearance.reconfigure(editorTheme),
  });
  assert.equal(changed.docChanged, false);
  assert.equal(changed.state.doc, edited.doc);
  assert.equal(changed.state.selection, edited.selection);
  assert.equal(changed.state.doc.toString(), 'abc');
  assert.deepEqual(
    [changed.state.selection.main.anchor, changed.state.selection.main.head],
    [3, 1],
  );
  assert.equal(undoDepth(changed.state), 1);
  assert.equal(undoDepth(changed.state), undoDepth(edited));
});

/** Actual emitted declarations delegate every semantic error decoration to the public host color. */
test('error text and all three underline roles consume the same application semantic foreground', () => {
  const state = EditorState.create({ extensions: [editorTheme] });
  for (const selector of [
    '.sp-unsupported-scene',
    '.sp-duplicate-segment',
    '.sp-spelling-diagnostic[data-diagnostic-active="false"]',
  ]) {
    assert.equal(
      rule(state, selector).includes(
        'text-decoration: underline wavy var(--sugarprompt-error-foreground, #b42318);',
      ),
      true,
    );
    assert.equal(rule(state, selector).includes('text-underline-offset: 3px;'), true);
  }
});

/** The Scene layer owns solid fill; line pseudo elements remain independent source chrome and regional rails. */
test('Scene fill does not consume either native line pseudo element', () => {
  const state = EditorState.create({ extensions: [editorTheme] });
  const scene = rule(state, '.sp-scene-fill-paint'),
    chrome = rule(state, '.sp-source-line::before');
  assert.match(scene, /position: absolute;/u);
  assert.match(scene, /background-color: var\(--sp-scene-fill\);/u);
  assert.match(chrome, /z-index: -1;/u);
  assert.match(chrome, /left: calc\(-12px - var\(--sugarprompt-content-inset, 0px\)\);/u);
  assert.match(chrome, /right: -7px;/u);
  assert.match(chrome, /top: 0; bottom: 0;/u);
  assert.equal(
    state
      .facet(EditorView.styleModule)
      .some((module) => module.getRules().includes('.sp-scene-fill::after')),
    false,
  );
});
