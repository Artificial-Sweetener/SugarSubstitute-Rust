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
/** Independent capability contracts cover retained scope, precedence and injected conflict policy. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  defaultPreferences,
  captureFeatureContext,
  sameFeatureContext,
  featureDefinitions,
  featureEnabled,
  resolveFeatureProfile,
  retainFeatureProfile,
  type FeatureProfile,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';

test('equivalent host contexts retain actual resolved output while every observable profile change publishes', () => {
  const previous = resolveFeatureProfile(defaultPreferences, {
    mode: 'workflow',
    loraScheduling: true,
  });
  const equivalent = resolveFeatureProfile(defaultPreferences, {
    mode: 'workflow',
    loraScheduling: true,
  });
  assert.notEqual(previous, equivalent);
  assert.equal(retainFeatureProfile(previous, equivalent), previous);
  const disabled = resolveFeatureProfile(defaultPreferences, {
    mode: 'workflow',
    loraScheduling: false,
  });
  assert.equal(retainFeatureProfile(previous, disabled), disabled);
  const disabledIndex = disabled.decisions.findIndex((decision) => !decision.enabled);
  const disabledDecision = disabled.decisions[disabledIndex]!;
  assert.equal(disabledDecision.enabled, false);
  if (disabledDecision.enabled) throw new Error('Disabled specimen missing');
  for (const patch of [
    { feature: 'wildcard_syntax' as const },
    { disabledReason: 'conflict' as const },
    { detail: 'Host detail changed' },
  ]) {
    const changed: FeatureProfile = {
      ...disabled,
      decisions: disabled.decisions.map((decision, index) =>
        index === disabledIndex ? { ...disabledDecision, ...patch } : decision,
      ),
    };
    assert.equal(retainFeatureProfile(disabled, changed), changed);
  }
  const syntaxChanged = {
    ...equivalent,
    rendererSyntax: equivalent.rendererSyntax.slice(1),
  };
  assert.equal(retainFeatureProfile(previous, syntaxChanged), syntaxChanged);
});

test('library preferences resolve independently of workflow proof and generation intent', () => {
  const preferences = applyPreferencePatch(defaultPreferences, {
    allowed: { lora_syntax: false, wildcard_syntax: false },
    resolveOnGeneration: false,
  });
  const profile = resolveFeatureProfile(preferences, { mode: 'library' });
  for (const loraScheduling of [false, true]) {
    const next = resolveFeatureProfile(preferences, {
      mode: 'library',
      loraScheduling,
    });
    assert.deepEqual(next, profile);
    assert.strictEqual(retainFeatureProfile(profile, next), profile);
  }
  assert.deepEqual(
    profile.decisions.filter(({ enabled }) => !enabled),
    [
      ['wildcard_syntax', null],
      ['wildcard_autocomplete', 'Requires wildcard_syntax.'],
      ['lora_syntax', null],
      ['lora_autocomplete', 'Requires lora_syntax.'],
      ['lora_picker', 'Requires lora_syntax.'],
    ].map(([feature, detail]) => ({
      feature,
      enabled: false,
      disabledReason: 'user_disabled',
      detail,
    })),
  );
  assert.deepEqual(profile.rendererSyntax, ['emphasis']);
  assert.equal(featureEnabled(profile, 'lora_trigger_words'), true);
  assert.deepEqual(Object.keys(profile).sort(), ['decisions', 'rendererSyntax']);
  assert.deepEqual(
    resolveFeatureProfile(applyPreferencePatch(preferences, { resolveOnGeneration: true }), {
      mode: 'library',
    }),
    profile,
  );
});

test('library service withdrawal retires actual profile output and recovery restores eligibility', () => {
  const available = resolveFeatureProfile(defaultPreferences, {
    mode: 'library',
  });
  const unavailable = resolveFeatureProfile(defaultPreferences, {
    mode: 'library',
    unavailableFeatures: ['lora_picker', 'wildcard_autocomplete'],
  });
  assert.deepEqual(
    unavailable.decisions.filter(({ enabled }) => !enabled),
    ['wildcard_autocomplete', 'lora_picker'].map((feature) => ({
      feature,
      enabled: false,
      disabledReason: 'missing_service',
      detail: 'The host has not supplied this capability.',
    })),
  );
  assert.deepEqual(unavailable.rendererSyntax, available.rendererSyntax);
  assert.strictEqual(retainFeatureProfile(available, unavailable), unavailable);
  const recovered = resolveFeatureProfile(defaultPreferences, {
    mode: 'library',
  });
  assert.strictEqual(retainFeatureProfile(unavailable, recovered), recovered);
  assert.deepEqual(recovered, available);
});

test('captured library declarations retain their policy when host arrays mutate', () => {
  const allowed = ['emphasis', 'wildcard_syntax', 'wildcard_autocomplete'];
  const missing = ['wildcard_autocomplete' as const];
  const original = {
    mode: 'library' as const,
    field: { prompt_features: allowed },
    unavailableFeatures: missing,
  };
  const captured = captureFeatureContext(original);
  const before = resolveFeatureProfile(defaultPreferences, captured);
  allowed.length = 0;
  missing.length = 0;
  assert.equal(sameFeatureContext(captured, original), false);
  assert.deepEqual(resolveFeatureProfile(defaultPreferences, captured), before);
  assert.equal(featureEnabled(before, 'emphasis'), true);
  assert.equal(featureEnabled(before, 'wildcard_syntax'), true);
  assert.equal(featureEnabled(before, 'wildcard_autocomplete'), false);
  assert.equal(Object.isFrozen(captured), true);
  assert.equal(Object.isFrozen(captured.field!.prompt_features), true);
  assert.equal(Object.isFrozen(captured.unavailableFeatures), true);
  assert.equal(
    sameFeatureContext(
      captured,
      captureFeatureContext({
        mode: 'library',
        field: {
          prompt_features: ['emphasis', 'wildcard_syntax', 'wildcard_autocomplete'],
        },
        unavailableFeatures: ['wildcard_autocomplete'],
      }),
    ),
    true,
  );
});
import {
  applyPreferencePatch,
  readPreferences,
} from '../../../../../../frontend/sugarprompt/services/src/features/preferences.ts';

test('registry defaults, settings order and unavailable host capabilities are explicit', () => {
  assert.equal(featureDefinitions.length, 12);
  assert.equal(
    featureDefinitions.some(({ id }) => String(id).includes('wiki')),
    false,
  );
  const library = resolveFeatureProfile(defaultPreferences, {
    mode: 'library',
  });
  assert.equal(
    library.decisions.every(({ enabled }) => enabled),
    true,
  );
  assert.deepEqual(library.rendererSyntax, ['emphasis', 'wildcard', 'lora']);
  const workflow = resolveFeatureProfile(defaultPreferences, {
    mode: 'workflow',
  });
  assert.deepEqual(
    workflow.decisions.filter(({ enabled }) => !enabled).map(({ feature }) => feature),
    ['lora_autocomplete', 'lora_picker', 'lora_trigger_words'],
  );
  assert.equal(featureEnabled(workflow, 'lora_syntax'), true);
  const missing = resolveFeatureProfile(defaultPreferences, {
    mode: 'library',
    unavailableFeatures: ['wildcard_syntax'],
  });
  assert.equal(
    missing.decisions.find(({ feature }) => feature === 'wildcard_autocomplete')?.disabledReason,
    'missing_service',
  );
  assert.equal(Object.isFrozen(library.decisions[0]), true);
});

test('field permission precedes user preferences and legacy aliases preserve retained baseline', () => {
  const preferences = applyPreferencePatch(defaultPreferences, {
    allowed: { emphasis: false, wildcard_syntax: false },
  });
  const explicit = resolveFeatureProfile(preferences, {
    mode: 'workflow',
    loraScheduling: true,
    field: {
      prompt_features: [' WILDCARD_AUTOCOMPLETE ', 'wildcard_syntax', 'unknown', 7],
      prompt_syntaxes: ['emphasis'],
    },
  });
  assert.deepEqual(
    explicit.decisions.find(({ feature }) => feature === 'emphasis'),
    {
      feature: 'emphasis',
      enabled: false,
      disabledReason: 'field_disabled',
      detail: null,
    },
  );
  assert.deepEqual(
    explicit.decisions.find(({ feature }) => feature === 'wildcard_autocomplete'),
    {
      feature: 'wildcard_autocomplete',
      enabled: false,
      disabledReason: 'user_disabled',
      detail: 'Requires wildcard_syntax.',
    },
  );
  const legacy = resolveFeatureProfile(defaultPreferences, {
    mode: 'workflow',
    field: { prompt_syntaxes: ['LoRA'] },
    loraScheduling: true,
  });
  assert.equal(featureEnabled(legacy, 'lora_picker'), true);
  assert.equal(featureEnabled(legacy, 'emphasis'), false);
  assert.equal(featureEnabled(legacy, 'spellcheck'), true);
  assert.equal(
    resolveFeatureProfile(defaultPreferences, {
      mode: 'library',
      field: { prompt_features: [] },
    }).decisions.some(({ enabled }) => enabled),
    false,
  );
});

test('conflicts deterministically retain registry priority and disable dependent actions', () => {
  const definitions = featureDefinitions.map((entry) => ({
    ...entry,
    conflicts: entry.id === 'emphasis' ? ['lora_syntax' as const] : entry.conflicts,
  }));
  const profile = resolveFeatureProfile(defaultPreferences, { mode: 'library' }, definitions);
  assert.equal(featureEnabled(profile, 'emphasis'), true);
  assert.equal(
    profile.decisions.find(({ feature }) => feature === 'lora_syntax')?.disabledReason,
    'conflict',
  );
  assert.equal(
    profile.decisions.find(({ feature }) => feature === 'lora_picker')?.disabledReason,
    'conflict',
  );
});

test('decoded partial preferences retain defaults while malformed data is rejected without coercion', () => {
  const partial = readPreferences({
    allowed: { emphasis: false },
    wheelMode: 'focus_required',
    resolveOnGeneration: false,
  });
  assert.equal(partial?.allowed.emphasis, false);
  assert.equal(partial?.allowed.lora_picker, true);
  assert.equal(partial?.resolveOnGeneration, false);
  for (const value of [
    [],
    undefined,
    { schema: 99 },
    { allowed: { emphasis: 'false' } },
    { allowed: { danbooru_wiki_lookup: false } },
    { resolveOnGeneration: 0 },
    { wheelMode: 'unknown' },
  ])
    assert.equal(readPreferences(value), null);
});
