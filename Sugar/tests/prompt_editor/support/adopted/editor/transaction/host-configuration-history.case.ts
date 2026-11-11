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
/** Actual host publications preserve contiguous native word history only across authenticated neutral configuration. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, Facet, StateEffect, Transaction } from '@codemirror/state';
import { undo, redo, undoDepth, historyField, isolateHistory } from '@codemirror/commands';
import type {
  HostConfiguration,
  HostUpdate,
} from '../../../../../../frontend/sugarprompt/editor/src/host-update.ts';
import { prepareHostDocument } from '../../../../../../frontend/sugarprompt/editor/src/host-document-preparation.ts';
import { configurationHistoryIntent } from '../../../../../../frontend/sugarprompt/editor/src/prompt-history.ts';
import { fixture } from './host-configuration-history-fixture.ts';
import {
  defaultPreferences,
  resolveFeatureProfile,
  prepareTagCompletionCatalog,
  prepareWildcardCatalog,
  prepareSceneTitleCatalog,
  createLoraCatalogAdapter,
  prepareLoraMetadata,
  prepareSceneQueue,
  SpellcheckSession,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';

const tags = prepareTagCompletionCatalog({
  revision: 'tags',
  bundled: [{ tag: 'cat', popularity: 1 }],
  customTags: [],
  censoredTags: [],
});
const session = new SpellcheckSession({
  identity: Symbol(),
  languageTag: 'en',
  availability: () => ({
    status: 'available',
    dictionaryAdd: false,
    sessionIgnore: false,
  }),
  checkWords: async (words) => ({
    status: 'ready',
    value: words.map(() => true),
  }),
  suggest: async () => ({ status: 'ready', value: [] }),
});
test.after(() => session.dispose());
const neutral: readonly [keyof HostConfiguration, HostConfiguration][] = [
  ['placeholder', { placeholder: 'Changed' }],
  ['sourceLineChrome', { sourceLineChrome: { enabled: true, contentLeftInset: 12 } }],
  ['wheelMode', { wheelMode: 'focus_required' }],
  ['tagLexicon', { tagLexicon: tags.lexicon }],
  ['tagCompletionCatalog', { tagCompletionCatalog: tags }],
  [
    'loraCompletionCatalog',
    {
      loraCompletionCatalog: createLoraCatalogAdapter(
        { revision: 'models', authoritative: true, items: [] },
        { searchMetadata: [] },
      ).completionCatalog!,
    },
  ],
  [
    'wildcardCatalog',
    {
      wildcardCatalog: prepareWildcardCatalog({
        status: 'pending',
        revision: 'wildcards',
      }),
    },
  ],
  [
    'sceneTitleCatalog',
    {
      sceneTitleCatalog: prepareSceneTitleCatalog({
        status: 'pending',
        revision: 'titles',
      }),
    },
  ],
  [
    'sceneQueueHost',
    {
      sceneQueueHost: {
        capability: prepareSceneQueue({
          revision: 'queue',
          sourceRevision: 1,
          queueableKeys: [],
          queue: async () => ({ status: 'queued' }),
        }),
        observe() {},
        report() {},
      },
    },
  ],
  [
    'loraPageHost',
    {
      loraPageHost: {
        adapter: { open: async () => ({ status: 'opened' }) },
        observe() {},
        report() {},
      },
    },
  ],
  [
    'loraMetadataHost',
    {
      loraMetadataHost: {
        capability: prepareLoraMetadata({
          revision: 'metadata',
          refresh: true,
          thumbnail: false,
          choices: [],
          activeImageId: null,
          execute: async () => ({ status: 'cancelled' }),
        }),
        catalogRevision: 'models',
        accept() {},
        observe() {},
        report() {},
      },
    },
  ],
  ['loraSchedulingHost', { loraSchedulingHost: { context: null, catalogRevision: 'models' } }],
  ['booruPostLookup', { booruPostLookup: async () => ({ status: 'unavailable' }) }],
  ['spellcheckSession', { spellcheckSession: session }],
];
for (const [key, configuration] of neutral)
  test(`actual neutral ${key} publication preserves the native word and repeated undo/redo`, () => {
    const f = fixture();
    try {
      f.type('c');
      const before = f.host.state();
      assert.equal(f.host.updates.apply({ mode: 'preserve', configuration }).status, 'applied');
      const after = f.host.state();
      assert.strictEqual(after.doc, before.doc);
      assert.strictEqual(after.selection, before.selection);
      assert.equal(undoDepth(after), 1);
      assert.equal(f.host.updates.apply({ mode: 'preserve', configuration }).status, 'unchanged');
      assert.strictEqual(f.host.state(), after);
      f.type('a');
      f.type('t');
      assert.equal(f.host.source(), 'cat');
      assert.equal(undoDepth(f.host.state()), 1);
      for (let index = 0; index < 2; index++) {
        assert.equal(undo(f.host.transport), true);
        assert.equal(f.host.source(), '');
        assert.equal(redo(f.host.transport), true);
        assert.equal(f.host.source(), 'cat');
      }
    } finally {
      f.destroy();
    }
  });

const reset: readonly [string, HostUpdate][] = [
  [
    'permission',
    {
      mode: 'preserve',
      configuration: { placeholder: 'Changed', readOnly: true },
    },
  ],
  [
    'profile',
    {
      mode: 'preserve',
      configuration: {
        placeholder: 'Changed',
        featureProfile: resolveFeatureProfile(
          {
            ...defaultPreferences,
            allowed: { ...defaultPreferences.allowed, emphasis: false },
          },
          { mode: 'library' },
        ),
      },
    },
  ],
  [
    'rendering',
    {
      mode: 'preserve',
      configuration: { placeholder: 'Changed', richRendering: false },
    },
  ],
  [
    'document-kind',
    {
      mode: 'preserve',
      configuration: { placeholder: 'Changed', documentKind: 'wildcard-txt' },
    },
  ],
  [
    'conditioning',
    {
      mode: 'preserve',
      configuration: {
        placeholder: 'Changed',
        conditioningContext: { key: 'changed', mode: 'independent' },
      },
    },
  ],
  [
    'explicit-selection',
    {
      mode: 'preserve',
      configuration: { placeholder: 'Changed' },
      selection: { anchor: 1, head: 1 },
    },
  ],
];
for (const [kind, update] of reset)
  test(`mixed neutral and ${kind} remains a word boundary`, () => {
    const f = fixture();
    try {
      f.type('c');
      assert.equal(f.host.updates.apply(update).status, 'applied');
      if (kind === 'permission')
        assert.equal(
          f.host.updates.apply({
            mode: 'preserve',
            configuration: { readOnly: false },
          }).status,
          'applied',
        );
      f.type('a');
      f.type('t');
      assert.equal(undoDepth(f.host.state()), 2);
      assert.equal(undo(f.host.transport), true);
      assert.equal(f.host.source(), 'c');
    } finally {
      f.destroy();
    }
  });

for (const kind of ['foreign', 'composition', 'isolation', 'source'] as const)
  test(`complete effect vector refuses ${kind} additions at actual publication`, () => {
    const f = fixture(),
      compartment = new Compartment(),
      foreign = Facet.define<number, number>({
        combine: (values) => values[0] ?? 0,
      });
    try {
      f.host.transport.dispatch({
        effects: StateEffect.appendConfig.of(compartment.of(foreign.of(0))),
      });
      f.type('c');
      const state = f.host.state(),
        prepared = prepareHostDocument(
          state,
          { mode: 'preserve', configuration: { placeholder: 'Changed' } },
          f.host.configuration,
          1,
          () => {
            throw new Error('Unexpected baseline');
          },
        );
      const effects = prepared.transaction.effects;
      f.host.transport.dispatch(
        state.update({
          effects:
            kind === 'foreign'
              ? [...effects, compartment.reconfigure(foreign.of(1))]
              : kind === 'composition'
                ? [
                    ...effects,
                    compartment.reconfigure(historyField.init(() => state.field(historyField))),
                  ]
                : effects,
          ...(kind === 'isolation' ? { annotations: isolateHistory.of('full') } : {}),
          ...(kind === 'source'
            ? {
                changes: { from: 1, insert: 'x' },
                selection: { anchor: 2 },
                userEvent: 'input.type',
                annotations: Transaction.time.of(1_000_001),
              }
            : {}),
        }),
      );
      f.type('a');
      f.type('t');
      assert.equal(undoDepth(f.host.state()), kind === 'source' ? 3 : 2);
      assert.equal(undo(f.host.transport), true);
      assert.equal(f.host.source(), kind === 'source' ? 'cx' : 'c');
    } finally {
      f.destroy();
    }
  });

test('neutral intent captures a frozen exact vector and rejects different effect identity', () => {
  const f = fixture(),
    effect = StateEffect.define<null>(),
    first = effect.of(null),
    second = effect.of(null);
  try {
    f.type('c');
    const intent = configurationHistoryIntent(['placeholder'], [first]);
    assert.notEqual(intent, null);
    assert.equal(Object.isFrozen(intent!.value), true);
    const compartment = new Compartment();
    f.host.transport.dispatch({
      effects: [StateEffect.appendConfig.of(compartment.of([])), second, intent!],
    });
    f.type('a');
    f.type('t');
    assert.equal(undoDepth(f.host.state()), 2);
    assert.equal(undo(f.host.transport), true);
    assert.equal(f.host.source(), 'c');
  } finally {
    f.destroy();
  }
});

test('baseline resets native history even alongside a neutral publication', () => {
  const f = fixture();
  try {
    f.type('c');
    assert.equal(
      f.host.updates.apply({
        mode: 'baseline',
        source: 'c',
        configuration: { placeholder: 'Changed' },
      }).status,
      'applied',
    );
    assert.equal(undoDepth(f.host.state()), 0);
    f.type('a');
    f.type('t');
    assert.equal(undoDepth(f.host.state()), 1);
    assert.equal(undo(f.host.transport), true);
    assert.equal(f.host.source(), 'c');
  } finally {
    f.destroy();
  }
});

test('unchanged permission and typing operands do not turn a neutral publication into a boundary', () => {
  const f = fixture(),
    profile = resolveFeatureProfile(defaultPreferences, { mode: 'library' });
  try {
    assert.equal(
      f.host.updates.apply({
        mode: 'preserve',
        configuration: { featureProfile: profile },
      }).status,
      'applied',
    );
    f.type('c');
    assert.equal(
      f.host.updates.apply({
        mode: 'preserve',
        configuration: {
          placeholder: 'Changed',
          featureProfile: profile,
          richRendering: true,
          readOnly: false,
        },
      }).status,
      'applied',
    );
    f.type('a');
    f.type('t');
    assert.equal(undoDepth(f.host.state()), 1);
    assert.equal(undo(f.host.transport), true);
    assert.equal(f.host.source(), '');
  } finally {
    f.destroy();
  }
});

for (const kind of ['removed', 'reordered', 'duplicated-intent'] as const)
  test(`captured final effect vector refuses ${kind} publication`, () => {
    const f = fixture();
    try {
      f.type('c');
      const state = f.host.state();
      const prepared = prepareHostDocument(
        state,
        {
          mode: 'preserve',
          configuration: {
            placeholder: 'Changed',
            sourceLineChrome: { enabled: true, contentLeftInset: 12 },
          },
        },
        f.host.configuration,
        1,
        () => {
          throw new Error('Unexpected baseline');
        },
      );
      const effects = prepared.transaction.effects;
      assert.equal(effects.length, 3);
      const changed =
        kind === 'removed'
          ? effects.slice(1)
          : kind === 'reordered'
            ? [effects[1]!, effects[0]!, effects[2]!]
            : [...effects, effects[2]!];
      f.host.transport.dispatch(state.update({ effects: changed }));
      f.type('a');
      f.type('t');
      assert.equal(undoDepth(f.host.state()), 2);
      assert.equal(undo(f.host.transport), true);
      assert.equal(f.host.source(), 'c');
    } finally {
      f.destroy();
    }
  });

test('actual prepared intent wrapper cannot replace its frozen vector to admit unknown reconfiguration', () => {
  for (const mutate of [false, true]) {
    const f = fixture(),
      compartment = new Compartment(),
      foreign = Facet.define<number, number>({
        combine: (values) => values[0] ?? 0,
      });
    try {
      f.host.transport.dispatch({
        effects: StateEffect.appendConfig.of(compartment.of(foreign.of(0))),
      });
      f.type('c');
      const state = f.host.state();
      const prepared = prepareHostDocument(
        state,
        { mode: 'preserve', configuration: { placeholder: 'Changed' } },
        f.host.configuration,
        1,
        () => {
          throw new Error('Unexpected baseline');
        },
      );
      const effects = prepared.transaction.effects,
        intent = effects.at(-1)!,
        extra = compartment.reconfigure(foreign.of(1));
      assert.equal(Object.isFrozen(intent), true);
      assert.equal(Object.isFrozen(intent.value), true);
      assert.ok(Array.isArray(intent.value));
      const original = intent.value;
      if (mutate)
        assert.equal(Reflect.set(intent, 'value', Object.freeze([...original, extra])), false);
      assert.strictEqual(intent.value, original);
      f.host.transport.dispatch(state.update({ effects: [...effects, extra] }));
      assert.equal(f.host.state().facet(foreign), 1);
      f.type('a');
      f.type('t');
      assert.equal(f.host.source(), 'cat');
      assert.equal(undoDepth(f.host.state()), 2);
      assert.equal(undo(f.host.transport), true);
      assert.equal(f.host.source(), 'c');
      assert.equal(redo(f.host.transport), true);
      assert.equal(f.host.source(), 'cat');
      assert.equal(undoDepth(f.host.state()), 2);
    } finally {
      f.destroy();
    }
  }
});
