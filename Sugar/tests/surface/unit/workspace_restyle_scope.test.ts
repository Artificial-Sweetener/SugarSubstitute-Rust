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
/**
 * Keep relational rules off the page root and the workspace container, and keep the attributes
 * that replace them in step with what they describe.
 *
 * A `:has()` anchored on an element that holds the whole workspace makes the browser restyle
 * everything beneath it after every change anywhere inside it. Switching between masonry and graph
 * makes hundreds of such changes; with two such rules a switch blocked the page for a second.
 */
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { expect, test } from '@jest/globals';
import { setHidden } from '../../../frontend/comfyui/ui/core/HiddenAttribute.js';
import {
  GENERATION_QUEUE_DOCK,
  GENERATION_QUEUE_DOCKED_ATTRIBUTE,
  GenerationQueueDock,
} from '../../../frontend/comfyui/ui/surface/frame/GenerationQueueDock.js';
import {
  MASONRY_MOUNTED_ATTRIBUTE,
  markMasonryMounted,
} from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryStyles.js';

/** Every authored page source. */
function sources(folder = 'frontend'): string[] {
  return readdirSync(folder, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(folder, entry.name);
    if (entry.isDirectory()) return entry.name === 'node_modules' ? [] : sources(file);
    return /\.(ts|vue)$/.test(entry.name) && !entry.name.endsWith('.d.ts') ? [file] : [];
  });
}

test('no rule asks the page root or the workspace container what it has', () => {
  const wide =
    /(?::root|\bhtml|\bbody|#graph-canvas-container|#vue-app)(?:\[[^\]]*\]|\.[\w-]+)*:has\(/;
  const offenders = sources().filter((file) => wide.test(readFileSync(file, 'utf8')));
  expect(offenders).toEqual([]);
});

test('the workspace container states whether a masonry is mounted in it', () => {
  const root = document.createElement('div');
  root.id = 'graph-canvas-container';
  const panel = document.createElement('div');
  root.append(panel);
  document.body.append(root);
  markMasonryMounted(panel);
  expect(root.hasAttribute(MASONRY_MOUNTED_ATTRIBUTE)).toBe(false);
  const masonry = document.createElement('div');
  masonry.className = 'sugarcubes-workflow-masonry';
  panel.append(masonry);
  markMasonryMounted(panel);
  expect(root.hasAttribute(MASONRY_MOUNTED_ATTRIBUTE)).toBe(true);
  // A second surface may overlap the first while one replaces the other.
  const next = masonry.cloneNode() as HTMLElement;
  panel.append(next);
  masonry.remove();
  markMasonryMounted(panel);
  expect(root.hasAttribute(MASONRY_MOUNTED_ATTRIBUTE)).toBe(true);
  next.remove();
  markMasonryMounted(panel);
  expect(root.hasAttribute(MASONRY_MOUNTED_ATTRIBUTE)).toBe(false);
  root.remove();
});

test('the page root states whether the queue is docked, and forgets it with its owner', async () => {
  const root = document.documentElement;
  const dock = new GenerationQueueDock(document, {}, () => undefined);
  expect(root.hasAttribute(GENERATION_QUEUE_DOCKED_ATTRIBUTE)).toBe(false);
  const panel = document.createElement('div');
  panel.setAttribute('data-testid', GENERATION_QUEUE_DOCK.replace(/^\[data-testid="|"\]$/g, ''));
  document.body.append(panel);
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(root.hasAttribute(GENERATION_QUEUE_DOCKED_ATTRIBUTE)).toBe(true);
  panel.remove();
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(root.hasAttribute(GENERATION_QUEUE_DOCKED_ATTRIBUTE)).toBe(false);
  document.body.append(panel);
  await new Promise((resolve) => setTimeout(resolve, 0));
  dock.dispose();
  expect(root.hasAttribute(GENERATION_QUEUE_DOCKED_ATTRIBUTE)).toBe(false);
  panel.remove();
});

test('a visibility that has not changed is not written again', () => {
  const element = document.createElement('div');
  let writes = 0;
  const toggle = element.toggleAttribute.bind(element);
  const set = element.setAttribute.bind(element);
  const remove = element.removeAttribute.bind(element);
  element.toggleAttribute = (...values) => ((writes += 1), toggle(...values));
  element.setAttribute = (...values) => ((writes += 1), set(...values));
  element.removeAttribute = (...values) => ((writes += 1), remove(...values));
  setHidden(element, false);
  expect(writes).toBe(0);
  setHidden(element, true);
  setHidden(element, true);
  expect(element.hidden).toBe(true);
  const afterHide = writes;
  setHidden(element, true);
  expect(writes).toBe(afterHide);
  setHidden(element, false);
  expect(element.hidden).toBe(false);
});
