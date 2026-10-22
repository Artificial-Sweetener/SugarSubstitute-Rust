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
/** Prove reversible native layout and actual queue-button ownership through Substitute presentation. */
import { jest } from '@jest/globals';
import { SubstituteToolbarLayout } from '../../../frontend/comfyui/ui/surface/frame/SubstituteToolbarLayout.js';
import { ComfyGenerationQueueSurface } from '../../../frontend/comfyui/ui/surface/frame/ComfyGenerationQueueSurface.js';
import { substituteFrameCss } from '../../../frontend/comfyui/ui/surface/frame/SubstituteFrameStyles.js';
import { workflowTabCss } from '../../../frontend/comfyui/ui/surface/frame/WorkflowTabStyles.js';

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
  document.documentElement.removeAttribute('data-sc-substitute-frame');
  jest.restoreAllMocks();
});

test('frame paint stays independent of view mode and queue overflow cannot expand the opaque row', () => {
  const style = document.createElement('style');
  style.textContent = substituteFrameCss;
  document.head.append(style);
  const frame = document.createElement('header');
  frame.className = 'sc-substitute-frame';
  document.body.append(frame);
  const rules = [...(style.sheet?.cssRules ?? [])].filter(
    // JSDOM represents unsupported @property registrations as CSSStyleRule instances.
    (rule): rule is CSSStyleRule =>
      rule instanceof CSSStyleRule && !rule.selectorText.startsWith('@'),
  );
  const framePaint = rules.find(
    (rule) => frame.matches(rule.selectorText) && rule.style.getPropertyValue('background'),
  );
  expect(framePaint?.style.getPropertyValue('background')).toBe(
    'var(--sc-native-frame-background,#000)',
  );
  expect(framePaint?.style.getPropertyValue('--sc-frame-muted')).toBe(
    'var(--sc-native-frame-foreground,rgba(255,255,255,.788))',
  );
  frame.dataset.colorScheme = 'light';
  expect(
    rules
      .filter((rule) => frame.matches(rule.selectorText) && rule !== framePaint)
      .some((rule) => rule.style.getPropertyValue('--sc-frame-muted')),
  ).toBe(false);

  const menu = document.createElement('div');
  menu.className = 'native-toolbar-menu';
  const nativeSpacing = document.createElement('style');
  nativeSpacing.textContent = '.native-toolbar-menu { padding-top:4px; gap:4px; }';
  document.head.append(nativeSpacing);
  menu.setAttribute('data-sc-substitute-toolbar-menu', '');
  const line = document.createElement('div');
  line.setAttribute('data-sc-substitute-toolbar-line', '');
  const progress = document.createElement('div');
  menu.append(line, progress);
  document.body.append(menu);
  document.documentElement.setAttribute('data-sc-substitute-frame', '');
  const layout = new SubstituteToolbarLayout(document);
  const toolbarStyle = document.head.lastElementChild;
  if (!(toolbarStyle instanceof HTMLStyleElement) || !toolbarStyle.sheet)
    throw new Error('Substitute toolbar stylesheet is missing');
  const toolbarRules = [...toolbarStyle.sheet.cssRules].filter(
    (rule): rule is CSSStyleRule => rule instanceof CSSStyleRule,
  );
  for (const surface of [menu, line]) {
    const paint = toolbarRules.find(
      (rule) => surface.matches(rule.selectorText) && rule.style.getPropertyValue('background'),
    );
    expect(paint?.style.getPropertyValue('background')).toBe('var(--comfy-menu-bg,#202020)');
  }
  expect(getComputedStyle(line).height).toBe('44px');
  expect(getComputedStyle(menu).paddingTop).toBe('0px');
  expect(getComputedStyle(menu).gap).toBe('0');
  const queue = document.createElement('div');
  queue.style.height = '600px';
  line.append(queue);
  expect(getComputedStyle(line).height).toBe('44px');
  expect(getComputedStyle(line).overflow).toBe('visible');
  expect(getComputedStyle(queue).height).toBe('600px');
  document.documentElement.removeAttribute('data-sc-substitute-frame');
  expect(getComputedStyle(menu).paddingTop).toBe('4px');
  expect(getComputedStyle(menu).gap).toBe('4px');
  layout.dispose();
});

