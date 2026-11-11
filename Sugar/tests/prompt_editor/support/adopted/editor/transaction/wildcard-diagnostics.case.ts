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
/** Real native state proves indexed diagnostic reuse, active editing and source/catalog retirement without duplicating browser input. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, StateEffect, type EditorState } from '@codemirror/state';
import type { Decoration } from '@codemirror/view';
import { undoDepth } from '@codemirror/commands';
import { prepareWildcardCatalog } from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import { wildcardCatalog } from '../../../../../../frontend/sugarprompt/editor/src/wildcard-catalog.ts';
import {
  richProjection,
  installProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  contextMenu,
  installContextMenu,
  type ContextMenu,
} from '../../../../../../frontend/sugarprompt/editor/src/context-menu-state.ts';
import { wildcardMenuTargetAt } from '../../../../../../frontend/sugarprompt/editor/src/wildcard-menu-target.ts';
import { prepareContextMenu } from '../../../../../../frontend/sugarprompt/editor/src/context-menu-preparation.ts';
import { exactReplacement } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { projectionState } from './projection-state.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import type { WildcardDiagnosticValue } from '../../../../../../frontend/sugarprompt/editor/src/wildcard-diagnostic-value.ts';

test('CSV diagnostics use mapped source/body bounds and decoded value ends instead of neighboring storage punctuation', () => {
  const source = 'left,right\r\n"🙂a""b""\r\n{missing}","{missing}"';
  const presentation = new Compartment();
  let state = projectionState(source, presentation, 'wildcard-csv').update({
    effects: StateEffect.appendConfig.of(
      wildcardCatalog.of(
        prepareWildcardCatalog({
          status: 'ready',
          revision: 'csv-missing',
          textIdentifiers: [],
          csvSources: [],
        }),
      ),
    ),
  }).state;
  const canonical = prepareNativeStructure(state),
    projection = prepareProjection(state, canonical);
  const findings: {
    from: number;
    to: number;
    value: WildcardDiagnosticValue;
  }[] = [];
  projection.wildcardDiagnostics.between(0, state.doc.length, (from, to, value) => {
    findings.push({ from, to, value });
  });
  const exactStarts = [source.indexOf('{missing}'), source.lastIndexOf('{missing}')];
  const logicalStarts = [
    state.doc.toString().indexOf('{missing}'),
    state.doc.toString().lastIndexOf('{missing}'),
  ];
  assert.equal(findings.length, 2);
  for (const [index, finding] of findings.entries()) {
    assert.deepEqual(finding.value.diagnostic.range, {
      from: exactStarts[index],
      to: exactStarts[index]! + 9,
    });
    assert.deepEqual(
      [finding.from, finding.to, finding.value.bodyFrom, finding.value.bodyTo],
      [
        logicalStarts[index],
        logicalStarts[index]! + 9,
        logicalStarts[index]! + 1,
        logicalStarts[index]! + 8,
      ],
    );
    assert.equal(finding.value.committedAtEnd, false);
  }
  state = state.update({ effects: installProjection.of(projection) }).state;
  state = state.update({ selection: { anchor: findings[0]!.to } }).state;
  assert.equal(state.field(richProjection).mutedWildcard, findings[0]!.value);
  assert.equal(state.field(richProjection).wildcardDiagnostics, projection.wildcardDiagnostics);
  state = state.update({ selection: { anchor: findings[1]!.to } }).state;
  assert.equal(state.field(richProjection).mutedWildcard, findings[1]!.value);
  state = state.update({
    effects: presentation.reconfigure(richRendering.of(false)),
  }).state;
  assert.equal(state.field(richProjection).wildcardDiagnostics.size, 0);
});

/** Mount actual semantic owners and publish their actual prepared findings. */
function prepared() {
  const catalog = new Compartment();
  const presentation = new Compartment();
  let state = projectionState('🙂\r\n{missing}, tail', presentation);
  state = state.update({
    effects: StateEffect.appendConfig.of([
      contextMenu,
      catalog.of(
        wildcardCatalog.of(
          prepareWildcardCatalog({
            status: 'ready',
            revision: 'missing',
            textIdentifiers: [],
            csvSources: [],
          }),
        ),
      ),
    ]),
  }).state;
  state = state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
  return { state, catalog, presentation };
}

