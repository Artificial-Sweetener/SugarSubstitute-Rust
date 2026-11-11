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
/** Native scheduling authority proves retirement without replacing source, selection or history owners. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, EditorState } from '@codemirror/state';
import { history, undoDepth } from '@codemirror/commands';

import {
  defaultPreferences,
  prepareLoraSchedulingContext,
  resolveFeatureProfile,
  createLoraCatalogAdapter,
  SceneLoraSession,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import {
  exactSourceExtension,
  exactReplacement,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  installLoraPresentation,
  loraPresentation,
  prepareLoraPresentation,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import {
  captureLoraSchedulingHost,
  installLoraSchedule,
  loraSchedulingHost,
  nativeLoraSchedule,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-schedule.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import {
  prepareNativeStructure,
  semanticStructures,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { prepareLoraTriggerEligibility } from '../../../../../../frontend/sugarprompt/editor/src/lora-trigger-eligibility.ts';

test('native selection reuses the actual trigger index while permissions and source retire prepared authority and reject late publication', () => {
  const permissions = new Compartment(),
    host = captureLoraSchedulingHost({
      catalogRevision: 'catalog',
      context: prepareLoraSchedulingContext({
        revision: 'context',
        catalogRevision: 'catalog',
        mode: 'workflow',
        supported: true,
        upstream: [
          {
            model: {
              promptName: 'real',
              backendValue: 'real.pt',
              basename: 'real',
            },
            title: 'Real',
            subtitle: null,
            triggerWords: ['trained_word'],
            thumbnail: { status: 'unavailable' },
            modelPageUrl: null,
          },
        ],
        fields: [],
      }),
    }),
    enabled = resolveFeatureProfile(defaultPreferences, {
      mode: 'workflow',
      loraScheduling: true,
    }),
    disabled = resolveFeatureProfile(defaultPreferences, {
      mode: 'workflow',
      loraScheduling: false,
    }),
    scopeSession = new SceneLoraSession();
  let state = EditorState.create({
    doc: 'cat',
    extensions: [
      exactSourceExtension('cat'),
      editorFreshness,
      semanticStructures,
      history(),
      loraPresentation,
      loraSchedulingHost.of(host),
      permissions.of(featureProfile.of(enabled)),
      nativeLoraSchedule,
    ],
  });
  const resources = prepareLoraPresentation(state, {
    status: 'ready',
    resources: [],
  });
  state = state.update({
    effects: installLoraPresentation.of(resources),
  }).state;
  const native = prepareNativeStructure(state),
    loraScopes = native.loraScopes,
    scopes = scopeSession.prepareDocument(loraScopes, resources.snapshot, 'catalog', host.context),
    snapshot = scopes.at(0)!.snapshot,
    publication = {
      freshness: state.field(editorFreshness),
      host,
      resources,
      scopes,
      hasTriggersAt: prepareLoraTriggerEligibility(state, native.nativeValues, scopes),
    };
  assert.equal(snapshot.status, 'ready');
  state = state.update({ effects: installLoraSchedule.of(publication) }).state;
  state = state.update({ selection: { anchor: 2 } }).state;
  assert.strictEqual(state.field(nativeLoraSchedule), publication);
  if (snapshot.status !== 'ready') throw new Error('Schedule unavailable');
  assert.strictEqual(state.field(nativeLoraSchedule)!.scopes.at(2)!.snapshot, snapshot);
  assert.deepEqual(snapshot.triggers.search('trained'), [
    { word: 'trained_word', display: 'Real', backendValue: 'real.pt' },
  ]);
  state = state.update({
    effects: permissions.reconfigure(featureProfile.of(disabled)),
  }).state;
  assert.equal(state.field(nativeLoraSchedule), null);
  state = state.update({ effects: installLoraSchedule.of(publication) }).state;
  assert.equal(state.field(nativeLoraSchedule), null);
  assert.equal(state.doc.toString(), 'cat');
  assert.equal(state.selection.main.head, 2);
  assert.equal(undoDepth(state), 0);
  state = state.update({
    effects: permissions.reconfigure(featureProfile.of(enabled)),
  }).state;
  state = state.update({ effects: installLoraSchedule.of(publication) }).state;
  assert.equal(state.field(nativeLoraSchedule), null);
  const refreshed = { ...publication, freshness: state.field(editorFreshness) };
  state = state.update({ effects: installLoraSchedule.of(refreshed) }).state;
  assert.strictEqual(state.field(nativeLoraSchedule), refreshed);
  state = state.update(exactReplacement(state, 3, 3, 's')).state;
  assert.equal(state.field(nativeLoraSchedule), null);
  state = state.update({ effects: installLoraSchedule.of(publication) }).state;
  assert.equal(state.field(nativeLoraSchedule), null);
  assert.equal(state.doc.toString(), 'cats');
  assert.equal(undoDepth(state), 1);
  scopeSession.dispose();
});

test('canonical CSV schedules isolate populated values and refuse header, padding, storage and encoding gaps', async (t) => {
  const source = 'header\r\n"  <lora:first:1>\r\n""quoted""  ",<lora:second:1>\r\n,""',
    state = EditorState.create({
      doc: source.replaceAll('\r\n', '\n'),
      extensions: [
        exactSourceExtension(source),
        editorFreshness,
        semanticStructures,
        documentKind.of('wildcard-csv'),
      ],
    }),
    native = prepareNativeStructure(state),
    owner = new SceneLoraSession(),
    catalog = createLoraCatalogAdapter({
      revision: 'values',
      authoritative: true,
      items: ['first', 'second'].map((name) => ({
        model: { promptName: name, backendValue: `${name}.pt`, basename: name },
        title: name,
        subtitle: null,
        triggerWords: [`${name}_word`],
        thumbnail: { status: 'unavailable' as const },
        modelPageUrl: null,
      })),
    });
  t.after(() => owner.dispose());
  const resources = await catalog.resolve(
      native.loraScopes.resourceNames,
      new AbortController().signal,
    ),
    schedules = owner.prepareDocument(
      native.loraScopes,
      { status: 'ready', resources },
      'values',
      prepareLoraSchedulingContext({
        revision: 'values',
        catalogRevision: 'values',
        mode: 'workflow',
        supported: true,
        upstream: [],
        fields: [],
      }),
    ),
    first = schedules.at(source.indexOf('<lora:first')),
    second = schedules.at(source.indexOf('<lora:second')),
    eligible = prepareLoraTriggerEligibility(state, native.nativeValues, schedules);
  assert.ok(first && second);
  assert.notStrictEqual(first, second);
  assert.deepEqual([...first.insertions.keys()], ['first.pt']);
  assert.deepEqual([...second.insertions.keys()], ['second.pt']);
  assert.strictEqual(schedules.at(source.indexOf('quoted')), first);
  assert.equal(schedules.at(0), null);
  assert.equal(schedules.at(source.indexOf('"  ') + 1), null);
  assert.equal(schedules.at(source.indexOf('",<') + 1), null);
  const entry = native.nativeValues.entries.find((item) =>
      item.scope?.mapping.text.includes('first'),
    )!,
    map = entry.semanticMapping!;
  assert.equal(eligible(map.logicalRange.from, 'first.pt'), true);
  assert.equal(eligible(map.logicalRange.from, 'second.pt'), false);
  assert.equal(eligible(map.logicalRange.from - 1), false);
  const doubledQuote = state.doc.toString().indexOf('""quoted');
  assert.equal(eligible(doubledQuote + 1), false);
  assert.equal(schedules.at(source.length - 1)?.insertions.size, 0);
  owner.dispose();
  assert.throws(() => schedules.at(source.indexOf('first')), /disposed/u);
});
