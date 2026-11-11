import { editorPresentations } from '../../../EditorPresentationFixture.js';
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
/** Independent malformed and branded controls require rejection before any native authority commits. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undo, undoDepth, redoDepth } from '@codemirror/commands';
import { hostFixture } from './host-document-fixture.ts';
import type { HostUpdate } from '../../../../../../frontend/sugarprompt/editor/src/host-update.ts';
import {
  prepareSceneQueue,
  prepareLoraMetadata,
  resolveFeatureProfile,
  defaultPreferences,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import { sceneQueueHost } from '../../../../../../frontend/sugarprompt/editor/src/scene-queue.ts';
import { loraMetadataHost } from '../../../../../../frontend/sugarprompt/editor/src/lora-action-hosts.ts';
import { NativeConfiguration } from '../../../../../../frontend/sugarprompt/editor/src/native-configuration.ts';
import type { FeatureProfile } from '../../../../../../frontend/sugarprompt/services/src/index.ts';

test('holes at every prepared profile position reject at mount and every update mode without retiring authority', () => {
  const canonical = resolveFeatureProfile(defaultPreferences, {
    mode: 'library',
  });
  for (const field of ['decisions', 'rendererSyntax'] as const) {
    for (let index = 0; index < canonical[field].length; index++) {
      const array = [...canonical[field]];
      // eslint-disable-next-line @typescript-eslint/no-array-delete -- Deliberately supply a sparse array to prove fail-closed input validation.
      Reflect.deleteProperty(array, index);
      const profile = Object.freeze({
        ...canonical,
        [field]: Object.freeze(array),
      }) as FeatureProfile;
      assert.throws(
        () =>
          new NativeConfiguration({
            ...editorPresentations,
            source: '',
            label: 'Test',
            placeholder: '',
            readOnly: false,
            onUpdate() {},
            featureProfile: profile,
          }),
        TypeError,
      );
      for (const mode of ['preserve', 'exact-edit', 'baseline'] as const) {
        const owner = hostFixture();
        owner.updates.apply({ mode: 'exact-edit', source: 'first' });
        owner.updates.apply({ mode: 'exact-edit', source: 'second' });
        undo(owner.transport);
        const state = owner.state(),
          configuration = owner.configuration.values,
          revision = owner.revision(),
          counts = owner.counts();
        const update = {
          mode,
          ...(mode === 'preserve' ? {} : { source: 'final' }),
          configuration: {
            featureProfile: profile,
            readOnly: true,
            placeholder: 'new',
          },
        } as HostUpdate;
        assert.throws(() => owner.updates.apply(update), TypeError);
        assert.strictEqual(owner.state(), state);
        assert.strictEqual(owner.configuration.values, configuration);
        assert.equal(owner.revision(), revision);
        assert.deepEqual(owner.counts(), counts);
        assert.equal(owner.permission.matches(false), true);
        assert.deepEqual([undoDepth(state), redoDepth(state)], [1, 1]);
      }
    }
  }
  for (const profile of [
    canonical,
    resolveFeatureProfile(defaultPreferences, {
      mode: 'workflow',
      loraScheduling: false,
      field: { prompt_features: [] },
    }),
  ])
    for (const mode of ['preserve', 'exact-edit', 'baseline'] as const) {
      const owner = hostFixture();
      const outcome = owner.updates.apply({
        mode,
        ...(mode === 'preserve' ? {} : { source: 'final' }),
        configuration: { featureProfile: profile },
      } as HostUpdate);
      assert.equal(outcome.status, 'applied');
      assert.strictEqual(owner.configuration.values.featureProfile, profile);
    }
});

test('supplied falsy selections, malformed profiles and host envelopes preserve the complete prior bundle', () => {
  const owner = hostFixture();
  owner.updates.apply({ mode: 'exact-edit', source: 'first' });
  owner.updates.apply({ mode: 'exact-edit', source: 'second' });
  undo(owner.transport);
  const state = owner.state(),
    configuration = owner.configuration.values,
    revision = owner.revision(),
    counts = owner.counts();
  assert.equal(undoDepth(state), 1);
  assert.equal(redoDepth(state), 1);
  const fields = [
    ...[null, false, 0, '', undefined, [], { anchor: '0', head: 0 }].map((selection) => ({
      selection,
    })),
    ...[null, false, 0, '', undefined].map((configuration) => ({
      configuration,
    })),
    ...[
      null,
      false,
      {
        decisions: [{ feature: 'unknown', enabled: true }],
        rendererSyntax: [],
      },
    ].map((featureProfile) => ({ configuration: { featureProfile } })),
    ...['loraMetadataHost', 'loraPageHost', 'sceneQueueHost'].map((key) => ({
      configuration: { [key]: false },
    })),
    {
      configuration: {
        sceneQueueHost: {
          capability: { sourceRevision: 1, request: () => null },
          observe() {},
          report() {},
        },
      },
    },
    {
      configuration: {
        loraMetadataHost: {
          capability: { revision: 'forged' },
          catalogRevision: 'catalog',
          accept() {},
          observe() {},
          report() {},
        },
      },
    },
  ];
  for (const fieldsValue of fields) {
    assert.throws(() =>
      owner.updates.apply({
        mode: 'baseline',
        source: 'new',
        ...fieldsValue,
      } as unknown as HostUpdate),
    );
    assert.strictEqual(owner.state(), state);
    assert.strictEqual(owner.configuration.values, configuration);
    assert.equal(owner.revision(), revision);
    assert.deepEqual(owner.counts(), counts);
    assert.equal(owner.permission.matches(false), true);
  }
});

test('canonical service capabilities and complete profile adopt together and retain actual prepared publications', () => {
  const owner = hostFixture();
  const queue = prepareSceneQueue({
    revision: 'queue',
    sourceRevision: 1,
    queueableKeys: ['scene'],
    queue: async () => ({ status: 'queued' }),
  });
  const metadata = prepareLoraMetadata({
    revision: 'metadata',
    refresh: true,
    thumbnail: false,
    choices: [],
    activeImageId: null,
    execute: async () => ({ status: 'cancelled' }),
  });
  const profile = resolveFeatureProfile(defaultPreferences, {
    mode: 'library',
  });
  const queueHost = { capability: queue, observe() {}, report() {} },
    metadataHost = {
      capability: metadata,
      catalogRevision: 'catalog',
      accept() {},
      observe() {},
      report() {},
    };
  const outcome = owner.updates.apply({
    mode: 'baseline',
    source: '**scene\nbody',
    configuration: {
      featureProfile: profile,
      sceneQueueHost: queueHost,
      loraMetadataHost: metadataHost,
    },
  });
  assert.equal(outcome.status, 'applied');
  assert.strictEqual(owner.configuration.values.featureProfile, profile);
  assert.strictEqual(owner.state().facet(sceneQueueHost), queueHost);
  assert.strictEqual(owner.state().facet(loraMetadataHost), metadataHost);
  assert.strictEqual(queue.request('scene'), queue.request('scene'));
});
