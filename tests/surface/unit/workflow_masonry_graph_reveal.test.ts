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
/** Prove graph-only node fades preserve native state and exclude projected or nested nodes. */
import { afterEach, expect, test } from '@jest/globals';
import { WorkflowMasonryGraphReveal } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryGraphReveal.js';
import type { WorkflowMasonrySection } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryProjection.js';

afterEach(() => document.body.replaceChildren());

/** Build identified native graph cards without borrowing a second presentation. */
function node(id: string): HTMLElement {
  const element = document.createElement('article');
  element.className = 'lg-node';
  element.dataset.nodeId = id;
  return element;
}

test('fades an unprojected decode concurrently and restores its exact native opacity', () => {
  const root = document.createElement('div');
  const sampler = node('1');
  const decode = node('2');
  decode.style.setProperty('opacity', '0.8', 'important');
  const nested = node('3');
  sampler.append(nested);
  root.append(sampler, decode);
  document.body.append(root);
  const sections: WorkflowMasonrySection[] = [
    {
      id: 'loose:1',
      label: 'Sampler',
      cube: null,
      target: { _nodes: [] },
      cards: [
        {
          id: 'root:1',
          node: { id: 1 },
          visible: true,
          enabled: true,
          showActivationControl: false,
          label: 'Sampler',
          columnSpan: 1,
        },
      ],
    },
  ];
  const reveal = new WorkflowMasonryGraphReveal();
  reveal.reconcile(root, sections);
  expect(reveal.frames(true)).toEqual([
    { element: decode, frames: [{ opacity: '0.8' }, { opacity: '0' }] },
  ]);
  expect(decode.style.opacity).toBe('0');
  expect(sampler.style.opacity).toBe('');
  expect(nested.style.opacity).toBe('');
  expect(reveal.frames(false)[0]?.frames).toEqual([{ opacity: '0' }, { opacity: '0.8' }]);
  expect(decode.style.getPropertyPriority('opacity')).toBe('');
  reveal.frames(true);
  reveal.dispose();
  expect(decode.style.opacity).toBe('0.8');
  expect(decode.style.getPropertyPriority('opacity')).toBe('important');
});

test('hides newly mounted graph-only nodes and releases removed nodes during topology reconciliation', () => {
  const root = document.createElement('div');
  document.body.append(root);
  const first = node('1');
  root.append(first);
  const reveal = new WorkflowMasonryGraphReveal();
  reveal.reconcile(root, []);
  reveal.frames(true);
  first.remove();
  const second = node('2');
  root.append(second);
  reveal.reconcile(root, []);
  expect(first.style.opacity).toBe('');
  expect(second.style.opacity).toBe('0');
  reveal.dispose();
  expect(second.style.opacity).toBe('');
});
