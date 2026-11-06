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
/** Preserve the desktop presentation protocol and reversible native frame ownership. */
import { afterEach, expect, test } from '@jest/globals';
import {
  DesktopFramePresentation,
  desktopFrameConfiguration,
} from '../../../frontend/comfyui/ui/surface/frame/DesktopFramePresentation.js';
import { desktopFrameCss } from '../../../frontend/comfyui/ui/surface/frame/DesktopFrameStyles.js';

const configuration = {
  schemaVersion: 1,
  captionWidth: 138,
  titlebarHeight: 34,
  darkTheme: true,
};

afterEach(() => {
  document.body.replaceChildren();
});

test('protocol rejects unsupported geometry, missing fields, and unknown fields before mutation', () => {
  const owner = new DesktopFramePresentation();
  const mount = document.createElement('div');
  document.body.append(mount);
  owner.attach(mount);
  const original = document.documentElement.outerHTML;
  for (const value of [
    null,
    [],
    {},
    { ...configuration, schemaVersion: 2 },
    { ...configuration, captionWidth: 144 },
    { ...configuration, titlebarHeight: 40 },
    { ...configuration, darkTheme: 'true' },
    { ...configuration, shellCommand: 'unused' },
  ])
    expect(() => owner.configure(value)).toThrow(TypeError);
  expect(document.documentElement.outerHTML).toBe(original);
  expect(owner.read()).toBeNull();
  expect(Object.isFrozen(desktopFrameConfiguration(configuration))).toBe(true);
  owner.dispose();
});

test('desktop marks only frame ancestors and restores exact host attributes and property priorities', () => {
  const outer = document.createElement('main');
  const mount = document.createElement('div');
  const graph = document.createElement('canvas');
  mount.setAttribute('data-sc-desktop-frame-host', 'original');
  mount.style.setProperty('--sc-native-caption-width', '10px', 'important');
  outer.append(mount, graph);
  document.body.append(outer);
  const owner = new DesktopFramePresentation();
  owner.configure(configuration);
  expect(owner.read()?.mounted).toBe(false);
  owner.attach(mount);
  expect(owner.read()).toEqual({ ...configuration, mounted: true });
  expect(mount.style.getPropertyValue('--sc-native-caption-width')).toBe('138px');
  expect(mount.style.getPropertyValue('--sc-native-frame-background')).toBe('transparent');
  expect(outer.hasAttribute('data-sc-desktop-frame-surface')).toBe(true);
  expect(document.documentElement.hasAttribute('data-sc-desktop-frame-surface')).toBe(true);
  expect(graph.hasAttribute('data-sc-desktop-frame-surface')).toBe(false);
  expect(mount.style.getPropertyValue('--comfy-menu-bg')).toBe('');
  owner.detach();
  expect(owner.read()?.mounted).toBe(false);
  expect(mount.getAttribute('data-sc-desktop-frame-host')).toBe('original');
  expect(mount.style.getPropertyValue('--sc-native-caption-width')).toBe('10px');
  expect(mount.style.getPropertyPriority('--sc-native-caption-width')).toBe('important');
  expect(document.documentElement.hasAttribute('data-sc-desktop-frame-surface')).toBe(false);
  owner.dispose();
  expect(owner.read()).toBeNull();
});

test('desktop publishes the original Substitute Mica washes for the native theme and retracts them', () => {
  const root = document.documentElement;
  const mount = document.createElement('div');
  document.body.append(mount);
  const owner = new DesktopFramePresentation();
  owner.configure(configuration);
  owner.attach(mount);
  expect(root.style.getPropertyValue('--sc-desktop-body-wash')).toBe('rgba(32, 32, 32, 0.5882)');
  expect(root.style.getPropertyValue('--sc-desktop-chrome-wash')).toBe('rgba(44, 44, 44, 0.5882)');
  expect(root.style.getPropertyValue('--sc-desktop-card-wash')).toBe('rgba(255, 255, 255, 0.0510)');
  owner.configure({ ...configuration, darkTheme: false });
  expect(root.style.getPropertyValue('--sc-desktop-body-wash')).toBe('rgba(251, 251, 251, 0.7373)');
  expect(root.style.getPropertyValue('--sc-desktop-card-wash')).toBe('rgba(255, 255, 255, 0.7020)');
  expect(root.style.getPropertyValue('--sc-desktop-chrome-wash')).toBe(
    'rgba(252, 252, 252, 0.7686)',
  );
  owner.dispose();
  expect(root.style.getPropertyValue('--sc-desktop-body-wash')).toBe('');
  expect(root.style.getPropertyValue('--sc-desktop-chrome-wash')).toBe('');
  expect(root.style.getPropertyValue('--sc-desktop-card-wash')).toBe('');
});

