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
/** Actual service and CM dispatch characterize finite mounted checks, lazy transport and accepted session effects. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Transaction, EditorState } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import type { SpellingLookup } from '../../../../../../frontend/sugarprompt/services/src/diagnostics/spelling-contracts.ts';
import { spellingFixture } from './spelling-provider-fixture.ts';
import {
  spellingDiagnosticAt,
  spellingDiagnostics,
  spellingSession,
} from '../../../../../../frontend/sugarprompt/editor/src/spelling-diagnostics.ts';
import { installContextMenu } from '../../../../../../frontend/sugarprompt/editor/src/context-menu-state.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { prepareTagLexicon } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import { resolveFeatureProfile } from '../../../../../../frontend/sugarprompt/services/src/features/profile.ts';
import { defaultPreferences } from '../../../../../../frontend/sugarprompt/services/src/features/preferences.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

test('source replacement cancels ignored-abort checks before any late publication', async () => {
  const f = spellingFixture();
  try {
    let finish: ((value: SpellingLookup) => void) | null = null;
    let signal: AbortSignal | null = null;
    f.boundary.check = (_words, abort) => {
      signal = abort;
      return new Promise((resolve) => {
        finish = resolve;
      });
    };
    const pending = f.check();
    assert.ok(finish);
    f.view.dispatch({ changes: { from: f.state().doc.length, insert: '!' } });
    await pending;
    assert.equal((signal as AbortSignal | null)?.aborted, true);
    (finish as (value: SpellingLookup) => void)({
      status: 'ready',
      value: [false, true],
    });
    assert.equal(f.state().field(spellingDiagnostics), null);
  } finally {
    f.dispose();
  }
});
test('accepted Ignore survives its own revision and actual menu-close link while only derived requests retire', async () => {
  const f = spellingFixture();
  try {
    await f.check();
    const value = spellingDiagnosticAt(f.state(), 1)!,
      source = readExactSource(f.state());
    let finish: ((value: SpellingLookup) => void) | null = null;
    let signal: AbortSignal | null = null;
    f.boundary.ignore = (abort) => {
      signal = abort;
      return new Promise((resolve) => {
        finish = resolve;
      });
    };
    const pending = f.provider.operate(value, 'ignore');
    assert.ok(finish);
    assert.equal(f.session.current(value.snapshot, value.issue), false);
    assert.equal((signal as AbortSignal | null)?.aborted, false);
    f.provider.cancelSuggestions();
    f.view.dispatch({
      effects: installContextMenu.of(null),
      annotations: Transaction.addToHistory.of(false),
    });
    await f.check();
    assert.equal(f.state().field(spellingDiagnostics)!.findings.size, 0);
    assert.equal((signal as AbortSignal | null)?.aborted, false);
    (finish as (value: SpellingLookup) => void)({
      status: 'ready',
      value: true,
    });
    await pending;
    assert.equal(readExactSource(f.state()), source);
    assert.equal(undoDepth(f.state()), 0);
  } finally {
    f.dispose();
  }
});
test('the same host session retains word-wide casefold Ignore across native remount while a new session starts empty', async () => {
  const first = spellingFixture();
  await first.check();
  await first.provider.operate(spellingDiagnosticAt(first.state(), 1)!, 'ignore');
  const session = first.session;
  first.dispose();
  const second = spellingFixture('HELO, tail', false, 'prompt', session);
  try {
    await second.check();
    assert.equal(second.state().field(spellingDiagnostics)!.findings.size, 0);
  } finally {
    second.dispose();
  }
  const fresh = spellingFixture();
  try {
    await fresh.check();
    assert.ok(spellingDiagnosticAt(fresh.state(), 1));
  } finally {
    fresh.dispose();
  }
});
test('false dictionary persistence and unavailable transport preserve source history and unsuppressed authority', async () => {
  const f = spellingFixture();
  try {
    await f.check();
    const value = spellingDiagnosticAt(f.state(), 1)!,
      source = readExactSource(f.state());
    f.boundary.add = async () => ({ status: 'ready', value: false });
    await f.provider.operate(value, 'add');
    assert.equal(f.session.current(value.snapshot, value.issue), true);
    assert.equal(f.session.revision, 0);
    f.boundary.add = async () => ({
      status: 'unavailable',
      code: 'dictionary-unavailable',
    });
    await f.provider.operate(value, 'add');
    assert.equal(f.session.revision, 0);
    assert.equal(readExactSource(f.state()), source);
    assert.equal(undoDepth(f.state()), 0);
  } finally {
    f.dispose();
  }
});
test('unexpected check and accepted command causes retain exact identity without empty ready success', async () => {
  const f = spellingFixture();
  try {
    const cause = new Error('injected-boundary');
    f.boundary.check = async () => {
      throw cause;
    };
    await assert.rejects(f.check(), (error) => error === cause);
    assert.equal(f.state().field(spellingDiagnostics)!.status, 'failed');
  } finally {
    f.dispose();
  }
  const g = spellingFixture();
  try {
    await g.check();
    g.boundary.add = async () => {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      throw undefined;
    };
    await g.provider.operate(spellingDiagnosticAt(g.state(), 1)!, 'add').then(
      () => assert.fail('unexpected success'),
      (cause: unknown) => assert.equal(cause, undefined),
    );
    assert.equal(g.session.revision, 0);
  } finally {
    g.dispose();
  }
});
test('current unavailable dictionary is explicit and never an empty ready snapshot', async () => {
  const f = spellingFixture();
  try {
    f.boundary.available = false;
    await f.check();
    const actual = f.state().field(spellingDiagnostics)!;
    assert.equal(actual.status, 'unavailable');
    assert.equal(actual.code, 'backend-unavailable');
    assert.equal(actual.snapshot, null);
    assert.equal(f.counts.checks, 0);
  } finally {
    f.dispose();
  }
});
test('source profile kind lexicon permission session composition and teardown retire a pending accepted external operation', async () => {
  for (const mode of [
    'source',
    'profile',
    'kind',
    'lexicon',
    'permission',
    'session',
    'composition',
    'destroy',
  ] as const) {
    const f = spellingFixture();
    try {
      await f.check();
      let signal: AbortSignal | null = null;
      f.boundary.add = (abort) => {
        signal = abort;
        return new Promise(() => {});
      };
      const pending = f.provider.operate(spellingDiagnosticAt(f.state(), 1)!, 'add');
      assert.ok(signal);
      if (mode === 'source')
        f.view.dispatch({
          changes: { from: f.state().doc.length, insert: '!' },
        });
      else if (mode === 'profile')
        f.view.dispatch({
          effects: f.profile.reconfigure(
            featureProfile.of(resolveFeatureProfile(defaultPreferences, { mode: 'workflow' })),
          ),
        });
      else if (mode === 'kind')
        f.view.dispatch({
          effects: f.initial.kindSlot.reconfigure(documentKind.of('wildcard-csv')),
        });
      else if (mode === 'lexicon') {
        f.setLexicon(prepareTagLexicon(['helo']));
        f.view.dispatch({ effects: invalidateCommandContext.of(null) });
      } else if (mode === 'permission')
        f.view.dispatch({
          effects: f.initial.permission.reconfigure(EditorState.readOnly.of(true)),
        });
      else if (mode === 'session')
        f.view.dispatch({
          effects: f.host.reconfigure(spellingSession.of(null)),
        });
      else if (mode === 'composition') f.provider.compositionStart();
      else f.provider.destroy();
      await pending;
      assert.equal((signal as AbortSignal | null)?.aborted, true);
      assert.equal(f.session.revision, 0);
    } finally {
      f.dispose();
    }
  }
});
test('synchronous observer context reentry at pending publication prevents stale backend submission', async () => {
  const f = spellingFixture();
  try {
    let observed = false;
    f.observe(() => {
      if (!observed) {
        observed = true;
        f.view.dispatch({
          effects: invalidateCommandContext.of(null),
          annotations: Transaction.addToHistory.of(false),
        });
      }
    });
    await f.check();
    assert.equal(observed, true);
    assert.equal(f.counts.checks, 0);
    assert.equal(f.state().field(spellingDiagnostics), null);
  } finally {
    f.dispose();
  }
});
test('lazy suggestions settle cancelled after menu retirement even when the gateway ignores its signal', async () => {
  const f = spellingFixture();
  try {
    await f.check();
    let finish: ((value: SpellingLookup) => void) | null = null;
    f.boundary.suggest = () =>
      new Promise((resolve) => {
        finish = resolve;
      });
    const pending = f.provider.suggest(spellingDiagnosticAt(f.state(), 1)!);
    assert.ok(finish);
    f.provider.cancelSuggestions();
    const outcome = await pending;
    assert.equal(outcome.status, 'cancelled');
    (finish as (value: SpellingLookup) => void)({
      status: 'ready',
      value: ['hello'],
    });
    assert.equal(f.counts.suggestions, 1);
  } finally {
    f.dispose();
  }
});

test('failed diagnostic publication preserves the original backend cause before synchronous observer failure', async () => {
  const f = spellingFixture(),
    publicationCause = new Error('spelling-publication-observer');
  try {
    f.boundary.check = async () => {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      throw undefined;
    };
    f.observe(() => {
      if (f.state().field(spellingDiagnostics)?.status === 'failed') throw publicationCause;
    });
    let observed: unknown;
    try {
      await f.check();
    } catch (cause) {
      observed = cause;
    }
    assert.ok(observed instanceof AggregateError);
    assert.deepEqual(observed.errors, [undefined, publicationCause]);
    assert.equal(f.state().field(spellingDiagnostics)!.status, 'failed');
    assert.equal(undoDepth(f.state()), 0);
  } finally {
    f.dispose();
  }
});
