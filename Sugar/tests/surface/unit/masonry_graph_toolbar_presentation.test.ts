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
/** Verify graph-anchored selection controls follow the complete masonry surface lifetime. */
import { beforeEach, afterEach, expect, test } from '@jest/globals';
import { ensureWorkflowMasonryStyles } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryStyles.js';

beforeEach(() => {
  const style = document.createElement('style');
  style.textContent =
    '.selection-toolbox button, .graph-canvas-panel button { pointer-events:auto; }';
  document.head.append(style);
});

afterEach(() => {
  document.head.replaceChildren();
  document.body.replaceChildren();
});

/** Build the native fixed toolbox sibling independently of graph node layers. */
function toolbox() {
  const wrapper = document.createElement('div');
  wrapper.style.cssText = 'position:fixed;z-index:40;transform:translate(120px,180px)';
  const panel = document.createElement('div');
  panel.dataset.testid = 'selection-toolbox';
  panel.className = 'selection-toolbox';
  const button = document.createElement('button');
  button.textContent = 'Save Cube';
  panel.append(button);
  wrapper.append(panel);
  return { wrapper, panel, button };
}

/** Model native navigation beside the allocated masonry workspace, including minimap chrome. */
function graphNavigation() {
  const panel = document.createElement('div');
  panel.className = 'graph-canvas-panel';
  const workspace = document.createElement('div');
  workspace.className = 'sc-output-workspace-content';
  const toolbar = document.createElement('div');
  toolbar.setAttribute('aria-label', 'Canvas Toolbar');
  const zoom = document.createElement('button');
  toolbar.append(zoom);
  const minimap = document.createElement('div');
  minimap.className = 'minimap-main-container';
  const map = document.createElement('div');
  map.className = 'litegraph-minimap';
  const settings = document.createElement('button');
  minimap.append(map, settings);
  panel.append(toolbar, minimap, workspace);
  return { panel, workspace, toolbar, zoom, minimap, settings };
}

test('suppresses navigation with root masonry state across workspace allocations and restores native controls', () => {
  ensureWorkflowMasonryStyles(document);
  const root = document.createElement('div');
  root.id = 'graph-canvas-container';
  const native = graphNavigation();
  const unrelated = graphNavigation();
  root.append(native.panel);
  document.body.append(root, unrelated.panel);
  const surface = document.createElement('div');
  surface.className = 'sugarcubes-workflow-masonry';
  for (const parent of [native.panel, native.workspace]) {
    parent.append(surface);
    root.dataset.sugarcubesMasonry = 'true';
    for (const element of [native.toolbar, native.minimap]) {
      const style = getComputedStyle(element);
      expect(style.visibility).toBe('hidden');
    }
    for (const element of [native.zoom, native.settings])
      expect(getComputedStyle(element).pointerEvents).toBe('none');
    expect(getComputedStyle(surface).visibility).toBe('visible');
    expect(getComputedStyle(unrelated.toolbar).visibility).toBe('visible');
    delete root.dataset.sugarcubesMasonry;
    for (const element of [native.toolbar, native.minimap]) {
      expect(getComputedStyle(element).visibility).toBe('visible');
    }
    expect(getComputedStyle(native.zoom).pointerEvents).toBe('none');
    surface.remove();
    for (const element of [native.zoom, native.settings])
      expect(getComputedStyle(element).pointerEvents).toBe('auto');
    expect(root.querySelector('[aria-label="Canvas Toolbar"]')).toBe(native.toolbar);
  }
});

test('suppresses selection controls throughout preparation, travel and exit without losing native identity', () => {
  ensureWorkflowMasonryStyles(document);
  const root = document.createElement('div');
  root.id = 'graph-canvas-container';
  const container = document.createElement('div');
  container.className = 'graph-canvas-panel';
  const native = toolbox();
  const unrelated = toolbox();
  document.body.append(root, unrelated.wrapper);
  root.append(container, native.wrapper);
  const surface = document.createElement('div');
  surface.className = 'sugarcubes-workflow-masonry';
  const original = native.wrapper.getAttribute('style');
  for (let visit = 0; visit < 3; visit += 1) {
    expect(getComputedStyle(native.panel).visibility).toBe('visible');
    surface.dataset.scMasonryPreparing = '';
    container.append(surface);
    for (const phase of ['preparing', 'enter', 'resting', 'exit']) {
      surface.dataset.scMasonryTransition = phase;
      expect(getComputedStyle(native.wrapper).visibility).toBe('hidden');
      expect(getComputedStyle(native.button).pointerEvents).toBe('none');
      expect(getComputedStyle(unrelated.panel).visibility).toBe('visible');
    }
    surface.remove();
    expect(getComputedStyle(native.panel).visibility).toBe('visible');
    expect(getComputedStyle(native.button).pointerEvents).toBe('auto');
    expect(native.wrapper.getAttribute('style')).toBe(original);
    expect(root.querySelector('[data-testid="selection-toolbox"]')).toBe(native.panel);
  }
});

test('suppresses late toolbox mounts and leaves masonry controls and detached menus interactive', () => {
  ensureWorkflowMasonryStyles(document);
  const root = document.createElement('div');
  root.id = 'graph-canvas-container';
  const surface = document.createElement('div');
  surface.className = 'sugarcubes-workflow-masonry';
  const control = document.createElement('button');
  control.textContent = 'Edit Cube';
  surface.append(control);
  root.append(surface);
  document.body.append(root);
  const native = toolbox();
  root.append(native.wrapper);
  expect(getComputedStyle(native.wrapper).visibility).toBe('hidden');
  expect(getComputedStyle(native.button).pointerEvents).toBe('none');
  expect(getComputedStyle(control).visibility).toBe('visible');
  expect(getComputedStyle(control).pointerEvents).not.toBe('none');
  surface.remove();
  expect(getComputedStyle(native.button).pointerEvents).toBe('auto');
});
