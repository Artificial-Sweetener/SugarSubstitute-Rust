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
/** Verify provider-owned editor navigation delegates ancestry to Comfy and leaves overrides unchanged. */
import { jest, beforeAll, afterAll, afterEach, test, expect } from '@jest/globals';
import { Updates } from '@microsoft/fast-element';
import { WorkflowEditorNavigationHost } from '../../../frontend/comfyui/ui/surface/WorkflowEditorNavigationHost.js';
import { AppearanceState } from '../../../frontend/comfyui/ui/surface/appearance/AppearanceState.js';
import { publishVisualStyle } from '../../../frontend/comfyui/ui/surface/appearance/VisualStyleBoundary.js';
import type { StyledVisualStyle } from '../../../frontend/comfyui/ui/surface/appearance/VisualStyle.js';
import { createButton } from '../../../frontend/comfyui/ui/surface/appearance/ControlFactory.js';

const descriptor = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'attachInternals');
beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, 'attachInternals', {
    configurable: true,
    value: function (this: HTMLElement) {
      return {
        role: null,
        ariaDisabled: null,
        states: new Set<string>(),
        shadowRoot: this.shadowRoot,
      };
    },
  });
});
afterAll(() => {
  if (descriptor) Object.defineProperty(HTMLElement.prototype, 'attachInternals', descriptor);
  else Reflect.deleteProperty(HTMLElement.prototype, 'attachInternals');
});
afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
  document.documentElement.removeAttribute('data-sc-visual-style');
  jest.restoreAllMocks();
});

/** Deliver native mount observation and Microsoft's custom-element bindings. */
async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) {
    await Updates.next();
    await Promise.resolve();
  }
}

/** Model the native breadcrumb command boundary without replacing navigation policy in the product. */
function fixture(style: StyledVisualStyle = 'fluent') {
  publishVisualStyle(document, style);
  const graphs = [{ id: 'root' }, { id: 'cube' }, { id: 'nested' }];
  const app = { graph: graphs[0], canvas: { graph: graphs[0] } };
  const native = document.createElement('div');
  native.className = 'subgraph-breadcrumb';
  const bar = document.createElement('div');
  bar.className = 'actionbar-container';
  const cluster = document.createElement('div');
  cluster.className = 'sc-orb-actions';
  const pin = createButton(document);
  pin.setAttribute('aria-label', 'Set Global Override');
  cluster.append(pin);
  document.body.append(native, bar, cluster);
  let level = 0;
  const commands = jest.fn<(destination: number) => void>();
  const render = (destination: number): void => {
    level = destination;
    app.canvas.graph = graphs[destination];
    const back = document.createElement('button');
    back.dataset.testid = 'subgraph-breadcrumb-back';
    back.addEventListener('click', () => {
      commands(level - 1);
      render(level - 1);
    });
    const trail = document.createElement('nav');
    for (let index = 0; index <= level; index++) {
      const item = document.createElement('div');
      item.dataset.testid = `subgraph-breadcrumb-item-${index}`;
      const label = document.createElement('span');
      label.className = 'p-breadcrumb-item-label';
      label.textContent = ['Workflow', '<img src=x onerror=evil()>', 'Nested'][index] ?? '';
      item.append(label);
      item.addEventListener('click', (event) => {
        if (event.detail !== 1) return;
        commands(index);
        render(index);
      });
      trail.append(item);
    }
    native.replaceChildren(...(level ? [back, trail] : [trail]));
  };
  render(0);
  const suspendOverrides = jest.fn<(suspended: boolean) => void>();
  const report = jest.fn<(error: unknown) => void>();
  const appearance = new AppearanceState();
  const host = new WorkflowEditorNavigationHost({
    document,
    app,
    appearance,
    suspendOverrides,
    report,
  });
  host.setup();
  return {
    host,
    app,
    render,
    bar,
    native,
    cluster,
    pin,
    suspendOverrides,
    commands,
    report,
    appearance,
  };
}

test.each(['fluent', 'spectrum'] as const)(
  'shows %s back and literal breadcrumbs in place of overrides, then restores the root',
  async (style) => {
    const f = fixture(style);
    f.render(2);
    await settle();
    const nav = f.bar.querySelector('nav')!;
    expect(nav.getAttribute('aria-label')).toBe('Workflow editor navigation');
    expect(
      nav.querySelectorAll(style === 'spectrum' ? 'sp-action-button' : 'fluent-button'),
    ).toHaveLength(3);
    expect(
      nav.querySelectorAll(style === 'spectrum' ? 'fluent-button' : 'sp-action-button'),
    ).toHaveLength(0);
    expect(nav.querySelector('[aria-current="page"]')?.textContent).toBe('Nested');
    expect(nav.querySelector('img')).toBeNull();
    expect(nav.textContent).toContain('<img src=x onerror=evil()>');
    expect(f.pin.hasAttribute('data-sc-editor-native')).toBe(false);
    expect(f.pin.hasAttribute('disabled')).toBe(true);
    expect(f.cluster.children).toHaveLength(1);
    expect(f.suspendOverrides).toHaveBeenLastCalledWith(true);
    const back = f.bar.querySelector<HTMLElement>('.sc-editor-bar-back')!;
    back.click();
    await settle();
    expect(f.commands).toHaveBeenLastCalledWith(1);
    expect(nav.querySelector('[aria-current="page"]')?.textContent).toBe(
      '<img src=x onerror=evil()>',
    );
    back.click();
    await settle();
    expect(f.commands).toHaveBeenLastCalledWith(0);
    expect(f.bar.querySelector('nav')).toBeNull();
    // The hidden trail must not retain the replaced native breadcrumb entries.
    expect(nav.querySelectorAll('li')).toHaveLength(0);
    expect(f.pin.hasAttribute('data-sc-editor-native')).toBe(false);
    expect(f.pin.hasAttribute('disabled')).toBe(false);
    expect(f.suspendOverrides).toHaveBeenLastCalledWith(false);
    expect(f.report).not.toHaveBeenCalled();
    f.host.dispose();
  },
);

