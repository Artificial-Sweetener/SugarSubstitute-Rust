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
  MASONRY_HIDES_ATTRIBUTE,
  MASONRY_STILLS_ATTRIBUTE,
  ensureWorkflowMasonryStyles,
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

test('graph chrome is marked for as long as a masonry is mounted in its container', () => {
  const root = document.createElement('div');
  root.id = 'graph-canvas-container';
  const panel = document.createElement('div');
  const toolbar = document.createElement('div');
  toolbar.setAttribute('aria-label', 'Canvas Toolbar');
  const toolbox = document.createElement('div');
  toolbox.dataset.testid = 'selection-toolbox';
  panel.append(toolbar, toolbox);
  root.append(panel);
  document.body.append(root);
  const marks = (): [boolean, string | null] => [
    toolbar.hasAttribute(MASONRY_STILLS_ATTRIBUTE),
    toolbox.getAttribute(MASONRY_HIDES_ATTRIBUTE),
  ];
  markMasonryMounted(panel);
  expect(marks()).toEqual([false, null]);
  const masonry = document.createElement('div');
  masonry.className = 'sugarcubes-workflow-masonry';
  panel.append(masonry);
  markMasonryMounted(panel);
  expect(marks()).toEqual([true, 'mounted']);
  // A second surface may overlap the first while one replaces the other.
  const next = masonry.cloneNode() as HTMLElement;
  panel.append(next);
  masonry.remove();
  markMasonryMounted(panel);
  expect(marks()).toEqual([true, 'mounted']);
  next.remove();
  markMasonryMounted(panel);
  expect(marks()).toEqual([false, null]);
  // The container itself carries no state for a rule to reach down from.
  expect(root.attributes.length).toBe(1);
  root.remove();
});

test('no masonry rule reaches from the workspace container to something inside it', () => {
  ensureWorkflowMasonryStyles(document);
  const css = document.getElementById('sugarcubes-workflow-masonry-styles')?.textContent ?? '';
  const reaching = css
    .split('}')
    .flatMap((rule) => (rule.split('{')[0] ?? '').split(','))
    .map((selector) => selector.trim())
    .filter((selector) => /^#graph-canvas-container\[[^\]]*\]\s*\S/.test(selector));
  expect(reaching).toEqual([]);
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

test('no rule keyed on the workspace container reaches for everything beneath it', () => {
  // `#graph-canvas-container[state] ... *` is checked against every element in the container each
  // time the state changes; the state belongs on the element whose subtree the rule is about.
  const universal = /#graph-canvas-container(?:\[[^\]]*\])+[^{},;`]*[\s>]\*\s*[,{]/;
  const offenders = sources().filter((file) => universal.test(readFileSync(file, 'utf8')));
  expect(offenders).toEqual([]);
});

test('everything beside the masonry’s own workspace carries a mark while a masonry hides the graph', async () => {
  const { WorkflowMasonryGraphVisibility } = await import(
    '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryGraphVisibility.js'
  );
  const { MASONRY_HIDES_ATTRIBUTE } = await import(
    '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryStyles.js'
  );
  const root = document.createElement('div');
  root.id = 'graph-canvas-container';
  const canvas = document.createElement('canvas');
  canvas.id = 'graph-canvas';
  const overlay = document.createElement('div');
  overlay.className = 'ph-no-capture';
  const other = document.createElement('div');
  const panel = document.createElement('div');
  root.append(canvas, overlay, other, panel);
  document.body.append(root);
  const visibility = new WorkflowMasonryGraphVisibility(() => panel);
  visibility.setHidden(true);
  expect(canvas.getAttribute(MASONRY_HIDES_ATTRIBUTE)).toBe('graph');
  expect(overlay.getAttribute(MASONRY_HIDES_ATTRIBUTE)).toBe('graph');
  // Not a known graph layer, and hidden all the same: only the masonry's workspace is kept.
  expect(other.getAttribute(MASONRY_HIDES_ATTRIBUTE)).toBe('graph');
  expect(panel.hasAttribute(MASONRY_HIDES_ATTRIBUTE)).toBe(false);
  expect(root.dataset.sugarcubesMasonry).toBe('true');
  // A mark another owner set on a layer is not this owner's to clear.
  overlay.setAttribute(MASONRY_HIDES_ATTRIBUTE, 'mounted');
  visibility.setHidden(false);
  expect(canvas.hasAttribute(MASONRY_HIDES_ATTRIBUTE)).toBe(false);
  expect(overlay.getAttribute(MASONRY_HIDES_ATTRIBUTE)).toBe('mounted');
  expect(root.dataset.sugarcubesMasonry).toBeUndefined();
  root.remove();
});

test('a stylesheet that is already last in the head is not moved', async () => {
  const { ensureOwnedControlAppearanceStyles } = await import(
    '../../../frontend/comfyui/ui/surface/appearance/OwnedControlAppearanceStyles.js'
  );
  ensureOwnedControlAppearanceStyles(document);
  const records: MutationRecord[] = [];
  const observer = new MutationObserver((batch) => records.push(...batch));
  observer.observe(document.head, { childList: true });
  ensureOwnedControlAppearanceStyles(document);
  ensureOwnedControlAppearanceStyles(document);
  await Promise.resolve();
  expect(records).toEqual([]);
  // A sheet added after it gives it back its place at the end, once.
  const later = document.createElement('style');
  document.head.append(later);
  await Promise.resolve();
  records.length = 0;
  ensureOwnedControlAppearanceStyles(document);
  await Promise.resolve();
  expect(document.head.lastElementChild?.id).toBe('sugarcubes-owned-control-appearance');
  expect(records.length).toBeGreaterThan(0);
  observer.disconnect();
  later.remove();
});

test('something empty always follows the masonry surface and its content', async () => {
  const { WorkflowMasonryView } = await import(
    '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryView.js'
  );
  const { CubeFaceCardStateService } = await import(
    '../../../frontend/comfyui/ui/surface/CubeFaceCardStateService.js'
  );
  const { CubeNodeCatalog } = await import(
    '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js'
  );
  const view = new WorkflowMasonryView({
    document,
    sections: [],
    cardState: new CubeFaceCardStateService({ nodes: new CubeNodeCatalog(), history: {} }),
    openCube() {},
  });
  // An element that starts or stops being its parent's last child is restyled with everything
  // in it; a layer or rail appended beside the cards must displace only something empty.
  const content = view.element.querySelector('.sugarcubes-workflow-masonry__content');
  expect(view.element.lastElementChild).not.toBe(content);
  expect(view.element.lastElementChild?.childElementCount).toBe(0);
  expect(view.tail.hidden).toBe(true);
  expect(view.tail.childElementCount).toBe(0);
  view.dispose();
});
