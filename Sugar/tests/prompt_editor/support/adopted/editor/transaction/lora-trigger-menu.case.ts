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
/** Actual scope-qualified LoRA trigger commands preserve model identity and encoded native output. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, StateEffect } from '@codemirror/state';
import { readLoraResources } from '../../../../../../frontend/sugarprompt/services/src/lora/validation.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { loraModelTargetAt } from '../../../../../../frontend/sugarprompt/editor/src/lora-menu-targets.ts';
import {
  installLoraPresentation,
  prepareLoraPresentation,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import {
  defaultPreferences,
  resolveFeatureProfile,
  prepareLoraSchedulingContext,
  SceneLoraSession,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { prepareLoraTriggerMenu } from '../../../../../../frontend/sugarprompt/editor/src/lora-trigger-menu.ts';
import { prepareLoraTriggerEligibility } from '../../../../../../frontend/sugarprompt/editor/src/lora-trigger-eligibility.ts';
import {
  nativeLoraSchedule,
  installLoraSchedule,
  loraSchedulingHost,
  captureLoraSchedulingHost,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-schedule.ts';
import { currentDocumentValueEdit } from '../../../../../../frontend/sugarprompt/editor/src/document-value-edit-issuance.ts';
import {
  installSemanticStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { projectionState } from './projection-state.ts';
import {
  contextMenuTargetAt,
  prepareContextMenu,
} from '../../../../../../frontend/sugarprompt/editor/src/context-menu-preparation.ts';

test('canonical CSV trigger leaves preserve actual model identity, encoded inline scope and refuse cross-value selection', (t) => {
  const source = 'header\n"<lora:first:1>",<lora:second:1>',
    owner = new SceneLoraSession(),
    host = captureLoraSchedulingHost({
      catalogRevision: 'scoped',
      context: prepareLoraSchedulingContext({
        revision: 'scoped',
        catalogRevision: 'scoped',
        mode: 'workflow',
        supported: true,
        upstream: [],
        fields: [],
      }),
    });
  t.after(() => owner.dispose());
  let state = projectionState(source, new Compartment(), 'wildcard-csv');
  state = state.update({
    effects: StateEffect.appendConfig.of([
      nativeLoraSchedule,
      loraSchedulingHost.of(host),
      featureProfile.of(
        resolveFeatureProfile(defaultPreferences, {
          mode: 'workflow',
          loraScheduling: true,
        }),
      ),
    ]),
  }).state;
  const resources = prepareLoraPresentation(state, {
    status: 'ready',
    resources: readLoraResources(
      ['first', 'second'].map((name) => ({
        name,
        resolution: 'found',
        model: { promptName: name, backendValue: `${name}.pt`, basename: name },
        modelPageUrl: null,
        title: name,
        subtitle: null,
        triggerWords: [`${name} "word"`],
        thumbnail: { status: 'unavailable' },
      })),
      ['first', 'second'],
    ),
  });
  state = state.update({
    effects: installLoraPresentation.of(resources),
  }).state;
  const native = prepareNativeStructure(state),
    scopes = owner.prepareDocument(native.loraScopes, resources.snapshot, 'scoped', host.context);
  state = state.update({
    effects: [
      installSemanticStructure.of({ purpose: 'native', prepared: native }),
      installProjection.of(prepareProjection(state, native)),
      installLoraSchedule.of({
        freshness: state.field(editorFreshness),
        host,
        resources,
        scopes,
        hasTriggersAt: prepareLoraTriggerEligibility(state, native.nativeValues, scopes),
      }),
    ],
  }).state;
  const first = loraModelTargetAt(state, source.indexOf('<lora:first'))!,
    second = loraModelTargetAt(state, source.indexOf('<lora:second'))!,
    menu = prepareLoraTriggerMenu(state, first.from, first),
    leaf = [...menu.leaves][0]!;
  assert.equal(menu.leaves.size, 1);
  assert.equal(leaf.label, 'first');
  const aggregateTarget = contextMenuTargetAt(state, first.from)!;
  assert.ok(aggregateTarget.inline);
  const aggregate = prepareContextMenu(state, aggregateTarget, 'field')!;
  assert.equal(aggregate.actions?.kind, 'general');
  assert.deepEqual(
    [...aggregate.actions!.editLeaves].map((item) => item.label),
    ['first'],
  );
  assert.equal(aggregate.items.at(-1), aggregate.renderingItem);
  assert.equal(
    aggregate.items.some(
      (item) => item.kind === 'text' || item.kind === 'page' || item.kind === 'action',
    ),
    false,
  );
  assert.equal(readExactSource(state), source);
  assert.equal(prepareLoraTriggerMenu(state, second.from, second).leaves.size, 1);
  assert.equal([...prepareLoraTriggerMenu(state, second.from, second).leaves][0]!.label, 'second');
  const edit = leaf.replacement;
  assert.equal(edit.transactions[0]!.isUserEvent('input.exact'), true);
  assert.equal(edit.transactions[0]!.scrollIntoView, true);
  assert.equal(
    currentDocumentValueEdit(
      state,
      {
        values: native.nativeValues,
        semantics: native.documentSemantics,
        enabled: true,
        composing: false,
      },
      edit,
    ),
    null,
  );
  assert.equal(
    readExactSource(edit.transactions[0]!.state),
    'header\n"<lora:first:1>, first ""word""",<lora:second:1>',
  );
  const followup = prepareNativeStructure(edit.transactions[0]!.state, edit);
  assert.equal(followup.scopes.scopes[0]!.mapping.text, '<lora:first:1>, first "word"');
  assert.equal(followup.scopes.scopes[1]!.mapping.text, '<lora:second:1>');
  assert.equal(prepareLoraTriggerMenu(state, 0, null).leaves.size, 0);
  const cross = state.update({
    selection: { anchor: first.from, head: second.to },
  }).state;
  assert.equal(prepareLoraTriggerMenu(cross, first.from, null).leaves.size, 0);
  assert.equal(
    currentDocumentValueEdit(
      cross,
      {
        values: native.nativeValues,
        semantics: native.documentSemantics,
        enabled: true,
        composing: false,
      },
      edit,
    ),
    'stale-context',
  );
  assert.equal(
    currentDocumentValueEdit(
      state,
      {
        values: native.nativeValues,
        semantics: native.documentSemantics,
        enabled: true,
        composing: false,
      },
      { ...edit },
    ),
    'foreign',
  );
});