test.each(['fluent', 'spectrum'] as const)(
  'ancestor actions in %s delegate to the current native path and disposal restores borrowed controls',
  async (style) => {
    const f = fixture(style);
    f.render(2);
    await settle();
    const intermediate = f.bar.querySelectorAll<HTMLElement>('ol [data-sc-button]')[1]!;
    intermediate.click();
    await settle();
    expect(f.commands).toHaveBeenLastCalledWith(1);
    const root = f.bar.querySelectorAll<HTMLElement>('ol [data-sc-button]')[0]!;
    const nativeTrail = f.native.querySelector('nav')!;
    expect(nativeTrail.hasAttribute('data-sc-editor-native')).toBe(true);
    root.click();
    await settle();
    expect(f.commands).toHaveBeenLastCalledWith(0);
    f.render(2);
    await settle();
    const replacement = f.native.querySelector('nav')!;
    f.host.dispose();
    expect(replacement.hasAttribute('data-sc-editor-native')).toBe(false);
    expect(f.cluster.children).toHaveLength(1);
    expect(f.pin.hasAttribute('disabled')).toBe(false);
    expect(f.pin.hasAttribute('data-sc-editor-native')).toBe(false);
  },
);

test('changes the active provider without changing the native ancestry or sending navigation commands', async () => {
  const f = fixture();
  f.render(2);
  await settle();
  const nav = f.bar.querySelector('nav');
  const nativeTrail = f.native.querySelector('nav');
  for (const style of ['spectrum', 'fluent', 'spectrum'] as const) {
    publishVisualStyle(document, style);
    await settle();
    expect(f.bar.querySelector('nav')).toBe(nav);
    expect(f.native.querySelector('nav')).toBe(nativeTrail);
    expect(
      f.bar.querySelectorAll(style === 'spectrum' ? 'sp-action-button' : 'fluent-button'),
    ).toHaveLength(3);
    expect(nav?.querySelector('[aria-current="page"]')?.textContent).toBe('Nested');
    expect(nav?.querySelector('img')).toBeNull();
    expect(f.commands).not.toHaveBeenCalled();
    expect(f.suspendOverrides).toHaveBeenLastCalledWith(true);
  }
  f.bar.querySelector<HTMLElement>('.sc-editor-bar-back')?.click();
  await settle();
  expect(f.commands).toHaveBeenCalledTimes(1);
  expect(f.commands).toHaveBeenLastCalledWith(1);
  expect(f.report).not.toHaveBeenCalled();
  f.host.dispose();
});

test('unrelated popup activity does not refresh navigation or touch override presentation', async () => {
  const f = fixture();
  f.render(1);
  await settle();
  const nav = f.bar.querySelector('nav');
  const buttons = [...f.bar.querySelectorAll('fluent-button')];
  f.suspendOverrides.mockClear();
  const popup = document.createElement('div');
  popup.setAttribute('role', 'menu');
  document.body.append(popup);
  popup.textContent = 'Unrelated popup';
  await settle();
  expect(f.suspendOverrides).not.toHaveBeenCalled();
  expect(f.bar.querySelector('nav')).toBe(nav);
  expect([...f.bar.querySelectorAll('fluent-button')]).toEqual(buttons);
  f.appearance.update('fluent', false);
  await settle();
  expect([...f.bar.querySelectorAll('fluent-button')]).toEqual(buttons);
  f.host.dispose();
});

test('reports missing native navigation and rejects stale ancestry commands', async () => {
  const f = fixture();
  f.render(1);
  await settle();
  const root = f.bar.querySelectorAll<HTMLElement>('ol fluent-button')[0]!;
  f.native.replaceChildren();
  root.click();
  expect(f.report).toHaveBeenCalledWith(expect.any(Error));
  f.bar.querySelector<HTMLElement>('.sc-editor-bar-back')?.click();
  expect(f.report).toHaveBeenCalledTimes(2);
  await settle();
  expect(f.bar.querySelector('nav')).toBeNull();
  f.host.dispose();
});

test('restores the pin state when it is replaced or navigation is disposed', async () => {
  const f = fixture();
  f.pin.setAttribute('disabled', '');
  f.pin.title = 'Original pin tooltip';
  f.render(1);
  await settle();
  const replacement = document.createElement('fluent-button');
  replacement.setAttribute('aria-label', 'Set Global Override');
  f.pin.replaceWith(replacement);
  await settle();
  expect(f.pin.hasAttribute('disabled')).toBe(true);
  expect(f.pin.title).toBe('Original pin tooltip');
  expect(replacement.hasAttribute('disabled')).toBe(true);
  f.host.dispose();
  expect(replacement.hasAttribute('disabled')).toBe(false);
  expect(f.cluster.firstElementChild).toBe(replacement);
});
