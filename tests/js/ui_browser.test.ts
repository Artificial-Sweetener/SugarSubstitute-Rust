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
import { describe, expect, test, beforeEach, jest } from '@jest/globals';
import { app as hostApp } from '/scripts/app.js';
import { api as hostApi } from '/scripts/api.js';
import type { MockSettingDefinition, MockSidebarTab } from './mocks/app.js';
import type { UnknownRecord } from '../../web/comfyui/ui/types/common.js';
import type { SugarCubesUI } from '../../web/comfyui/ui/SugarCubesUI.js';

let app = hostApp as unknown as (typeof import('./mocks/app.js'))['app'];
let api = hostApi as unknown as (typeof import('./mocks/api.js'))['api'];

interface ApiRequest {
  url: string;
  options: RequestInit;
}

function parseRequestBody(body: BodyInit | null | undefined): UnknownRecord {
  if (typeof body !== 'string') throw new Error('Expected a JSON request body');
  return JSON.parse(body) as UnknownRecord;
}

type TestElement = HTMLElement & {
  value: string;
  disabled: boolean;
  checked: boolean;
  placeholder: string;
  hidden: boolean;
  setSelectionRange(start: number | null, end: number | null): void;
};

function requiredElement(root: Document | Element, selector: string): TestElement {
  const element = root.querySelector<TestElement>(selector);
  if (!element) throw new Error(`Missing test element: ${selector}`);
  return element;
}

let registeredSettings: MockSettingDefinition[] = [];
let loadedUi: SugarCubesUI | null = null;

function setupBaseApp() {
  app.reset();
  app.graph = { _nodes: [], _groups: [] };
  app.canvas = {
    graph: app.graph,
    setDirty: jest.fn(),
    onAfterChange: () => {},
    onDrawForeground: () => {},
    onDrawBackground: () => {},
    processMouseMove: () => {},
    processMouseDown: () => {},
  };
  app.extensionManager = { registerSidebarTab: () => {} };
  app.ui = {
    settings: {
      addSetting: (payload: MockSettingDefinition) => registeredSettings.push(payload),
    },
  };
  app.clean = () => {};
  registeredSettings = [];
}

async function loadUi() {
  const loaded: { value?: typeof import('../../web/comfyui/ui.js') } = {};
  await jest.isolateModulesAsync(async () => {
    loaded.value = await import('../../web/comfyui/ui.js');
  });
  if (!loaded.value) throw new Error('SugarCubes UI module did not load');
  const { sugarCubesExtension, sugarCubesUI } = loaded.value;
  loadedUi = sugarCubesUI;
  const runtimeApp = sugarCubesUI.adapter.getApp() as unknown as typeof app;
  const runtimeApi = sugarCubesUI.adapter.getApi() as unknown as typeof api;
  if (runtimeApp !== app) {
    Object.assign(runtimeApp, app);
    app = runtimeApp;
  }
  if (runtimeApi !== api) {
    Object.assign(runtimeApi, api);
    api = runtimeApi;
  }
  if (!app._extensions.includes(sugarCubesExtension)) {
    app.registerExtension(sugarCubesExtension);
  }
  return sugarCubesExtension;
}

async function setupExtension() {
  await loadUi();
  const extension = app._extensions[0];
  if (!extension?.setup) throw new Error('SugarCubes extension did not register setup');
  await extension.setup();
  await flushPromises();
  return extension;
}

async function mountSidebar() {
  const registerCalls: MockSidebarTab[] = [];
  app.extensionManager.registerSidebarTab = (payload) => registerCalls.push(payload);
  await setupExtension();
  if (!loadedUi) throw new Error('SugarCubes UI module is not loaded');
  await loadedUi.cubeBrowser.refresh({ force: true });
  const container = document.createElement('div');
  const sidebarTab = registerCalls[0];
  if (!sidebarTab) throw new Error('SugarCubes sidebar did not register');
  sidebarTab.render(container);
  document.body.appendChild(container);
  await flushPromises();
  await flushPromises();
  return container;
}

function renderRegisteredSetting(id: string): HTMLElement {
  const setting = getRegisteredSetting(id);
  expect(setting).toBeDefined();
  const element = setting.type();
  document.body.appendChild(element);
  return element;
}

function getRegisteredSetting(id: string): MockSettingDefinition {
  const setting = registeredSettings.find((entry) => entry.id === id);
  if (!setting) throw new Error(`Missing registered setting: ${id}`);
  return setting;
}

async function renderRegisteredSettingAsync(id: string, flushCount = 2): Promise<HTMLElement> {
  const element = renderRegisteredSetting(id);
  for (let index = 0; index < flushCount; index += 1) {
    await flushPromises();
  }
  return element;
}

async function openTrackedPackManagerFromSettings(flushCount = 2): Promise<HTMLElement> {
  const row = await renderRegisteredSettingAsync('SugarCubes.CubePacks.Manager', flushCount);
  const openButton = Array.from(row.querySelectorAll('button')).find((button) =>
    button.textContent.includes('Open Manager'),
  );
  expect(openButton).not.toBeNull();
  if (!openButton) throw new Error('Missing Open Manager button');
  openButton.click();
  await flushPromises();
  await flushPromises();
  return requiredElement(document, '.sugarcubes-pack-manager-dialog');
}