test('remounts retain accepted configuration and later host mutations are not overwritten', () => {
  const first = document.createElement('div');
  const second = document.createElement('div');
  document.body.append(first, second);
  const owner = new DesktopFramePresentation();
  owner.configure(configuration);
  owner.attach(first);
  first.style.setProperty('--sc-native-frame-background', 'red');
  first.setAttribute('data-sc-desktop-frame-host', 'new-owner');
  owner.attach(second);
  expect(first.style.getPropertyValue('--sc-native-frame-background')).toBe('red');
  expect(first.getAttribute('data-sc-desktop-frame-host')).toBe('new-owner');
  expect(second.style.getPropertyValue('--sc-native-caption-width')).toBe('138px');
  owner.configure({ ...configuration, darkTheme: false });
  expect(second.style.getPropertyValue('--sc-native-frame-foreground')).toBe('#000');
  expect(owner.read()?.mounted).toBe(true);
  owner.dispose();
  expect(second.hasAttribute('data-sc-desktop-frame-host')).toBe(false);
});

test('native dragging belongs to frame gaps and leaves existing controls and tab gestures interactive', () => {
  const style = document.createElement('style');
  style.textContent = desktopFrameCss;
  document.head.append(style);
  try {
    const mount = document.createElement('div');
    mount.setAttribute('data-sc-desktop-frame-host', '');
    const frame = document.createElement('header');
    frame.className = 'sc-substitute-frame';
    mount.append(frame);
    document.body.append(mount);
    const property = (element: HTMLElement, name: string): string =>
      [...(style.sheet?.cssRules ?? [])]
        .filter((rule): rule is CSSStyleRule => rule instanceof CSSStyleRule)
        .filter((rule) => element.matches(rule.selectorText))
        .reduce((value, rule) => rule.style.getPropertyValue(name) || value, '');
    expect(property(frame, 'app-region')).toBe('drag');
    for (const tag of ['button', 'input', 'select', 'textarea', 'a']) {
      const control = document.createElement(tag);
      frame.append(control);
      expect(property(control, 'app-region')).toBe('no-drag');
    }
    for (const className of ['sc-workflow-strip', 'sc-app-orb', 'sc-generation-strip']) {
      const control = document.createElement('div');
      control.className = className;
      frame.append(control);
      expect(property(control, 'app-region')).toBe('no-drag');
    }
    const browserFrame = document.createElement('header');
    browserFrame.className = 'sc-substitute-frame';
    document.body.append(browserFrame);
    expect(property(browserFrame, 'app-region')).toBe('');
    expect(property(frame, '--comfy-menu-bg')).toBe('');
  } finally {
    style.remove();
  }
});

test('washes reach the selected tab, toolbar, graph and masonry only inside the desktop frame', () => {
  const style = document.createElement('style');
  style.textContent = desktopFrameCss;
  document.head.append(style);
  const root = document.documentElement;
  try {
    const graph = document.createElement('canvas');
    graph.id = 'graph-canvas';
    const backdrop = document.createElement('div');
    backdrop.className = 'sugarcubes-workflow-masonry__backdrop';
    const frame = document.createElement('header');
    frame.className = 'sc-substitute-frame';
    document.body.append(graph, backdrop, frame);
    const property = (element: HTMLElement, name: string): string =>
      [...(style.sheet?.cssRules ?? [])]
        .filter((rule): rule is CSSStyleRule => rule instanceof CSSStyleRule)
        .filter((rule) => element.matches(rule.selectorText))
        .reduce((value, rule) => rule.style.getPropertyValue(name) || value, '');
    for (const element of [root, graph, backdrop, frame])
      for (const name of [
        '--sc-workflow-tab-selected-fill',
        '--sc-substitute-toolbar-fill',
        'background',
      ])
        expect(property(element, name)).toBe('');
    root.setAttribute('data-sc-desktop-frame-surface', '');
    expect(property(root, '--sc-workflow-tab-selected-fill')).toBe('var(--sc-desktop-chrome-wash)');
    expect(property(root, '--sc-substitute-toolbar-fill')).toBe('var(--sc-desktop-chrome-wash)');
    expect(property(graph, 'background')).toBe('var(--sc-desktop-body-wash)');
    expect(property(backdrop, 'background')).toContain('--sc-stack-wash-opacity');
    expect(property(frame, 'background')).toBe('');
  } finally {
    root.removeAttribute('data-sc-desktop-frame-surface');
    style.remove();
  }
});