/** Observe one actual body descriptor by its prepared metadata identity rather than recreating its rules. */
function bodyMark(state: EditorState): Decoration {
  let mark: Decoration | null = null;
  state.field(richProjection).decorations.between(3, 12, (_from, _to, value) => {
    if (value.spec.wildcardDiagnostic) mark = value;
  });
  assert.ok(mark);
  return mark;
}

test("caret movement reuses indexed findings and unchanged marks while visibility replaces only the active body's descriptor", () => {
  let { state } = prepared();
  const projection = state.field(richProjection),
    target = wildcardMenuTargetAt(state, 4)!,
    initial = bodyMark(state);
  assert.deepEqual(target.finding.diagnostic.range, { from: 4, to: 13 });
  assert.equal(target.from, 3);
  assert.equal(target.to, 12);
  assert.equal(wildcardMenuTargetAt(state, 12), null);
  state = state.update({ selection: { anchor: 3 } }).state;
  assert.equal(bodyMark(state), initial);
  assert.equal(state.field(richProjection).wildcardDiagnostics, projection.wildcardDiagnostics);
  state = state.update({ selection: { anchor: 6 } }).state;
  const muted = bodyMark(state);
  assert.equal(muted.spec.attributes['data-diagnostic-active'], 'true');
  assert.notEqual(muted, initial);
  assert.equal(wildcardMenuTargetAt(state, 4), null);
  state = state.update({ selection: { anchor: 7 } }).state;
  assert.equal(bodyMark(state), muted);
  assert.equal(state.field(richProjection).mutedWildcard, target.finding);
  assert.equal(state.field(richProjection).wildcardDiagnostics, projection.wildcardDiagnostics);
  state = state.update({ selection: { anchor: 12 } }).state;
  assert.equal(bodyMark(state).spec.attributes['data-diagnostic-active'], 'false');
  assert.equal(wildcardMenuTargetAt(state, 4)?.finding, target.finding);
  assert.equal(undoDepth(state), 0);
});

test('derived menus share the actual finding and reject source, catalog, selection and raw presentation retirement', () => {
  for (const kind of ['source', 'catalog', 'selection', 'raw'] as const) {
    const fixture = prepared();
    let { state } = fixture;
    const menu: ContextMenu = {
      ...prepareContextMenu(
        state,
        {
          position: 4,
          inline: null,
          diagnostic: wildcardMenuTargetAt(state, 4)!,
          documentDiagnostic: null,
          duplicateDiagnostic: null,
          spellingDiagnostic: null,
        },
        'local',
      )!,
      anchor: { left: 1, right: 1, top: 1, bottom: 1 },
    };
    state = state.update({ effects: installContextMenu.of(menu) }).state;
    assert.equal(state.field(contextMenu), menu);
    assert.equal(menu.items[0]?.kind, 'disabled');
    state = state.update(
      kind === 'source'
        ? exactReplacement(state, 0, 2, 'cat')
        : kind === 'selection'
          ? { selection: { anchor: 6 } }
          : kind === 'raw'
            ? {
                effects: fixture.presentation.reconfigure(richRendering.of(false)),
              }
            : {
                effects: fixture.catalog.reconfigure(wildcardCatalog.of(null)),
              },
    ).state;
    assert.equal(state.field(contextMenu), null);
    state = state.update({ effects: installContextMenu.of(menu) }).state;
    assert.equal(state.field(contextMenu), null);
    assert.equal(wildcardMenuTargetAt(state, 4), null);
  }
});