test('selected tabs share the opaque toolbar surface in either palette and while held', () => {
  const style = document.createElement('style');
  style.textContent = workflowTabCss;
  document.head.append(style);
  const rules = [...(style.sheet?.cssRules ?? [])].filter(
    (rule): rule is CSSStyleRule => rule instanceof CSSStyleRule,
  );
  const selectedPaint = rules.filter(
    (rule) =>
      rule.selectorText.includes("[aria-selected='true']") &&
      !rule.selectorText.includes(':not(') &&
      !rule.selectorText.includes('::after') &&
      rule.style.getPropertyValue('background'),
  );
  expect(selectedPaint).toHaveLength(1);
  expect(selectedPaint[0]?.style.getPropertyValue('background')).toBe(
    'var(--comfy-menu-bg,#202020)',
  );
});

test('Substitute workspace removes native side margins without moving unrelated splitters', () => {
  const nativeStyle = document.createElement('style');
  nativeStyle.textContent = '.splitter-overlay-bottom { margin:0 4px 4px; }';
  const frameStyle = document.createElement('style');
  frameStyle.textContent = substituteFrameCss;
  document.head.append(nativeStyle, frameStyle);
  const workspace = document.createElement('div');
  workspace.className = 'splitter-overlay-bottom';
  const graph = document.createElement('div');
  graph.className = 'graph-canvas-panel';
  workspace.append(graph);
  const unrelated = document.createElement('div');
  unrelated.className = 'splitter-overlay-bottom';
  document.body.append(workspace, unrelated);
  document.documentElement.setAttribute('data-sc-substitute-frame', '');
  expect(getComputedStyle(workspace).marginLeft).toBe('0px');
  expect(getComputedStyle(workspace).marginRight).toBe('0px');
  expect(getComputedStyle(workspace).marginBottom).toBe('4px');
  expect(getComputedStyle(unrelated).marginLeft).toBe('4px');
  document.documentElement.removeAttribute('data-sc-substitute-frame');
  expect(getComputedStyle(workspace).marginLeft).toBe('4px');
  expect(getComputedStyle(workspace).marginRight).toBe('4px');
});

test('workflow and native toolbar wrappers receive reversible marks without moving host controls', async () => {
  const line = document.createElement('div');
  const workflowColumn = document.createElement('div');
  const workflowGroup = document.createElement('div');
  const workflow = document.createElement('button');
  workflow.setAttribute('aria-label', 'Workflow actions');
  workflowGroup.append(workflow);
  workflowColumn.append(workflowGroup);
  const column = document.createElement('div');
  const bar = document.createElement('div');
  bar.className = 'actionbar-container';
  column.append(bar);
  line.append(workflowColumn, column);
  document.body.append(line);
  const layout = new SubstituteToolbarLayout(document);
  expect(workflow.parentElement).toBe(workflowGroup);
  expect(bar.parentElement).toBe(column);
  expect(line.hasAttribute('data-sc-substitute-toolbar-line')).toBe(true);
  expect(workflowColumn.hasAttribute('data-sc-substitute-workflow-column')).toBe(true);
  expect(column.hasAttribute('data-sc-substitute-toolbar-column')).toBe(true);
  const replacement = document.createElement('div');
  replacement.className = 'actionbar-container';
  bar.replaceWith(replacement);
  await Promise.resolve();
  expect(bar.hasAttribute('data-sc-substitute-toolbar')).toBe(false);
  expect(replacement.hasAttribute('data-sc-substitute-toolbar')).toBe(true);
  layout.dispose();
  expect(document.querySelector('[data-sc-substitute-toolbar-line]')).toBeNull();
});

