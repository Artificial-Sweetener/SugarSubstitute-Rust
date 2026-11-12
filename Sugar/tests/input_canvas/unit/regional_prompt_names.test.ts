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
/** Prove that the prompts related to one regional mask loader share their separator names. */
import { jest } from '@jest/globals';
import { synchronizeRegionalPromptNames } from '../../../frontend/comfyui/ui/input_canvas/domain/RegionalPromptNames.js';
import { ComfyRegionalPromptNames } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyRegionalPromptNames.js';
import type { RegionalMask } from '../../../frontend/comfyui/ui/input_canvas/application/WorkflowRegionalPrompts.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

test('an edited name propagates and every other ordinal takes its first-authored name', () => {
  expect(
    synchronizeRegionalPromptNames([
      { text: 'a\n[SEP|sky]\nb\n[SEP]\nc', previous: 'a\n[SEP|sea]\nb\n[SEP]\nc' },
      { text: 'x\n[SEP|sea]\ny\n[SEP|ground]\nz', previous: null },
      { text: 'only global', previous: null },
    ]),
  ).toEqual(['a\n[SEP|sky]\nb\n[SEP|ground]\nc', 'x\n[SEP|sky]\ny\n[SEP|ground]\nz', null]);
});

test('clearing a name clears it everywhere and separators are never added', () => {
  expect(
    synchronizeRegionalPromptNames([
      { text: 'a\n[SEP]\nb', previous: 'a\n[SEP|sky]\nb' },
      { text: 'x\n[SEP|sky]\ny\n[SEP|ground]\nz', previous: null },
    ]),
  ).toEqual([null, 'x\n[SEP]\ny\n[SEP|ground]\nz']);
  expect(
    synchronizeRegionalPromptNames([
      { text: 'a\n[SEP|sky]\nb', previous: null },
      { text: 'x\n[SEP|sky]\ny', previous: null },
    ]),
  ).toEqual([null, null]);
});

/** Build a live multiline prompt node whose widget commits record their order. */
function prompt(id: number, text: string, steps: string[]): ComfyNode {
  const node: ComfyNode = {
    id,
    type: 'PrimitiveStringMultiline',
    widgets: [{ name: 'value', value: text, callback: () => steps.push('callback ' + String(id)) }],
  };
  node.onWidgetChanged = () => steps.push('changed ' + String(id));
  return node;
}

/** Relate prompts to one ordered loader through a notifying regional owner. */
function names(
  nodes: Record<string, ComfyNode>,
  steps: string[],
  related: readonly string[] = Object.keys(nodes),
) {
  const listeners = new Set<() => void>();
  let masks: readonly RegionalMask[] = [
    { endpoint: 'regions', entries: ['r0'], names: [null], prompts: related },
  ];
  const report = jest.fn();
  const owner = new ComfyRegionalPromptNames(
    {
      masks: () => masks,
      subscribe: (listener) => {
        listeners.add(listener);
        return () => listeners.delete(listener);
      },
    },
    (path) => nodes[path] ?? null,
    () => ({
      beforeChange: () => steps.push('before'),
      afterChange: () => steps.push('after'),
    }),
    report,
  );
  const notify = () => {
    for (const listener of [...listeners]) listener();
  };
  return {
    owner,
    report,
    notify,
    relate(next: readonly RegionalMask[]) {
      masks = next;
      notify();
    },
  };
}

/** Read a prompt node's authored text. */
function text(node: ComfyNode | undefined): unknown {
  return node?.widgets?.[0]?.value;
}

test('the first reading records authored names without modifying the workflow', () => {
  const steps: string[] = [];
  const nodes = {
    positive: prompt(1, 'a\n[SEP|sky]\nb', steps),
    negative: prompt(2, 'x\n[SEP|sea]\ny', steps),
  };
  const n = names(nodes, steps);
  n.notify();
  n.notify();
  expect(steps).toEqual([]);
  expect(text(nodes.negative)).toBe('x\n[SEP|sea]\ny');
  n.owner.dispose();
});

test('an edit in one prompt rewrites its peers in one native history step', () => {
  const steps: string[] = [];
  const nodes = {
    positive: prompt(1, 'a\n[SEP|sky]\nb', steps),
    negative: prompt(2, 'x\n[SEP|sky]\ny', steps),
    unrelated: prompt(3, 'u\n[SEP|other]\nv', steps),
  };
  const n = names(nodes, steps, ['positive', 'negative']);
  n.notify();
  nodes.positive.widgets![0]!.value = 'a\n[SEP|person]\nb';
  n.notify();
  expect(text(nodes.negative)).toBe('x\n[SEP|person]\ny');
  expect(text(nodes.positive)).toBe('a\n[SEP|person]\nb');
  expect(text(nodes.unrelated)).toBe('u\n[SEP|other]\nv');
  expect(steps).toEqual(['before', 'callback 2', 'changed 2', 'after']);
  n.notify();
  expect(steps).toHaveLength(4);
  expect(n.report).not.toHaveBeenCalled();
  n.owner.dispose();
  nodes.positive.widgets![0]!.value = 'a\n[SEP|sea]\nb';
  n.notify();
  expect(text(nodes.negative)).toBe('x\n[SEP|person]\ny');
});

test('a prompt that joins a loader is recorded first and synchronized on its next edit', () => {
  const steps: string[] = [];
  const nodes = {
    positive: prompt(1, 'a\n[SEP|sky]\nb', steps),
    negative: prompt(2, 'x\n[SEP]\ny\n[SEP|ground]\nz', steps),
  };
  const n = names(nodes, steps, ['positive']);
  n.notify();
  n.relate([
    { endpoint: 'regions', entries: ['r0'], names: [null], prompts: ['positive', 'negative'] },
  ]);
  expect(steps).toEqual([]);
  nodes.negative.widgets![0]!.value = 'x\n[SEP]\ny\n[SEP|grass]\nz';
  n.notify();
  expect(text(nodes.negative)).toBe('x\n[SEP|sky]\ny\n[SEP|grass]\nz');
  expect(text(nodes.positive)).toBe('a\n[SEP|sky]\nb');
  expect(steps).toEqual(['before', 'callback 2', 'changed 2', 'after']);
});
