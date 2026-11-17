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
/** The shipped completion extension hands model candidates to a host gallery and keeps selection, keys and acceptance native. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { EditorView } from '@codemirror/view';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { completionSession } from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';
import { mountModelCompletion, type ModelRow } from '../transaction/model-completion-fixture.ts';
import type { CompletionNode } from './completion-dom-fixture.ts';

/** Seven models laid out by the double's three-column grid. */
const gridModels: readonly ModelRow[] = Array.from({ length: 7 }, (_, index) => ({
  promptName: `grid/model${index}`,
}));

/** Every node below `node`, for asserting what the tooltip contains. */
function descendants(node: CompletionNode): CompletionNode[] {
  return node.children.flatMap((child) => [child, ...descendants(child)]);
}

/** The single mounted tooltip's root node. */
function only(f: ReturnType<typeof mountModelCompletion>): CompletionNode {
  const tooltips = f.tooltips();
  assert.equal(tooltips.length, 1);
  return tooltips[0]!.dom as unknown as CompletionNode;
}

test('one gallery is mounted per session, follows refinement and selection, and is released on dismissal', () => {
  const f = mountModelCompletion('<lora:', 6);
  try {
    assert.equal(f.refresh()?.surface, 'models');
    assert.equal(f.host.popups.length, 1);
    const popup = f.host.popups[0]!;
    assert.deepEqual(popup.updates, [
      { models: ['folder/midday', 'folder/midnight', 'other/sunrise'], selected: 0 },
    ]);
    f.type('mid');
    assert.equal(f.host.popups.length, 1);
    assert.deepEqual(popup.last(), { models: ['folder/midday', 'folder/midnight'], selected: 0 });
    assert.equal(f.press('ArrowRight'), true);
    assert.deepEqual(popup.last(), { models: ['folder/midday', 'folder/midnight'], selected: 1 });
    // Refinement keeps the chosen model current even when its rank changes.
    f.type('n');
    assert.deepEqual(popup.last(), { models: ['folder/midnight'], selected: 0 });
    assert.equal(f.host.popups.length, 1);
    assert.equal(popup.disposed, 0);
    assert.equal(f.press('Escape'), true);
    assert.equal(f.state().field(completionSession), null);
    assert.equal(f.tooltips().length, 0);
    assert.equal(popup.disposed, 1);
    f.refresh();
    assert.equal(f.host.popups.length, 2);
    assert.equal(popup.disposed, 1);
  } finally {
    f.restore();
  }
  assert.ok(f.host.popups.every((popup) => popup.disposed === 1));
});

test('the model tooltip is chrome-free and holds only the host gallery', () => {
  const f = mountModelCompletion('<lora:mid');
  try {
    f.refresh();
    const root = only(f);
    assert.equal(root.className, 'sp-model-completions');
    assert.equal(root.classList.contains('cm-tooltip-autocomplete'), false);
    assert.deepEqual(root.children, [f.host.popups[0]!.node]);
    assert.equal(
      descendants(root).some((node) => node.classList.contains('sp-completion-row')),
      false,
    );
  } finally {
    f.restore();
  }
});

test('switching between word and model sessions leaves exactly one surface mounted', () => {
  const f = mountModelCompletion('mid');
  try {
    assert.equal(f.refresh()?.surface, 'words');
    assert.equal(only(f).className, 'cm-tooltip-autocomplete');
    assert.equal(f.host.popups.length, 0);
    f.view.dispatch({
      changes: { from: 0, to: 3, insert: '<lora:mid' },
      selection: { anchor: 9 },
    });
    assert.equal(f.refresh()?.surface, 'models');
    assert.equal(only(f).className, 'sp-model-completions');
    assert.equal(f.host.popups.length, 1);
    f.view.dispatch({
      changes: { from: 0, to: 9, insert: 'mid' },
      selection: { anchor: 3 },
    });
    assert.equal(f.refresh()?.surface, 'words');
    assert.equal(only(f).className, 'cm-tooltip-autocomplete');
    assert.equal(f.host.popups[0]!.disposed, 1);
    assert.equal(f.host.popups.length, 1);
  } finally {
    f.restore();
  }
});

test("arrow keys on a model session follow the gallery's neighbour and never move the caret", () => {
  const f = mountModelCompletion('<lora:', 6, gridModels);
  try {
    f.refresh();
    const popup = f.host.popups[0]!,
      doc = f.state().doc;
    const selected = () => f.state().field(completionSession)!.selected;
    for (const [key, index] of [
      ['ArrowRight', 1],
      ['ArrowDown', 4],
      ['ArrowLeft', 3],
      ['ArrowUp', 0],
    ] as const) {
      assert.equal(f.press(key), true, key);
      assert.equal(selected(), index, key);
      assert.equal(popup.last()?.selected, index, key);
      assert.equal(f.state().selection.main.head, 6);
      assert.equal(f.state().doc, doc);
    }
    assert.deepEqual(
      popup.neighborCalls.map((call) => [call.selected, call.key]),
      [
        [0, 'ArrowRight'],
        [1, 'ArrowDown'],
        [4, 'ArrowLeft'],
        [3, 'ArrowUp'],
      ],
    );
    // At the gallery's edge the neighbour is the same candidate: the key is consumed, nothing changes.
    const updates = popup.updates.length,
      state = f.state();
    for (const key of ['ArrowUp', 'ArrowLeft'] as const) {
      assert.equal(f.press(key), true, key);
      assert.equal(f.state(), state);
    }
    assert.equal(popup.updates.length, updates);
    // Without a mounted gallery the key still belongs to the session.
    f.tooltips()[0]!.destroy?.();
    assert.equal(f.press('ArrowDown'), true);
    assert.equal(f.state(), state);
  } finally {
    f.restore();
  }
});

