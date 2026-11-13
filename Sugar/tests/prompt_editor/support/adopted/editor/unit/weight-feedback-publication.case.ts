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
/** Actual preview-owner characterization isolates queued spatial intent from source-current derived publication. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, StateEffect, type EditorSelection } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { weightStateFor } from '../transaction/weight-command-state.ts';
import { prepareWeightChange } from '../../../../../../frontend/sugarprompt/editor/src/weight-command.ts';
import {
  editorFreshness,
  invalidateCommandContext,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { weightSession } from '../../../../../../frontend/sugarprompt/editor/src/weight-session.ts';
import { weightPreviewText } from '../../../../../../frontend/sugarprompt/editor/src/weight-preview-value.ts';
import { showWeightPreview } from '../../../../../../frontend/sugarprompt/editor/src/weight-preview.ts';
import {
  installLoraPresentation,
  loraPresentation,
  prepareLoraPresentation,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import {
  showWheelFeedback,
  wheelFeedback,
} from '../../../../../../frontend/sugarprompt/editor/src/wheel-feedback.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { previewFixture } from './weight-feedback-fixture.ts';

/** Genuine accepted commands prepare the exact LoRA spelling and outside native caret before the controlled transport begins. */
function acceptedLora(owner: 'pointer' | 'wheel'): EditorState {
  const source = 'lead, <lora:model:1.05>, tail',
    initial = weightStateFor(source, source.length),
    target = {
      freshness: initial.field(editorFreshness),
      kind: 'lora' as const,
      range: { from: source.indexOf('<lora:'), to: source.indexOf('>') + 1 },
    };
  const transactions = prepareWeightChange(
    initial,
    { kind: 'step', direction: 'increase' },
    owner === 'pointer' ? { ...target, owner } : { ...target, owner, token: Symbol() },
  );
  assert.equal(transactions.length, 2);
  return transactions.at(-1)!.state.update({ effects: StateEffect.appendConfig.of(wheelFeedback) })
    .state;
}

/** Publish the genuine accepted state through the established controlled native transport before any queued observation begins. */
function loraFixture(owner: 'pointer' | 'wheel') {
  const fixture = previewFixture();
  fixture.publish(acceptedLora(owner));
  return fixture;
}

/** Feedback observes exact source, full native selection and history without acquiring editing authority. */
function unchanged(
  state: EditorState,
  source: string,
  selection: EditorSelection,
  history: number,
): void {
  assert.equal(readExactSource(state), source);
  assert.ok(state.selection.eq(selection, true));
  assert.equal(undoDepth(state), history);
}

for (const owner of ['pointer', 'wheel'] as const)
  for (const publication of ['resources', 'feedback', 'both'] as const)
    test(`queued ${owner} LoRA preview survives same-session ${publication} publication and paints its actual accepted value`, async () => {
      const f = loraFixture(owner);
      try {
        const session = f.session(),
          freshness = f.state.field(editorFreshness),
          source = readExactSource(f.state),
          selection = f.state.selection,
          history = undoDepth(f.state),
          previousResources = f.state.field(loraPresentation),
          resources = prepareLoraPresentation(f.state, {
            status: 'ready',
            resources: [{ name: 'model', resolution: 'missing' }],
          });
        assert.equal(weightPreviewText(f.state, session), '1.10');
        showWeightPreview(f.view, session, { x: 100, y: 90 });
        assert.equal(f.jobs.length, 0);
        const next = f.state.update({
          effects: [
            ...(publication !== 'feedback' ? [installLoraPresentation.of(resources)] : []),
            ...(publication !== 'resources'
              ? [
                  showWheelFeedback.of({
                    kind: 'lora',
                    range: session.range,
                    freshness,
                  }),
                ]
              : []),
          ],
        }).state;
        assert.equal(next.field(weightSession), session);
        assert.equal(next.field(editorFreshness), freshness);
        assert.equal(weightPreviewText(next, session), '1.10');
        if (publication !== 'feedback') {
          assert.notEqual(next.field(loraPresentation), previousResources);
          assert.equal(next.field(loraPresentation), resources);
        }
        if (publication !== 'resources')
          assert.equal(next.field(wheelFeedback).target?.range, session.range);
        f.publish(next);
        await Promise.resolve();
        assert.equal(f.jobs.length, 1);
        f.measure();
        f.measure();
        assert.equal(f.root.hidden, false);
        assert.equal(f.root.style.visibility, 'visible');
        assert.equal(f.root.textContent, '1.10');
        assert.equal(f.timers.size, 1);
        unchanged(f.state, source, selection, history);
      } finally {
        f.destroy();
      }
      assert.equal(f.listeners.size + f.timers.size, 0);
    });

for (const retirement of [
  'source',
  'selection',
  'identical-selection',
  'context',
  'read-only',
  'raw',
  'composition',
  'clear',
  'dispose',
] as const)
  test(`queued LoRA publication remains retired after ${retirement}`, async () => {
    const f = loraFixture('wheel');
    try {
      showWeightPreview(f.view, f.session(), { x: 100, y: 90 });
      if (retirement === 'dispose') f.retire();
      else if (retirement === 'clear') {
        f.owner.clear();
        f.rejectObservations();
      } else if (retirement === 'composition') Object.assign(f.view, { compositionStarted: true });
      else {
        const next = f.state.update(
          retirement === 'source'
            ? { changes: { from: 0, insert: 'z' } }
            : retirement === 'selection'
              ? { selection: { anchor: 0 } }
              : retirement === 'identical-selection'
                ? { selection: f.state.selection }
                : retirement === 'context'
                  ? { effects: invalidateCommandContext.of(null) }
                  : retirement === 'read-only'
                    ? {
                        effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)),
                      }
                    : {
                        effects: StateEffect.appendConfig.of(richRendering.of(false)),
                      },
        ).state;
        if (retirement === 'identical-selection') {
          assert.ok(next.selection.eq(f.state.selection, true));
          assert.equal(next.field(weightSession), null);
        }
        f.publish(next);
      }
      await Promise.resolve();
      assert.equal(f.jobs.length + f.timers.size, 0);
      assert.equal(f.root.hidden, true);
      assert.equal(f.root.textContent, '');
    } finally {
      f.destroy();
    }
    assert.equal(f.listeners.size + f.timers.size, 0);
  });

test('two queued current spatial intents retain only the newest captured point', async () => {
  const f = loraFixture('pointer');
  try {
    const session = f.session();
    showWeightPreview(f.view, session, { x: 40, y: 70 });
    showWeightPreview(f.view, session, { x: 130, y: 90 });
    await Promise.resolve();
    assert.equal(f.jobs.length, 1);
    f.measure();
    f.measure();
    assert.equal(f.root.style.left, '105px');
    assert.equal(f.root.style.top, '66px');
    assert.equal(f.root.textContent, '1.10');
    assert.equal(f.timers.size, 1);
  } finally {
    f.destroy();
  }
});