test('Extensions stays off the override row through late mounts and restores on mode exit', async () => {
  const row = document.createElement('div');
  const bar = document.createElement('div');
  bar.className = 'actionbar-container';
  const overrides = document.createElement('div');
  overrides.className = 'sc-overrides-dock-slot';
  bar.append(overrides);
  row.append(bar);
  document.body.append(row);
  document.documentElement.setAttribute('data-sc-substitute-frame', '');
  const layout = new SubstituteToolbarLayout(document);
  const wrapper = document.createElement('div');
  const extensions = document.createElement('button');
  extensions.setAttribute('aria-label', 'Extensions traduites');
  const icon = document.createElement('i');
  icon.className = 'icon-[comfy--extensions-blocks] size-4';
  extensions.append(icon);
  wrapper.append(extensions);
  const open = jest.fn();
  extensions.addEventListener('click', open);
  row.prepend(wrapper);
  await Promise.resolve();
  expect(getComputedStyle(wrapper).display).toBe('none');
  expect(getComputedStyle(bar).display).not.toBe('none');
  expect(getComputedStyle(overrides).display).not.toBe('none');
  expect(extensions.parentElement).toBe(wrapper);
  const replacement = wrapper.cloneNode(true);
  if (!(replacement instanceof HTMLElement)) throw new Error('Expected a native manager wrapper');
  wrapper.replaceWith(replacement);
  await Promise.resolve();
  expect(wrapper.hasAttribute('data-sc-substitute-extensions')).toBe(false);
  expect(getComputedStyle(replacement).display).toBe('none');
  document.documentElement.removeAttribute('data-sc-substitute-frame');
  expect(getComputedStyle(replacement).display).not.toBe('none');
  layout.dispose();
  expect(document.querySelector('[data-sc-substitute-extensions]')).toBeNull();
  extensions.click();
  expect(open).toHaveBeenCalledTimes(1);
});

test('queue keeps its original parent, listeners and popover anchor while counts and geometry update', async () => {
  const parent = document.createElement('div');
  const button = document.createElement('button');
  button.dataset.testid = 'queue-overlay-toggle';
  const count = document.createElement('span');
  count.textContent = '3 active';
  button.append(count);
  parent.append(button);
  const frame = document.createElement('header');
  const slot = document.createElement('span');
  frame.append(slot);
  document.body.append(parent, frame);
  jest.spyOn(slot, 'getBoundingClientRect').mockReturnValue({
    x: 1200,
    y: 0,
    left: 1200,
    top: 0,
    right: 1240,
    bottom: 30,
    width: 40,
    height: 30,
    toJSON: () => ({ x: 1200, y: 0, width: 40, height: 30 }),
  });
  const open = jest.fn();
  button.addEventListener('click', open);
  const surface = new ComfyGenerationQueueSurface(document, frame, slot);
  expect(button.parentElement).toBe(parent);
  expect(button.style.getPropertyValue('--sc-queue-left')).toBe('1200px');
  expect(button.querySelector('.sc-generation-queue-badge')?.textContent).toBe('3');
  button.click();
  expect(open).toHaveBeenCalledTimes(1);
  count.textContent = '0 active';
  await Promise.resolve();
  expect(button.querySelector<HTMLElement>('.sc-generation-queue-badge')?.hidden).toBe(true);
  count.textContent = '2 active';
  button.replaceChildren(count);
  await Promise.resolve();
  expect(button.querySelector('.sc-generation-queue-badge')?.textContent).toBe('2');
  surface.setPalette(false);
  expect(button.dataset.scQueueTheme).toBe('light');
  surface.dispose();
  expect(button.parentElement).toBe(parent);
  expect(button.getAttribute('aria-label')).toBeNull();
  expect(button.textContent).toBe('2 active');
  expect(button.style.getPropertyValue('--sc-queue-left')).toBe('');
  button.click();
  expect(open).toHaveBeenCalledTimes(2);
});