test('arrow keys on a word session wrap through rows, and left or right dismiss it', () => {
  for (const key of ['ArrowLeft', 'ArrowRight'] as const) {
    const f = mountModelCompletion('mid');
    try {
      const result = f.refresh();
      assert.equal(result?.surface, 'words');
      assert.equal(result.options.length, 2);
      const selected = () => f.state().field(completionSession)?.selected;
      assert.equal(f.press('ArrowDown'), true);
      assert.equal(selected(), 1);
      assert.equal(f.press('ArrowDown'), true);
      assert.equal(selected(), 0);
      assert.equal(f.press('ArrowUp'), true);
      assert.equal(selected(), 1);
      assert.equal(f.state().selection.main.head, 3);
      assert.equal(f.press(key), false);
      assert.equal(f.state().field(completionSession), null);
      assert.equal(f.tooltips().length, 0);
      assert.equal(f.host.popups.length, 0);
    } finally {
      f.restore();
    }
  }
});

test('Tab accepts the selected model, keeping a typed weight, and returns focus to the prompt', () => {
  for (const [source, caret, accepted] of [
    ['a, <lora:mid', 12, 'a, <lora:folder/midnight:1.00>'],
    ['a, <lora:mid:0.7>', 12, 'a, <lora:folder/midnight:0.7>'],
  ] as const) {
    const f = mountModelCompletion(source, caret);
    try {
      f.refresh();
      assert.equal(f.press('ArrowRight'), true);
      assert.equal(f.press('Tab'), true);
      assert.equal(readExactSource(f.state()), accepted);
      assert.equal(f.state().selection.main.head, accepted.indexOf('>') + 1);
      assert.equal(f.state().field(completionSession), null);
      assert.equal(f.host.popups[0]!.disposed, 1);
      assert.ok(f.restored() > 0);
    } finally {
      f.restore();
    }
  }
});

test('the gallery accepts the candidate the person activates', () => {
  const f = mountModelCompletion('<lora:mid');
  try {
    f.refresh();
    f.host.popups[0]!.activate(1);
    assert.equal(readExactSource(f.state()), '<lora:folder/midnight:1.00>');
    assert.equal(f.state().field(completionSession), null);
    assert.equal(f.host.popups[0]!.disposed, 1);
    // A released gallery cannot accept again.
    const state = f.state();
    f.host.popups[0]!.activate(0);
    assert.equal(f.state(), state);
  } finally {
    f.restore();
  }
});

test('a choice made while typing refreshes the rows is kept for the current candidate', () => {
  for (const choose of ['keyboard', 'pointer'] as const) {
    const f = mountModelCompletion('<lora:mid');
    try {
      f.refresh();
      assert.equal(f.press('ArrowRight'), true);
      f.view.dispatch({
        changes: { from: 9, insert: 'n' },
        selection: { anchor: 10 },
        userEvent: 'input.type',
      });
      assert.equal(f.state().field(completionSession)?.phase, 'refreshing');
      if (choose === 'keyboard') assert.equal(f.press('Tab'), true);
      else f.host.popups[0]!.activate(1);
      assert.equal(readExactSource(f.state()), '<lora:midn');
      assert.deepEqual(f.state().field(completionSession)?.acceptance, {
        identity: 'lora:folder/midnight.safetensors',
        activation: choose,
      });
    } finally {
      f.restore();
    }
  }
});

test('Enter is not consumed by a model session', () => {
  const f = mountModelCompletion('<lora:mid');
  try {
    f.refresh();
    const doc = f.state().doc;
    assert.equal(f.press('Enter'), false);
    assert.equal(f.state().doc, doc);
    assert.equal(f.state().field(completionSession), null);
    assert.equal(f.host.popups[0]!.disposed, 1);
  } finally {
    f.restore();
  }
});

test('the gallery is bounded by the space below the caret and focus reaching it returns to the prompt', () => {
  const f = mountModelCompletion('<lora:mid');
  try {
    f.refresh();
    const tooltip = f.tooltips()[0]!,
      root = tooltip.dom as unknown as CompletionNode;
    tooltip.positioned?.({ left: 0, right: 800, top: 0, bottom: 510 });
    const job = f.jobs.findLast((candidate) => candidate.key === tooltip)!;
    f.doc.readsAllowed = true;
    const measured = job.read(f.view as EditorView);
    f.doc.readsAllowed = false;
    job.write?.(measured, f.view as EditorView);
    assert.equal(root.style['--sp-completion-outer-height'], '374px');
    const restored = f.restored();
    root.dispatchEvent(new Event('focusin'));
    assert.equal(f.restored(), restored + 1);
    const press = new Event('mousedown', { cancelable: true });
    Object.defineProperty(press, 'button', { value: 0 });
    root.dispatchEvent(press);
    assert.equal(press.defaultPrevented, true);
  } finally {
    f.restore();
  }
});