async function flushPromises() {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

async function submitFormModal(fieldValues: Record<string, unknown> = {}) {
  await flushPromises();
  for (const [labelText, value] of Object.entries(fieldValues)) {
    const label = Array.from(document.querySelectorAll('.sugarcubes-form-dialog label')).find(
      (node) => node.textContent.includes(labelText),
    );
    const input = label?.querySelector<HTMLInputElement>('input');
    if (!input) throw new Error(`Missing form input: ${labelText}`);
    if (input.type === 'checkbox') {
      input.checked = Boolean(value);
      input.dispatchEvent(new Event('change', { bubbles: true }));
      continue;
    }
    input.value = String(value);
    input.dispatchEvent(new Event('input', { bubbles: true }));
  }
  const confirmButton = requiredElement(document, '.sugarcubes-form-dialog button:last-child');
  expect(confirmButton).not.toBeNull();
  confirmButton.click();
  await flushPromises();
}

beforeEach(() => {
  jest.resetModules();
  loadedUi = null;
  setupBaseApp();
  localStorage.clear();
  document.body.innerHTML = '';
  globalThis.requestAnimationFrame = () => 1;
  globalThis.cancelAnimationFrame = () => {};
  window.requestAnimationFrame = globalThis.requestAnimationFrame;
  window.cancelAnimationFrame = globalThis.cancelAnimationFrame;
  window.SugarCubes = {} as SugarCubesPublicApi;
  window.alert = () => {};
  window.comfyAPI = { vueApp: { config: { globalProperties: { $toast: null } } } };
  if (!Element.prototype.scrollIntoView) {
    Element.prototype.scrollIntoView = () => {};
  }
  HTMLCanvasElement.prototype.getContext = (() => ({
    clearRect: () => {},
    fillRect: () => {},
    strokeRect: () => {},
    beginPath: () => {},
    moveTo: () => {},
    lineTo: () => {},
    arc: () => {},
    fill: () => {},
    stroke: () => {},
    save: () => {},
    restore: () => {},
    setLineDash: () => {},
    drawImage: () => {},
    fillText: () => {},
  })) as unknown as typeof HTMLCanvasElement.prototype.getContext;
  class TestLGraphCanvas {
    drawConnections(): void {}
    drawForeground(): void {}
  }
  class TestLGraphGroup {}
  TestLGraphCanvas.prototype.drawConnections = () => {};
  TestLGraphCanvas.prototype.drawForeground = () => {};
  globalThis.LiteGraph = {
    LGraphCanvas: TestLGraphCanvas,
    LGraphGroup: TestLGraphGroup,
    LinkDirection: { LEFT: 3, RIGHT: 4 },
  } as unknown as LiteGraphHost;

  api.fetchApi = async () => ({
    ok: true,
    json: async () => ({ cubes: [] }),
  });
  globalThis.fetch = async () => new Response('sdxl\nsd\n', { status: 200 });
});

describe('ui browser behaviors', () => {
  const silenceConsole = () => {
    const spies = [
      jest.spyOn(console, 'log').mockImplementation(() => {}),
      jest.spyOn(console, 'debug').mockImplementation(() => {}),
      jest.spyOn(console, 'info').mockImplementation(() => {}),
      jest.spyOn(console, 'warn').mockImplementation(() => {}),
      jest.spyOn(console, 'error').mockImplementation(() => {}),
    ];
    return () => spies.forEach((spy) => spy.mockRestore());
  };

  test('openLibrary focuses embedded browser', async () => {
    const restore = silenceConsole();
    const container = await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    const browser = container.querySelector('.sugarcubes-browser');
    expect(browser).not.toBeNull();
    expect(document.querySelector('.sugarcubes-browser-overlay')).toBeNull();
    restore();
  });

  test('sidebar omits legacy action buttons and single-tab chrome', async () => {
    const restore = silenceConsole();
    const container = await mountSidebar();

    expect(container.querySelector('.sugarcubes-sidebar-panel__actions')).toBeNull();
    expect(container.querySelector('[data-sugarcubes-save]')).toBeNull();
    expect(container.querySelector('[data-sugarcubes-import]')).toBeNull();
    expect(
      Array.from(container.querySelectorAll('button')).some(
        (button) => button.textContent.trim() === 'Create',
      ),
    ).toBe(false);
    expect(container.querySelector('.sugarcubes-sidebar-panel__tabs')).toBeNull();
    expect(container.querySelector('[data-sugarcubes-tab="library"]')).toBeNull();
    expect(container.querySelector('[data-sugarcubes-panel="library"]')).toBeNull();
    expect(container.querySelector('.sugarcubes-sidebar-panel__library')).not.toBeNull();
    expect(container.querySelector('.sugarcubes-browser')).not.toBeNull();
    restore();
  });

  test('favorites are loaded from storage and stale model filters are ignored', async () => {
    const restore = silenceConsole();
    localStorage.setItem('sugarcubes.favorites', JSON.stringify(['cube-b']));
    localStorage.setItem('sugarcubes.model_filter', JSON.stringify(['sdxl']));

    api.fetchApi = async () => ({
      ok: true,
      json: async () => ({
        cubes: [
          { name: 'Cube A', cube_id: 'cube-a', supported_models: ['sd'], tags: [], mtime: '' },
          { name: 'Cube B', cube_id: 'cube-b', supported_models: ['sdxl'], tags: [], mtime: '' },
        ],
      }),
    });

    await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    const rows = Array.from(
      document.querySelectorAll<HTMLElement>('.sugarcubes-browser__cube-row'),
    );
    expect(rows).toHaveLength(2);
    expect(rows.map((row) => row.dataset.cube).sort()).toEqual(['cube-a', 'cube-b']);
    const favoriteRow = rows.find((row) => row.dataset.cube === 'cube-b');
    if (!favoriteRow) throw new Error('Missing favorite cube row');
    const favoriteIndicator = requiredElement(
      favoriteRow,
      '.sugarcubes-browser__favorite-indicator',
    );
    expect(favoriteIndicator.textContent).toBe('\u2605');
    restore();
  });

  test('browser header only exposes search without model filter controls', async () => {
    const restore = silenceConsole();
    api.fetchApi = async () => ({
      ok: true,
      json: async () => ({
        cubes: [
          {
            name: 'SD Cube',
            cube_id: 'cube-sd',
            supported_models: ['SD 1.5'],
            tags: [],
            mtime: '',
          },
          { name: 'XL Cube', cube_id: 'cube-xl', supported_models: ['SDXL'], tags: [], mtime: '' },
        ],
      }),
    });

    const container = await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();
    await flushPromises();

    expect(container.querySelector('.sugarcubes-browser__filter-button')).toBeNull();
    expect(container.querySelector('.sugarcubes-browser__filter-panel')).toBeNull();
    const searchInput = requiredElement(container, '.sugarcubes-browser__list-header input');
    expect(searchInput).not.toBeNull();
    expect(searchInput.placeholder).toBe('Search cubes...');
    restore();
  });

  test('zebra row styles do not override hover and selected row colors', async () => {
    const restore = silenceConsole();
    await mountSidebar();

    const styleText = document.querySelector('#sugarcubes-browser-styles')?.textContent || '';
    const zebraIndex = styleText.indexOf(
      '.sugarcubes-browser__author-list .sugarcubes-browser__cube-row:nth-child(even)',
    );
    const hoverIndex = styleText.indexOf(
      '.sugarcubes-browser__author-list .sugarcubes-browser__cube-row:hover',
    );
    const selectedIndex = styleText.indexOf(
      '.sugarcubes-browser__author-list .sugarcubes-browser__cube-row.is-selected',
    );

    expect(zebraIndex).toBeGreaterThanOrEqual(0);
    expect(hoverIndex).toBeGreaterThan(zebraIndex);
    expect(selectedIndex).toBeGreaterThan(zebraIndex);
    restore();
  });

  test('browser renders cube icons and literal cube names', async () => {
    const restore = silenceConsole();
    api.fetchApi = async () => ({
      ok: true,
      json: async () => ({
        cubes: [
          {
            name: '<Text Cube>',
            display_name: '<Text Cube>',
            cube_id: 'Artificial-Sweetener/Base-Cubes/Text.cube',
            icon: {
              kind: 'asset',
              url: '/sugarcubes/assets/icon?cube_id=demo',
              media_type: 'image/png',
            },
            supported_models: [],
            tags: [],
            mtime: '',
          },
        ],
      }),
    });

    await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    const row = requiredElement(document, '.sugarcubes-browser__cube-row');
    expect(row).not.toBeNull();
    expect(row.querySelector('.sugarcubes-cube-icon img')?.getAttribute('src')).toBe(
      '/sugarcubes/assets/icon?cube_id=demo',
    );
    expect(requiredElement(row, '.sugarcubes-browser__cube-title-text').textContent).toBe(
      '<Text Cube>',
    );
    expect(row.querySelector('.sugarcubes-browser__cube-title-text img')).toBeNull();
    restore();
  });

  test('recents update when a cube is selected', async () => {
    const restore = silenceConsole();
    api.fetchApi = async () => ({
      ok: true,
      json: async () => ({
        cubes: [{ name: 'Cube A', cube_id: 'cube-a', supported_models: [], tags: [], mtime: '' }],
      }),
    });

    await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    const row = requiredElement(document, '.sugarcubes-browser__cube-row');
    row.dispatchEvent(new MouseEvent('click', { bubbles: true }));

    const stored = JSON.parse(localStorage.getItem('sugarcubes.recent') || '[]');
    expect(stored[0]).toBe('cube-a');
    restore();
  });

  test('browser detail shows catalog metadata first without path or layout rows', async () => {
    const restore = silenceConsole();
    api.fetchApi = async (url) => {
      if (url === '/sugarcubes/list') {
        return {
          ok: true,
          json: async () => ({
            cubes: [
              {
                name: 'text_to_image',
                display_name: 'Text to Image',
                cube_id: 'Artificial-Sweetener/Base-Cubes/text to image.cube',
                author: 'Artificial Sweetener',
                author_url: 'https://example.test/sugarcubes',
                version: '1.2.3',
                supported_models: ['SDXL', 'Flux .1 D'],
                size_bytes: 4096,
                is_writable: false,
                write_block_reason: 'Tracked GitHub repos are read-only.',
                tags: ['base'],
                lineage: {
                  name: 'Original Cube',
                  version: '1.0.0',
                  id: 'local/source/original.cube',
                },
                path: 'E:/ComfyUI/custom_nodes/ComfyUI-SugarCubes/cubes/text_to_image.cube',
                relative_path: 'cubes/text_to_image.cube',
                layout: { present: true, nodes: 2, markers: 1, groups: 1 },
                mtime: '2026-04-06T12:00:00Z',
              },
            ],
          }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    document
      .querySelector('.sugarcubes-browser__cube-row')
      ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await flushPromises();

    const rows = Array.from(document.querySelectorAll('.sugarcubes-browser__meta > div')).map(
      (row) => row.textContent || '',
    );
    expect(rows[0]).toBe('Default Alias: Text to Image');
    expect(rows[1]).toBe('Author: Artificial Sweetener');
    expect(rows[2]).toMatch(/^Version: 1\.2\.3 \| Updated: /);
    expect(rows[3]).toBe('ID: Artificial-Sweetener/Base-Cubes/text to image.cube');
    expect(rows[4]).toBe('Forked from: Original Cube v1.0.0 (local/source/original.cube)');
    expect(rows[5]).toBe('Models: SDXL, Flux .1 D');
    expect(rows[6]).toBe('Website: https://example.test/sugarcubes');
    expect(rows[7]).toBe('Tags: base');
    expect(rows).toHaveLength(8);
    restore();
  });

  test('browser detail labels local cubes as local instead of author', async () => {
    const restore = silenceConsole();
    api.fetchApi = async (url) => {
      if (url === '/sugarcubes/list') {
        return {
          ok: true,
          json: async () => ({
            cubes: [
              {
                name: 'text_to_image',
                display_name: 'Text to Image',
                cube_id: 'local/personal/text_to_image.cube',
                author: 'local',
                source: { type: 'local', namespace: 'personal' },
                version: '1.0.0',
                supported_models: [],
                tags: [],
                mtime: '2026-04-06T12:00:00Z',
              },
            ],
          }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    document
      .querySelector('.sugarcubes-browser__cube-row')
      ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await flushPromises();

    const rows = Array.from(document.querySelectorAll('.sugarcubes-browser__meta > div')).map(
      (row) => row.textContent || '',
    );
    expect(rows[0]).toBe('Default Alias: Text to Image');
    expect(rows[1]).toBe('local');
    expect(rows).not.toContain('Author: local');
    restore();
  });

  test('SugarCubes settings register human-readable categories and labels', async () => {
    const restore = silenceConsole();
    await setupExtension();

    expect(getRegisteredSetting('SugarCubes.CubePacks.TrackedPacks')).toMatchObject({
      category: ['SugarCubes', 'Cube Packs', 'TrackedPacks'],
      name: 'Tracked packs',
    });
    expect(getRegisteredSetting('SugarCubes.CubePacks.Manager')).toMatchObject({
      category: ['SugarCubes', 'Cube Packs', 'Manager'],
      name: 'Manage tracked packs',
    });
    expect(getRegisteredSetting('SugarCubes.Authoring.ClaimedGithubOwner')).toMatchObject({
      category: ['SugarCubes', 'Authoring', 'ClaimedGithubOwner'],
      name: 'Claimed GitHub owner',
    });
    expect(getRegisteredSetting('SugarCubes.Graph.ProximityLinks')).toMatchObject({
      category: ['SugarCubes', 'Graph', 'ProximityLinks'],
      name: 'Enable proximity links',
    });
    expect(registeredSettings.some((entry) => String(entry.name).startsWith('SugarCubes:'))).toBe(
      false,
    );
    restore();
  });

  test('tracked packs settings row renders a summary without embedding manager controls', async () => {
    const restore = silenceConsole();
    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/list') {
        return { ok: true, json: async () => ({ cubes: [] }) };
      }
      if (url === '/sugarcubes/repos' && (!options.method || options.method === 'GET')) {
        return {
          ok: true,
          json: async () => ({
            repos: [
              {
                owner: 'Artificial-Sweetener',
                repo: 'Base-Cubes',
                branch: 'main',
                enabled: true,
                default_base_repo: true,
                auto_update: true,
                last_sync_status: 'success',
                last_sync_at: '2026-04-06T12:00:00Z',
                last_sync_error: '',
                last_checked_at: '',
                last_check_status: 'never',
                last_check_error: '',
                update_available: false,
                is_writable: true,
              },
            ],
          }),
        };
      }
      if (url === '/sugarcubes/packs/check_all') {
        return {
          ok: true,
          json: async () => ({
            repos: [
              {
                owner: 'Artificial-Sweetener',
                repo: 'Base-Cubes',
                branch: 'main',
                enabled: true,
                default_base_repo: true,
                auto_update: true,
                last_sync_status: 'ok',
                last_sync_at: '2026-04-06T12:00:00Z',
                last_sync_error: '',
                last_checked_at: '2026-04-06T12:01:00Z',
                last_check_status: 'ok',
                last_check_error: '',
                update_available: true,
                is_writable: true,
              },
            ],
          }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    await setupExtension();
    await flushPromises();
    const row = await renderRegisteredSettingAsync('SugarCubes.CubePacks.TrackedPacks', 3);

    expect(row.textContent).toContain('1 tracked pack');
    expect(row.textContent).toContain('1 writable');
    expect(row.textContent).toContain('No updates available');
    expect(row.textContent).not.toContain('Artificial-Sweetener/Base-Cubes');
    expect(row.textContent).not.toContain('Add Pack');
    expect(row.textContent).not.toContain('Edit source');
    expect(row.textContent).not.toContain('SugarCubes:');
    restore();
  });

  test('tracked pack manager row opens manager dialog with pack details', async () => {
    const restore = silenceConsole();
    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/list') {
        return { ok: true, json: async () => ({ cubes: [] }) };
      }
      if (url === '/sugarcubes/repos' && (!options.method || options.method === 'GET')) {
        return {
          ok: true,
          json: async () => ({
            repos: [
              {
                owner: 'Artificial-Sweetener',
                repo: 'Base-Cubes',
                branch: 'main',
                enabled: true,
                default_base_repo: true,
                auto_update: false,
                last_sync_status: 'success',
                last_sync_at: '',
                last_sync_error: '',
                last_checked_at: '',
                last_check_status: 'never',
                last_check_error: '',
                update_available: false,
              },
            ],
          }),
        };
      }
      if (url === '/sugarcubes/packs/check_all') {
        return {
          ok: true,
          json: async () => ({
            repos: [
              {
                owner: 'Artificial-Sweetener',
                repo: 'Base-Cubes',
                branch: 'main',
                enabled: true,
                default_base_repo: true,
                auto_update: false,
                last_sync_status: 'ok',
                last_sync_at: '',
                last_sync_error: '',
                last_checked_at: '2026-04-06T12:01:00Z',
                last_check_status: 'ok',
                last_check_error: '',
                update_available: true,
              },
            ],
          }),
        };
      }
      if (url === '/sugarcubes/repos/sync_all') {
        return { ok: true, json: async () => ({ repos: [] }) };
      }
      return { ok: true, json: async () => ({}) };
    };

    await setupExtension();
    await flushPromises();
    await renderRegisteredSettingAsync('SugarCubes.CubePacks.TrackedPacks', 3);
    const dialog = await openTrackedPackManagerFromSettings(3);

    expect(dialog?.textContent).toContain('Manage tracked packs');
    expect(dialog?.textContent).toContain('Artificial-Sweetener/Base-Cubes');
    expect(dialog?.textContent).not.toContain('Branch:');
    expect(dialog?.textContent).not.toContain('Edit source');
    restore();
  });

  test('tracked pack manager refreshes packs created after initial setup', async () => {
    const restore = silenceConsole();
    let repoReadCount = 0;
    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/list') {
        return { ok: true, json: async () => ({ cubes: [] }) };
      }
      if (url === '/sugarcubes/repos' && (!options.method || options.method === 'GET')) {
        repoReadCount += 1;
        const repos = [
          {
            owner: 'Artificial-Sweetener',
            repo: 'Base-Cubes',
            enabled: true,
            default_base_repo: true,
            is_writable: true,
          },
        ];
        if (repoReadCount > 1) {
          repos.push({
            owner: 'Artificial-Sweetener',
            repo: 'New-Pack',
            enabled: true,
            default_base_repo: false,
            is_writable: true,
          });
        }
        return { ok: true, json: async () => ({ repos }) };
      }
      return { ok: true, json: async () => ({ repos: [] }) };
    };

    await setupExtension();
    await flushPromises();
    const dialog = await openTrackedPackManagerFromSettings(4);

    expect(repoReadCount).toBeGreaterThan(1);
    expect(dialog?.textContent).toContain('Artificial-Sweetener/New-Pack');
    restore();
  });

  test('sidebar shows the picker only and moves management into settings', async () => {
    const restore = silenceConsole();
    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/list') {
        return { ok: true, json: async () => ({ cubes: [] }) };
      }
      if (url === '/sugarcubes/repos' && (!options.method || options.method === 'GET')) {
        return {
          ok: true,
          json: async () => ({
            repos: [],
            identity_policy: {
              claimed_github_owner: 'Artificial-Sweetener',
              allow_system_owner_claim: true,
              has_claimed_github_owner: true,
              claimed_github_owner_source: 'file',
              allow_system_owner_claim_source: 'file',
              env_override_active: false,
            },
          }),
        };
      }
      return { ok: true, json: async () => ({ repos: [], identity_policy: {} }) };
    };

    const container = await mountSidebar();
    await flushPromises();

    expect(container.textContent).not.toContain('Cube Packs');
    expect(container.textContent).not.toContain('Add Pack');
    expect(container.textContent).not.toContain('Base Pack');
    expect(container.textContent).not.toContain('Authoring Access');
    expect(container.querySelector('[data-sugarcubes-proximity]')).toBeNull();
    restore();
  });

  test('env-managed authoring access disables owner controls and explains env-only system gate', async () => {
    const restore = silenceConsole();
    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/list') {
        return { ok: true, json: async () => ({ cubes: [] }) };
      }
      if (url === '/sugarcubes/repos' && (!options.method || options.method === 'GET')) {
        return {
          ok: true,
          json: async () => ({
            repos: [
              {
                owner: 'Artificial-Sweetener',
                repo: 'Base-Cubes',
                branch: 'main',
                enabled: true,
                default_base_repo: true,
                auto_update: true,
                last_sync_status: 'ok',
                last_sync_at: '',
                last_sync_error: '',
                last_checked_at: '',
                last_check_status: 'never',
                last_check_error: '',
                update_available: false,
                is_writable: true,
                ownership_mode: 'mine',
              },
            ],
            identity_policy: {
              claimed_github_owner: 'Artificial-Sweetener',
              allow_system_owner_claim: true,
              has_claimed_github_owner: true,
              claimed_github_owner_source: 'dotenv',
              allow_system_owner_claim_source: 'dotenv',
              env_override_active: true,
            },
          }),
        };
      }
      if (url === '/sugarcubes/packs/check_all') {
        return {
          ok: true,
          json: async () => ({
            repos: [
              {
                owner: 'Artificial-Sweetener',
                repo: 'Base-Cubes',
                branch: 'main',
                enabled: true,
                default_base_repo: true,
                auto_update: true,
                last_sync_status: 'ok',
                last_sync_at: '',
                last_sync_error: '',
                last_checked_at: '2026-04-06T12:01:00Z',
                last_check_status: 'ok',
                last_check_error: '',
                update_available: false,
                is_writable: true,
                ownership_mode: 'mine',
              },
            ],
            identity_policy: {
              claimed_github_owner: 'Artificial-Sweetener',
              allow_system_owner_claim: true,
              has_claimed_github_owner: true,
              claimed_github_owner_source: 'dotenv',
              allow_system_owner_claim_source: 'dotenv',
              env_override_active: true,
            },
          }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    await setupExtension();
    await flushPromises();
    const ownerRow = await renderRegisteredSettingAsync('SugarCubes.Authoring.ClaimedGithubOwner');

    expect(ownerRow.textContent).toContain(
      'Managed by environment configuration (.env or process environment).',
    );
    const changeClaimButton = Array.from(ownerRow.querySelectorAll('button')).find((button) =>
      button.textContent.includes('Change'),
    );
    const clearClaimButton = Array.from(ownerRow.querySelectorAll('button')).find((button) =>
      button.textContent.includes('Clear'),
    );

    expect(changeClaimButton?.disabled).toBe(true);
    expect(clearClaimButton?.disabled).toBe(true);
    restore();
  });

  test('file-managed authoring access keeps owner controls editable', async () => {
    const restore = silenceConsole();
    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/list') {
        return { ok: true, json: async () => ({ cubes: [] }) };
      }
      if (url === '/sugarcubes/repos' && (!options.method || options.method === 'GET')) {
        return {
          ok: true,
          json: async () => ({
            repos: [],
            identity_policy: {
              claimed_github_owner: 'ExampleUser',
              allow_system_owner_claim: false,
              has_claimed_github_owner: true,
              claimed_github_owner_source: 'file',
              allow_system_owner_claim_source: 'default',
              env_override_active: false,
            },
          }),
        };
      }
      if (url === '/sugarcubes/packs/check_all') {
        return {
          ok: true,
          json: async () => ({
            repos: [],
            identity_policy: {
              claimed_github_owner: 'ExampleUser',
              allow_system_owner_claim: false,
              has_claimed_github_owner: true,
              claimed_github_owner_source: 'file',
              allow_system_owner_claim_source: 'default',
              env_override_active: false,
            },
          }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    await setupExtension();
    await flushPromises();
    const ownerRow = await renderRegisteredSettingAsync('SugarCubes.Authoring.ClaimedGithubOwner');

    const changeClaimButton = Array.from(ownerRow.querySelectorAll('button')).find((button) =>
      button.textContent.includes('Change'),
    );
    const clearClaimButton = Array.from(ownerRow.querySelectorAll('button')).find((button) =>
      button.textContent.includes('Clear'),
    );

    expect(ownerRow.textContent).not.toContain('Managed by environment configuration');
    expect(ownerRow.textContent).toContain(
      'Artificial-Sweetener can only be claimed when SUGARCUBES_ALLOW_SYSTEM_OWNER_CLAIM is enabled in .env or the process environment.',
    );
    expect(changeClaimButton?.disabled).toBe(false);
    expect(clearClaimButton?.disabled).toBe(false);
    restore();
  });

  test('cube packs settings row adds a pack with normalized values', async () => {
    const restore = silenceConsole();
    const infoSpy = jest.spyOn(console, 'info').mockImplementation(() => {});
    const calls: ApiRequest[] = [];
    let repos: UnknownRecord[] = [];
    api.fetchApi = async (url, options = {}) => {
      calls.push({ url, options });
      if (url === '/sugarcubes/list') {
        return { ok: true, json: async () => ({ cubes: [] }) };
      }
      if (url === '/sugarcubes/repos' && (!options.method || options.method === 'GET')) {
        return { ok: true, json: async () => ({ repos }) };
      }
      if (url === '/sugarcubes/repos/preflight' && options.method === 'POST') {
        return {
          ok: true,
          json: async () => ({
            preflight: {
              contains_cubes: true,
              cube_count: 2,
              cube_paths: ['demo.cube', 'nested/alpha.cube'],
            },
          }),
        };
      }
      if (url === '/sugarcubes/repos' && options.method === 'POST') {
        repos = [
          {
            owner: 'Artificial-Sweetener',
            repo: 'Base-Cubes',
            branch: 'main',
            enabled: true,
            default_base_repo: true,
            auto_update: false,
            last_sync_status: 'never',
            last_sync_at: '',
            last_sync_error: '',
            last_checked_at: '',
            last_check_status: 'never',
            last_check_error: '',
            update_available: false,
          },
        ];
        return {
          ok: true,
          json: async () => ({ ok: true, preflight: { contains_cubes: true, cube_count: 2 } }),
        };
      }
      if (url === '/sugarcubes/packs/check_all') {
        return { ok: true, json: async () => ({ repos }) };
      }
      return { ok: true, json: async () => ({}) };
    };

    await setupExtension();
    const managerDialog = await openTrackedPackManagerFromSettings(3);
    const addButton = Array.from(managerDialog.querySelectorAll('button')).find(
      (button) => button.textContent === 'Add Pack',
    );
    if (!addButton) throw new Error('Missing Add Pack button');
    addButton.click();
    await submitFormModal({
      'Source repository': ' Artificial-Sweetener/Base-Cubes ',
    });
    await flushPromises();

    const addCall = calls.find(
      (entry) => entry.url === '/sugarcubes/repos' && entry.options.method === 'POST',
    );
    const preflightCall = calls.find(
      (entry) => entry.url === '/sugarcubes/repos/preflight' && entry.options.method === 'POST',
    );
    expect(preflightCall).toBeTruthy();
    expect(addCall).toBeTruthy();
    if (!preflightCall || !addCall) throw new Error('Missing tracked-pack requests');
    expect(calls.indexOf(preflightCall)).toBeLessThan(calls.indexOf(addCall));
    expect(parseRequestBody(preflightCall.options.body)).toEqual({
      owner: 'Artificial-Sweetener',
      repo: 'Base-Cubes',
      enabled: true,
      auto_update: false,
    });
    expect(parseRequestBody(addCall.options.body)).toEqual({
      owner: 'Artificial-Sweetener',
      repo: 'Base-Cubes',
      enabled: true,
      auto_update: false,
    });
    expect(infoSpy).toHaveBeenCalledWith(
      expect.stringContaining('Artificial-Sweetener/Base-Cubes is now tracked. Found 2 cubes.'),
    );
    expect(managerDialog.textContent).toContain('Artificial-Sweetener/Base-Cubes');
    expect(managerDialog.textContent).not.toContain('Branch:');
    infoSpy.mockRestore();
    restore();
  });

  test('cube packs settings row does not add a pack when preflight fails', async () => {
    const restore = silenceConsole();
    const errorSpy = jest.spyOn(console, 'error').mockImplementation(() => {});
    const calls: ApiRequest[] = [];
    api.fetchApi = async (url, options = {}) => {
      calls.push({ url, options });
      if (url === '/sugarcubes/list') {
        return { ok: true, json: async () => ({ cubes: [] }) };
      }
      if (url === '/sugarcubes/repos' && (!options.method || options.method === 'GET')) {
        return { ok: true, json: async () => ({ repos: [] }) };
      }
      if (url === '/sugarcubes/repos/preflight' && options.method === 'POST') {
        return {
          ok: false,
          statusText: 'Unprocessable Entity',
          json: async () => ({
            error: {
              message:
                "Repository 'Artificial-Sweetener/Empty-Pack' does not contain any .cube files on branch 'main'.",
            },
          }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    await setupExtension();
    const managerDialog = await openTrackedPackManagerFromSettings(3);
    const addButton = Array.from(managerDialog.querySelectorAll('button')).find(
      (button) => button.textContent === 'Add Pack',
    );
    if (!addButton) throw new Error('Missing Add Pack button');
    addButton.click();
    await submitFormModal({
      'Source repository': ' Artificial-Sweetener/Empty-Pack ',
    });
    await flushPromises();

    expect(
      calls.some((entry) => entry.url === '/sugarcubes/repos' && entry.options.method === 'POST'),
    ).toBe(false);
    expect(errorSpy).toHaveBeenCalledWith(
      expect.stringContaining('does not contain any .cube files'),
    );
    errorSpy.mockRestore();
    restore();
  });

  test('tracked pack manager no longer exposes branch editing controls', async () => {
    const restore = silenceConsole();
    const repos = [
      {
        owner: 'Artificial-Sweetener',
        repo: 'Base-Cubes',
        branch: 'main',
        enabled: true,
        default_base_repo: false,
        auto_update: false,
        last_sync_status: 'never',
        last_sync_at: '',
        last_sync_error: '',
        last_checked_at: '',
        last_check_status: 'never',
        last_check_error: '',
        update_available: false,
      },
    ];
    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/list') {
        return { ok: true, json: async () => ({ cubes: [] }) };
      }
      if (url === '/sugarcubes/repos' && (!options.method || options.method === 'GET')) {
        return { ok: true, json: async () => ({ repos }) };
      }
      if (url === '/sugarcubes/packs/check_all') {
        return { ok: true, json: async () => ({ repos }) };
      }
      return { ok: true, json: async () => ({}) };
    };

    await setupExtension();
    const managerDialog = await openTrackedPackManagerFromSettings(3);
    expect(managerDialog.textContent).not.toContain('Edit source');
    expect(managerDialog.textContent).not.toContain('Branch:');
    restore();
  });

  test('cube packs settings row persists auto-update toggle through backend update', async () => {
    const restore = silenceConsole();
    const calls: ApiRequest[] = [];
    api.fetchApi = async (url, options = {}) => {
      calls.push({ url, options });
      if (url === '/sugarcubes/list') {
        return { ok: true, json: async () => ({ cubes: [] }) };
      }
      if (url === '/sugarcubes/repos') {
        return {
          ok: true,
          json: async () => ({
            repos: [
              {
                owner: 'Artificial-Sweetener',
                repo: 'Base-Cubes',
                branch: 'main',
                enabled: true,
                default_base_repo: true,
                auto_update: false,
                last_sync_status: 'ok',
                last_sync_at: '',
                last_sync_error: '',
                last_checked_at: '',
                last_check_status: 'never',
                last_check_error: '',
                update_available: false,
              },
            ],
          }),
        };
      }
      if (url === '/sugarcubes/packs/check_all') {
        return {
          ok: true,
          json: async () => ({
            repos: [
              {
                owner: 'Artificial-Sweetener',
                repo: 'Base-Cubes',
                branch: 'main',
                enabled: true,
                default_base_repo: true,
                auto_update: false,
                last_sync_status: 'ok',
                last_sync_at: '',
                last_sync_error: '',
                last_checked_at: '2026-04-06T12:01:00Z',
                last_check_status: 'ok',
                last_check_error: '',
                update_available: false,
              },
            ],
          }),
        };
      }
      if (url === '/sugarcubes/repos' && options.method === 'PATCH') {
        return {
          ok: true,
          json: async () => ({
            repo: {
              owner: 'Artificial-Sweetener',
              repo: 'Base-Cubes',
              branch: 'main',
              enabled: true,
              default_base_repo: true,
              auto_update: true,
            },
          }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    await setupExtension();
    await flushPromises();
    const managerDialog = await openTrackedPackManagerFromSettings(3);

    const toggleLabels = Array.from(managerDialog.querySelectorAll('label'));
    const autoUpdateLabel = toggleLabels.find((label) => label.textContent.includes('Auto-update'));
    if (!autoUpdateLabel) throw new Error('Missing auto-update setting');
    const toggle = requiredElement(autoUpdateLabel, 'input[type="checkbox"]');
    toggle.checked = true;
    toggle.dispatchEvent(new Event('change', { bubbles: true }));
    await flushPromises();
    await flushPromises();

    const patchCall = calls.find(
      (entry) => entry.url === '/sugarcubes/repos' && entry.options.method === 'PATCH',
    );
    expect(patchCall).toBeTruthy();
    if (!patchCall) throw new Error('Missing tracked-pack update');
    expect(parseRequestBody(patchCall.options.body)).toMatchObject({
      owner: 'Artificial-Sweetener',
      repo: 'Base-Cubes',
      auto_update: true,
    });
    restore();
  });

  test('browser version combobox suggests versions and places selected revision', async () => {
    const restore = silenceConsole();
    const calls: ApiRequest[] = [];
    api.fetchApi = async (url, options = {}) => {
      calls.push({ url, options });
      if (url === '/sugarcubes/list') {
        return {
          ok: true,
          json: async () => ({
            cubes: [
              {
                name: 'text_to_image',
                display_name: 'text to image',
                cube_id: 'Artificial-Sweetener/Base-Cubes/text to image.cube',
                version: '1.2.0',
                supported_models: [],
                tags: [],
                mtime: '2026-04-06T12:00:00Z',
              },
            ],
          }),
        };
      }
      if (url.startsWith('/sugarcubes/revisions?cube_id=')) {
        return {
          ok: true,
          json: async () => ({
            revisions: [
              {
                revision_ref: 'WORKTREE',
                version: '1.2.0',
                current: true,
              },
              {
                revision_ref: 'mid123456789',
                version: '1.1.0',
                current: false,
              },
              {
                revision_ref: 'abc123456789',
                version: '1.0.0',
                current: false,
              },
              {
                revision_ref: 'def123456789',
                version: '1.0.0',
                current: false,
              },
            ],
          }),
        };
      }
      if (url === '/sugarcubes/load') {
        return {
          ok: true,
          json: async () => ({
            cube: {
              cube_id: 'Artificial-Sweetener/Base-Cubes/text to image.cube',
              version: '1.2.0',
            },
            layout: { origin: [0, 0] },
            nodes: [],
            markers: [],
          }),
        };
      }
      if (url === '/sugarcubes/load_revision') {
        return {
          ok: true,
          json: async () => ({
            cube: {
              cube_id: 'Artificial-Sweetener/Base-Cubes/text to image.cube',
              version: '1.0.0',
            },
            revision: { revision_ref: 'abc123456789', current: false },
            layout: { origin: [0, 0] },
            nodes: [],
            markers: [],
          }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    const canvasElement = document.createElement('canvas');
    canvasElement.getBoundingClientRect = () => new DOMRect(0, 0, 300, 200);
    app.canvas.canvas = canvasElement;

    await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    document
      .querySelector('.sugarcubes-browser__cube-row')
      ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await flushPromises();
    await flushPromises();
    await flushPromises();

    const versionInput = requiredElement(document, '.sugarcubes-browser__version-input');
    const versionToggle = requiredElement(document, '.sugarcubes-browser__version-toggle');
    const versionListbox = requiredElement(document, '.sugarcubes-browser__version-listbox');
    expect(versionInput).not.toBeNull();
    expect(versionInput.disabled).toBe(false);
    expect(versionInput.getAttribute('role')).toBe('combobox');
    const titleGroup = document.querySelector('.sugarcubes-browser__detail-title');
    const actionGroup = document.querySelector('.sugarcubes-browser__detail-actions');
    expect(titleGroup?.contains(versionInput)).toBe(true);
    expect(actionGroup?.contains(versionInput)).toBe(false);
    const versionControl = versionInput.closest('.sugarcubes-browser__version-control');
    expect(versionControl?.previousElementSibling?.tagName).toBe('H3');
    expect(versionControl?.querySelector('.sugarcubes-browser__version-prefix')?.textContent).toBe(
      'v',
    );
    expect(versionInput.value).toBe('1.2.0');
    const versionOptionTexts = () =>
      Array.from(versionListbox.querySelectorAll('.sugarcubes-browser__version-option')).map(
        (entry) => entry.textContent,
      );

    versionToggle.click();
    await flushPromises();
    expect(versionInput.getAttribute('aria-expanded')).toBe('true');
    expect(versionListbox.hidden).toBe(false);
    expect(versionOptionTexts()).toEqual(['1.2.0', '1.1.0', '1.0.0']);

    versionToggle.click();
    await flushPromises();
    expect(versionInput.getAttribute('aria-expanded')).toBe('false');
    expect(versionListbox.hidden).toBe(true);

    versionInput.click();
    await flushPromises();
    expect(versionInput.getAttribute('aria-expanded')).toBe('true');
    expect(versionListbox.hidden).toBe(false);
    expect(versionOptionTexts()).toEqual(['1.2.0', '1.1.0', '1.0.0']);

    versionInput.value = '1.1';
    versionInput.dispatchEvent(new Event('input', { bubbles: true }));
    expect(versionOptionTexts()).toEqual(['1.1.0']);
    expect(
      versionListbox.querySelector('.sugarcubes-browser__version-option.is-highlighted')
        ?.textContent,
    ).toBe('1.1.0');

    versionInput.value = 'v1.0.0';
    versionInput.dispatchEvent(new Event('input', { bubbles: true }));
    expect(versionOptionTexts()).toEqual(['1.0.0']);
    versionInput.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await flushPromises();
    await flushPromises();

    expect(versionInput.value).toBe('1.0.0');
    expect(versionInput.getAttribute('aria-expanded')).toBe('false');
    expect(calls.some((entry) => entry.url.startsWith('/sugarcubes/revisions?cube_id='))).toBe(
      true,
    );
    expect(calls.some((entry) => entry.url === '/sugarcubes/load_revision')).toBe(true);

    const previousRevisionLoads = calls.filter(
      (entry) => entry.url === '/sugarcubes/load_revision',
    ).length;
    requiredElement(document, '.sugarcubes-browser__place').click();
    await flushPromises();
    const revisionLoads = calls.filter((entry) => entry.url === '/sugarcubes/load_revision');
    expect(revisionLoads).toHaveLength(previousRevisionLoads + 1);
    const latestRevisionLoad = revisionLoads.at(-1);
    if (!latestRevisionLoad) throw new Error('Missing revision load request');
    expect(parseRequestBody(latestRevisionLoad.options.body)).toMatchObject({
      cube_id: 'Artificial-Sweetener/Base-Cubes/text to image.cube',
      revision_ref: 'abc123456789',
    });

    versionInput.click();
    versionInput.value = '1.2.9';
    versionInput.dispatchEvent(new Event('input', { bubbles: true }));
    expect(versionOptionTexts()).toEqual(['1.2.0']);
    versionInput.dispatchEvent(new Event('blur'));
    await flushPromises();
    await flushPromises();
    expect(versionInput.value).toBe('1.2.0');
    restore();
  });

  test('browser version combobox stays enabled when backend reports duplicate history', async () => {
    const restore = silenceConsole();
    api.fetchApi = async (url) => {
      if (url === '/sugarcubes/list') {
        return {
          ok: true,
          json: async () => ({
            cubes: [
              {
                name: 'text_to_image',
                display_name: 'text to image',
                cube_id: 'Artificial-Sweetener/Base-Cubes/text to image.cube',
                version: '1.1.1',
                supported_models: [],
                tags: [],
                mtime: '2026-04-06T12:00:00Z',
              },
            ],
          }),
        };
      }
      if (url.startsWith('/sugarcubes/revisions?cube_id=')) {
        return {
          ok: false,
          statusText: 'Conflict',
          json: async () => ({
            error: {
              message: 'Cube history contains duplicate version entries',
              details: {
                duplicates: [
                  {
                    version: '1.1.0',
                    first_revision_ref: 'abc123456789',
                    duplicate_revision_ref: 'def123456789',
                  },
                ],
              },
            },
          }),
        };
      }
      if (url === '/sugarcubes/load') {
        return {
          ok: true,
          json: async () => ({
            cube: {
              cube_id: 'Artificial-Sweetener/Base-Cubes/text to image.cube',
              version: '1.1.1',
            },
            layout: { origin: [0, 0] },
            nodes: [],
            markers: [],
          }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    document
      .querySelector('.sugarcubes-browser__cube-row')
      ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await flushPromises();
    await flushPromises();
    await flushPromises();

    const versionInput = requiredElement(document, '.sugarcubes-browser__version-input');
    const versionControl = versionInput?.closest('.sugarcubes-browser__version-control');
    const versionListbox = requiredElement(document, '.sugarcubes-browser__version-listbox');
    expect(versionInput).not.toBeNull();
    expect(versionInput.disabled).toBe(false);
    expect(versionInput.title).toBe('Spawn version');
    expect(versionControl?.classList.contains('is-error')).toBe(false);
    versionInput.click();
    await flushPromises();
    expect(
      Array.from(versionListbox.querySelectorAll('.sugarcubes-browser__version-option')).map(
        (entry) => entry.textContent,
      ),
    ).toEqual(['1.1.1']);
    restore();
  });

  test('proximity setting row updates enabled state', async () => {
    const restore = silenceConsole();
    await setupExtension();

    const row = await renderRegisteredSettingAsync('SugarCubes.Graph.ProximityLinks');
    const toggle = requiredElement(row, 'input[type="checkbox"]');
    expect(toggle.checked).toBe(true);
    expect(row.textContent).toContain('connect automatically during queueing');

    toggle.checked = false;
    toggle.dispatchEvent(new Event('change', { bubbles: true }));

    expect(toggle.checked).toBe(false);
    expect(row.textContent).toContain(
      'Queueing will not auto-connect nearby compatible cube markers',
    );
    restore();
  });

  test('listCubes rejects on fetch failure', async () => {
    const restore = silenceConsole();
    api.fetchApi = async () => {
      throw new Error('network');
    };
    await loadUi();
    await expect(window.SugarCubes.listCubes()).rejects.toThrow('network');
    restore();
  });

  test('previewCube encodes special characters', async () => {
    const restore = silenceConsole();
    const calls: string[] = [];
    api.fetchApi = async (url) => {
      calls.push(url);
      return { ok: true, json: async () => ({ ok: true }) };
    };
    await loadUi();

    await window.SugarCubes.previewCube('cube/id/\u6d4b\u8bd5');
    expect(calls[0]).toBe('/sugarcubes/preview?cube_id=cube%2Fid%2F%E6%B5%8B%E8%AF%95');
    restore();
  });

  test('place button triggers load request after selection', async () => {
    const restore = silenceConsole();
    const calls: string[] = [];
    api.fetchApi = async (url) => {
      calls.push(url);
      if (url === '/sugarcubes/list') {
        return {
          ok: true,
          json: async () => ({
            cubes: [
              {
                name: 'Cube A',
                cube_id: 'cube-a',
                version: '1.0.0',
                supported_models: [],
                tags: [],
                mtime: '',
              },
            ],
          }),
        };
      }
      if (url === '/sugarcubes/load') {
        return {
          ok: true,
          json: async () => ({ layout: { origin: [0, 0] }, nodes: [], markers: [] }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    const canvasElement = document.createElement('canvas');
    canvasElement.getBoundingClientRect = () => new DOMRect(0, 0, 300, 200);
    app.canvas.canvas = canvasElement;

    await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    const row = requiredElement(document, '.sugarcubes-browser__cube-row');
    row.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await flushPromises();
    await flushPromises();

    const versionInput = requiredElement(document, '.sugarcubes-browser__version-input');
    expect(versionInput).not.toBeNull();
    expect(versionInput.disabled).toBe(false);

    const placeButton = requiredElement(document, '.sugarcubes-browser__place');
    placeButton.click();
    await flushPromises();

    expect(calls).toContain('/sugarcubes/load');
    restore();
  });

  test('list renders cube names', async () => {
    const restore = silenceConsole();
    api.fetchApi = async () => ({
      ok: true,
      json: async () => ({
        cubes: [
          {
            name: 'Cube A',
            cube_id: 'cube-a',
            description: 'Test cube',
            tags: ['foo', 'bar'],
            supported_models: [],
            mtime: '2024-01-01T00:00:00Z',
          },
        ],
      }),
    });

    await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    const row = requiredElement(document, '.sugarcubes-browser__cube-row');
    const title = row?.querySelector('.sugarcubes-browser__cube-title')?.textContent || '';
    expect(title).toContain('Cube A');
    expect(row?.dataset.cube).toBe('cube-a');
    restore();
  });

  test('list renders pack name before lighter author name', async () => {
    const restore = silenceConsole();
    api.fetchApi = async () => ({
      ok: true,
      json: async () => ({
        cubes: [
          {
            name: 'Text to Image',
            cube_id: 'Artificial-Sweetener/Base-Cubes/text to image.cube',
            owner: 'Artificial-Sweetener',
            repo: 'Base-Cubes',
            source: {
              type: 'github',
              owner: 'Artificial-Sweetener',
              repo: 'Base-Cubes',
              repo_ref: 'Artificial-Sweetener/Base-Cubes',
            },
            supported_models: [],
            tags: [],
            mtime: '2024-01-01T00:00:00Z',
          },
          {
            name: 'Diffusion Upscale',
            cube_id: 'Artificial-Sweetener/Base-Cubes/diffusion upscale.cube',
            owner: 'Artificial-Sweetener',
            repo: 'Base-Cubes',
            source: {
              type: 'github',
              owner: 'Artificial-Sweetener',
              repo: 'Base-Cubes',
              repo_ref: 'Artificial-Sweetener/Base-Cubes',
            },
            supported_models: [],
            tags: [],
            mtime: '2024-01-02T00:00:00Z',
          },
        ],
      }),
    });

    await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    const packName = document.querySelector('.sugarcubes-browser__pack-name');
    const packAuthor = document.querySelector('.sugarcubes-browser__pack-author');
    const packLabel = document.querySelector('.sugarcubes-browser__pack-label');
    expect(packName?.textContent).toBe('Base-Cubes');
    expect(packAuthor?.textContent).toBe('Artificial-Sweetener');
    expect(Array.from(packLabel?.children || [])).toEqual([packName, packAuthor]);
    const packBody = document.querySelector('.sugarcubes-browser__author-list');
    const cubeRows = Array.from(packBody?.querySelectorAll('.sugarcubes-browser__cube-row') || []);
    expect(cubeRows).toHaveLength(2);
    expect(cubeRows.map((row) => row.textContent || '')).toEqual([
      'Diffusion Upscale',
      'Text to Image',
    ]);
    expect(document.querySelector('.sugarcubes-browser__author-label')).toBeNull();
    restore();
  });

  test('browser renders markup-like cube text literally in list and details', async () => {
    const restore = silenceConsole();
    api.fetchApi = async (url) => {
      if (url === '/sugarcubes/list') {
        return {
          ok: true,
          json: async () => ({
            cubes: [
              {
                name: '<img src=x onerror=1>',
                cube_id: 'Artificial-Sweetener/Base-Cubes/cube_a.cube',
                description: '<b>Bold</b>',
                author: '<script>alert(1)</script>',
                tags: ['<svg onload=1>'],
                supported_models: ['sdxl'],
                mtime: '2024-01-01T00:00:00Z',
              },
            ],
          }),
        };
      }
      if (url === '/sugarcubes/load') {
        return {
          ok: true,
          json: async () => ({ layout: { origin: [0, 0] }, nodes: [], markers: [] }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    const row = requiredElement(document, '.sugarcubes-browser__cube-row');
    expect(row?.querySelector('.sugarcubes-browser__cube-title')?.textContent).toContain(
      '<img src=x onerror=1>',
    );
    expect(row?.querySelector('img')).toBeNull();

    row.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await flushPromises();

    const detailMeta = document.querySelector('.sugarcubes-browser__meta');
    const detailDescription = document.querySelector('.sugarcubes-browser__detail pre');
    expect(detailMeta?.textContent).toContain('<img src=x onerror=1>');
    expect(detailMeta?.textContent).toContain('<script>alert(1)</script>');
    expect(detailMeta?.textContent).toContain('<svg onload=1>');
    expect(detailDescription?.textContent).toBe('<b>Bold</b>');
    expect(document.querySelector('.sugarcubes-browser__meta img')).toBeNull();
    restore();
  });

  test('browser editor preserves literal name and description values', async () => {
    const restore = silenceConsole();
    api.fetchApi = async (url) => {
      if (url === '/sugarcubes/list') {
        return {
          ok: true,
          json: async () => ({
            cubes: [
              {
                name: '<img src=x onerror=1>',
                display_name: '<img src=x onerror=1>',
                cube_id: 'cube-a',
                is_writable: true,
                description: '<b>Bold</b>',
                tags: ['tag'],
                supported_models: [],
                mtime: '2024-01-01T00:00:00Z',
              },
            ],
          }),
        };
      }
      if (url === '/sugarcubes/load') {
        return {
          ok: true,
          json: async () => ({ layout: { origin: [0, 0] }, nodes: [], markers: [] }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    const row = requiredElement(document, '.sugarcubes-browser__cube-row');
    row.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await flushPromises();

    const editButton = requiredElement(document, '.sugarcubes-browser__edit');
    editButton.click();

    const nameInput = requiredElement(document, '.sugarcubes-browser__title-input');
    const descriptionInput = requiredElement(document, '.sugarcubes-browser__edit-textarea');
    expect(nameInput.value).toBe('<img src=x onerror=1>');
    expect(descriptionInput.value).toBe('<b>Bold</b>');
    expect(document.querySelector('.sugarcubes-browser__detail-title img')).toBeNull();
    restore();
  });

  test('browser editor uses one supported-models text field with comma-separated values', async () => {
    const restore = silenceConsole();
    api.fetchApi = async (url) => {
      if (url === '/sugarcubes/list') {
        return {
          ok: true,
          json: async () => ({
            cubes: [
              {
                name: 'Cube A',
                display_name: 'Cube A',
                cube_id: 'local/example-user/Cube A.cube',
                is_writable: true,
                description: 'Test cube',
                supported_models: ['SDXL', 'Flux .1 D'],
                tags: [],
                mtime: '2024-01-01T00:00:00Z',
              },
            ],
          }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    document
      .querySelector('.sugarcubes-browser__cube-row')
      ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await flushPromises();

    requiredElement(document, '.sugarcubes-browser__edit').click();
    await flushPromises();

    const modelInput = requiredElement(document, '.sugarcubes-browser__model-text-input');
    const suggestionList = requiredElement(document, '.sugarcubes-browser__model-suggestions');
    expect(modelInput.value).toBe('SDXL, Flux .1 D');
    expect(suggestionList.hidden).toBe(true);
    expect(document.querySelector('.sugarcubes-browser__model-select')).toBeNull();
    restore();
  });

  test('browser editor autocomplete inserts suggested model for the active token only', async () => {
    const restore = silenceConsole();
    api.fetchApi = async (url) => {
      if (url === '/sugarcubes/list') {
        return {
          ok: true,
          json: async () => ({
            cubes: [
              {
                name: 'Cube A',
                display_name: 'Cube A',
                cube_id: 'local/example-user/Cube A.cube',
                is_writable: true,
                description: 'Test cube',
                supported_models: ['SDXL'],
                tags: [],
                mtime: '2024-01-01T00:00:00Z',
              },
            ],
          }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };
    globalThis.fetch = async () =>
      ({
        ok: true,
        text: async () => 'SDXL\nFlux .1 D\nFlux .1 Kontext\n',
      }) as Response;

    await mountSidebar();
    expect(loadedUi?.cubeBrowser.store.state.modelOptions).toEqual([
      'SDXL',
      'Flux .1 D',
      'Flux .1 Kontext',
    ]);
    window.SugarCubes.openLibrary();
    await flushPromises();

    document
      .querySelector('.sugarcubes-browser__cube-row')
      ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await flushPromises();

    requiredElement(document, '.sugarcubes-browser__edit').click();
    await flushPromises();

    const modelInput = requiredElement(document, '.sugarcubes-browser__model-text-input');
    modelInput.value = 'SDXL, Fl';
    modelInput.focus();
    modelInput.setSelectionRange(modelInput.value.length, modelInput.value.length);
    modelInput.dispatchEvent(new Event('input', { bubbles: true }));

    const suggestions = Array.from(
      document.querySelectorAll('.sugarcubes-browser__model-suggestion'),
    );
    expect(suggestions.map((node) => node.textContent)).toEqual(['Flux .1 D', 'Flux .1 Kontext']);

    modelInput.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    expect(modelInput.value).toBe('SDXL, Flux .1 D');
    restore();
  });

  test('browser editor saves supported models from comma-separated text including custom values', async () => {
    const restore = silenceConsole();
    const updateBodies: UnknownRecord[] = [];
    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/list') {
        return {
          ok: true,
          json: async () => ({
            cubes: [
              {
                name: 'Cube A',
                display_name: 'Cube A',
                cube_id: 'local/example-user/Cube A.cube',
                is_writable: true,
                description: 'Test cube',
                supported_models: [],
                tags: [],
                mtime: '2024-01-01T00:00:00Z',
              },
            ],
          }),
        };
      }
      if (url === '/sugarcubes/update_metadata') {
        updateBodies.push(parseRequestBody(options.body));
        return {
          ok: true,
          json: async () => ({
            cube: {
              name: 'Cube A',
              display_name: 'Cube A',
              cube_id: 'local/example-user/Cube A.cube',
              description: 'Test cube',
              supported_models: ['SDXL', 'Custom Family', 'Flux .1 D'],
              tags: [],
              mtime: '2024-01-01T00:00:00Z',
            },
          }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    document
      .querySelector('.sugarcubes-browser__cube-row')
      ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await flushPromises();

    requiredElement(document, '.sugarcubes-browser__edit').click();
    await flushPromises();

    const modelInput = requiredElement(document, '.sugarcubes-browser__model-text-input');
    modelInput.value = 'SDXL, Custom Family, , Flux .1 D ';

    requiredElement(document, '.sugarcubes-browser__edit-save').click();
    await flushPromises();
    await flushPromises();

    expect(updateBodies).toHaveLength(1);
    const updatedMetadata = updateBodies[0]?.metadata;
    if (!updatedMetadata || typeof updatedMetadata !== 'object') {
      throw new Error('Missing updated metadata');
    }
    expect((updatedMetadata as UnknownRecord).supported_models).toEqual([
      'SDXL',
      'Custom Family',
      'Flux .1 D',
    ]);
    restore();
  });

  test('browser editor updates display name through metadata when derived id stays the same', async () => {
    const restore = silenceConsole();
    const requests: ApiRequest[] = [];
    api.fetchApi = async (url, options = {}) => {
      requests.push({ url, options });
      if (url === '/sugarcubes/list') {
        return {
          ok: true,
          json: async () => ({
            cubes: [
              {
                name: 'automask detailer',
                display_name: 'automask detailer',
                cube_id: 'local/example-user/Automask Detailer.cube',
                is_writable: true,
                description: 'Test cube',
                supported_models: ['SDXL'],
                tags: [],
                mtime: '2024-01-01T00:00:00Z',
              },
            ],
          }),
        };
      }
      if (url === '/sugarcubes/update_metadata') {
        return {
          ok: true,
          json: async () => ({
            cube: {
              name: 'automask detailer',
              display_name: 'Automask Detailer',
              cube_id: 'local/example-user/Automask Detailer.cube',
              description: 'Test cube',
              supported_models: ['Flux .1 D'],
              tags: [],
              mtime: '2024-01-01T00:00:00Z',
            },
          }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    document
      .querySelector('.sugarcubes-browser__cube-row')
      ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await flushPromises();

    requiredElement(document, '.sugarcubes-browser__edit').click();
    await flushPromises();

    const titleInput = requiredElement(document, '.sugarcubes-browser__title-input');
    const targetIdField = Array.from(
      document.querySelectorAll('.sugarcubes-browser__edit-field'),
    ).find((field) => field.textContent.includes('Target ID'));
    const newIdInput = Array.from(document.querySelectorAll('.sugarcubes-browser__edit-field'))
      .find((field) => field.textContent.includes('New ID'))
      ?.querySelector('input');
    expect(titleInput?.value).toBe('automask detailer');
    expect(targetIdField).toBeUndefined();
    expect(newIdInput?.value).toBe('local/example-user/Automask Detailer.cube');

    titleInput.value = 'automask detailer';
    titleInput.dispatchEvent(new Event('input', { bubbles: true }));
    expect(newIdInput?.value).toBe('local/example-user/Automask Detailer.cube');
    requiredElement(document, '.sugarcubes-browser__edit-save').click();
    await flushPromises();
    await flushPromises();

    expect(requests.some((entry) => entry.url === '/sugarcubes/rename')).toBe(false);
    expect(requests.some((entry) => entry.url === '/sugarcubes/update_metadata')).toBe(true);
    const updateRequest = requests.find((entry) => entry.url === '/sugarcubes/update_metadata');
    if (!updateRequest) throw new Error('Missing metadata update request');
    expect(parseRequestBody(updateRequest.options.body)).toEqual({
      cube_id: 'local/example-user/Automask Detailer.cube',
      description: 'Test cube',
      metadata: {
        default_alias: 'Automask Detailer',
        author_url: '',
        tags: [],
        supported_models: ['SDXL'],
      },
    });
    restore();
  });

  test('browser editor derives identity move when default alias changes', async () => {
    const restore = silenceConsole();
    const requests: ApiRequest[] = [];
    api.fetchApi = async (url, options = {}) => {
      requests.push({ url, options });
      if (url === '/sugarcubes/list') {
        return {
          ok: true,
          json: async () => ({
            cubes: [
              {
                name: 'automask detailer',
                display_name: 'automask detailer',
                cube_id: 'local/example-user/Automask Detailer.cube',
                is_writable: true,
                description: 'Test cube',
                supported_models: ['SDXL'],
                tags: [],
                version: '1.2.3',
                mtime: '2024-01-01T00:00:00Z',
              },
            ],
          }),
        };
      }
      if (url === '/sugarcubes/rename') {
        return {
          ok: true,
          json: async () => ({
            cube: {
              name: 'automask detailer v2',
              display_name: 'Automask Detailer v2',
              cube_id: 'local/example-user/Automask Detailer v2.cube',
            },
          }),
        };
      }
      if (url === '/sugarcubes/update_metadata') {
        return {
          ok: true,
          json: async () => ({
            cube: {
              name: 'automask detailer v2',
              display_name: 'Automask Detailer v2',
              cube_id: 'local/example-user/Automask Detailer v2.cube',
              description: 'Test cube',
              supported_models: ['SDXL'],
              tags: [],
              version: '1.0.0',
              mtime: '2024-01-01T00:00:00Z',
            },
          }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    document
      .querySelector('.sugarcubes-browser__cube-row')
      ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await flushPromises();

    requiredElement(document, '.sugarcubes-browser__edit').click();
    await flushPromises();

    const titleInput = requiredElement(document, '.sugarcubes-browser__title-input');
    const targetIdField = Array.from(
      document.querySelectorAll('.sugarcubes-browser__edit-field'),
    ).find((field) => field.textContent.includes('Target ID'));
    const newIdInput = Array.from(document.querySelectorAll('.sugarcubes-browser__edit-field'))
      .find((field) => field.textContent.includes('New ID'))
      ?.querySelector('input');
    expect(targetIdField).toBeUndefined();
    expect(newIdInput?.value).toBe('local/example-user/Automask Detailer.cube');
    titleInput.value = 'automask detailer v2';
    titleInput.dispatchEvent(new Event('input', { bubbles: true }));
    expect(newIdInput?.value).toBe('local/example-user/Automask Detailer v2.cube');

    requiredElement(document, '.sugarcubes-browser__edit-save').click();
    await flushPromises();
    requiredElement(document, '.sugarcubes-confirm__confirm').click();
    await flushPromises();
    await flushPromises();

    const renameRequest = requests.find((entry) => entry.url === '/sugarcubes/rename');
    expect(renameRequest).toBeTruthy();
    if (!renameRequest) throw new Error('Missing rename request');
    expect(parseRequestBody(renameRequest.options.body)).toEqual({
      cube_id: 'local/example-user/Automask Detailer.cube',
      default_alias: 'Automask Detailer v2',
      target_cube_id: 'local/example-user/Automask Detailer v2.cube',
    });
    const updateRequest = requests.find((entry) => entry.url === '/sugarcubes/update_metadata');
    if (!updateRequest) throw new Error('Missing metadata update request');
    expect(parseRequestBody(updateRequest.options.body)).toEqual({
      cube_id: 'local/example-user/Automask Detailer v2.cube',
      description: 'Test cube',
      metadata: {
        default_alias: 'Automask Detailer v2',
        author_url: '',
        tags: [],
        supported_models: ['SDXL'],
      },
      version: '1.2.3',
    });
    restore();
  });

  test('read-only cubes hide edit and delete actions in the browser', async () => {
    const restore = silenceConsole();
    api.fetchApi = async (url) => {
      if (url === '/sugarcubes/list') {
        return {
          ok: true,
          json: async () => ({
            cubes: [
              {
                name: 'Cube A',
                display_name: 'Cube A',
                cube_id: 'cube-a',
                description: 'Test cube',
                is_writable: false,
                write_block_reason:
                  'Tracked GitHub repos are read-only until you claim one GitHub owner.',
                supported_models: [],
                tags: [],
                mtime: '2024-01-01T00:00:00Z',
              },
            ],
          }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    await mountSidebar();
    window.SugarCubes.openLibrary();
    await flushPromises();

    document
      .querySelector('.sugarcubes-browser__cube-row')
      ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await flushPromises();

    const editButton = document.querySelector('.sugarcubes-browser__edit');
    const deleteButton = document.querySelector('.sugarcubes-browser__delete');

    expect(editButton?.classList.contains('sugarcubes-browser__action-hidden')).toBe(true);
    expect(deleteButton?.classList.contains('sugarcubes-browser__action-hidden')).toBe(true);
    restore();
  });
});
