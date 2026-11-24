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
import { describe, expect, test, beforeEach, afterEach, jest } from '@jest/globals';
import { app as hostApp } from '/scripts/app.js';
import { api as hostApi } from '/scripts/api.js';
import type { MockSettingDefinition } from '../../frontend_core/support/mocks/app.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';
import type { SugarCubesUI } from '../../../frontend/comfyui/ui/SugarCubesUI.js';
import { preserveAppearanceRuntime } from '../../frontend_core/support/appearance_runtime.js';
type SettingsCatalogModule =
  typeof import('../../../frontend/comfyui/ui/settings/workspace/SettingsCatalog.js');
type SettingsCardsModule =
  typeof import('../../../frontend/comfyui/ui/settings/workspace/SettingsCards.js');
/** The settings modules of the loaded UI's own module registry. */
let workspace: { catalog: SettingsCatalogModule; cards: SettingsCardsModule } | null = null;
const settingsCatalogFor = (host: unknown) => {
  if (!workspace) throw new Error('The UI is not loaded.');
  return workspace.catalog.settingsCatalogFor(host);
};

let app = hostApp as unknown as (typeof import('../../frontend_core/support/mocks/app.js'))['app'];
let api = hostApi as unknown as (typeof import('../../frontend_core/support/mocks/api.js'))['api'];

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
    selectedItems: new Set(),
    getNodeMenuOptions: () => [],
    getCanvasMenuOptions: () => [],
    setDirty: jest.fn(),
    onAfterChange: () => {},
    onDrawForeground: () => {},
    onDrawBackground: () => {},
    processMouseMove: () => {},
    processMouseDown: () => {},
  };
  app.extensionManager.registerSidebarTab = () => {};
  const settings = app.ui?.settings;
  if (!settings) throw new Error('Mock settings were not initialized.');
  app.ui = {
    settings: {
      ...settings,
      addSetting: (payload: MockSettingDefinition) => registeredSettings.push(payload),
    },
  };
  app.clean = () => {};
  registeredSettings = [];
}

async function loadUi() {
  preserveAppearanceRuntime();
  const loaded: { value?: typeof import('../../../frontend/comfyui/ui.js') } = {};
  await jest.isolateModulesAsync(async () => {
    loaded.value = await import('../../../frontend/comfyui/ui.js');
    workspace = {
      catalog: await import('../../../frontend/comfyui/ui/settings/workspace/SettingsCatalog.js'),
      cards: await import('../../../frontend/comfyui/ui/settings/workspace/SettingsCards.js'),
    };
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

/** Draw one row a feature contributed to the settings workspace, as the workspace draws it. */
function renderRow(id: string): HTMLElement {
  const row = settingsCatalogFor(app).row(id);
  if (!row) throw new Error(`Missing settings row: ${id}`);
  if (!workspace) throw new Error('The UI is not loaded.');
  const view = workspace.cards.renderSettingsRow(document, row, () => undefined);
  document.body.appendChild(view.element);
  return view.element;
}

async function renderRowAsync(id: string, flushCount = 2): Promise<HTMLElement> {
  const element = renderRow(id);
  for (let index = 0; index < flushCount; index += 1) await flushPromises();
  return element;
}

/** The Library page body, which reads the tracked packs again when it is shown. */
async function openLibrary(flushCount = 3): Promise<HTMLElement> {
  return renderRowAsync('library.page', flushCount);
}

function button(root: ParentNode, label: string): HTMLButtonElement {
  const found = Array.from(root.querySelectorAll<HTMLButtonElement>('button')).find(
    (candidate) => candidate.textContent?.trim() === label,
  );
  if (!found) throw new Error(`Missing button: ${label}`);
  return found;
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
  const confirmButton = requiredElement(
    document,
    '.sugarcubes-form-dialog .p-dialog-footer button:last-child',
  );
  expect(confirmButton).not.toBeNull();
  confirmButton.click();
  await flushPromises();
}

function resetPage(): void {
  jest.resetModules();
  loadedUi = null;
  setupBaseApp();
  localStorage.clear();
  // Isolated entrypoints model fresh pages, including their document-owned stylesheet registry.
  document.head.replaceChildren();
  document.body.replaceChildren();
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
  // The host fetch boundary consumes only status and text for bundled model suggestions.
  globalThis.fetch = async () => ({ ok: true, text: async () => 'sdxl\nsd\n' }) as Response;
}

function retirePage(): void {
  // Host adapters outlive the UI service graph and release resources on page teardown.
  window.dispatchEvent(new Event('pagehide'));
  loadedUi?.dispose();
  loadedUi = null;
  jest.restoreAllMocks();
}

/** Prepare cold source transformation during collection, then retire its isolated service graph. */
async function prepareEntrypoint(): Promise<SugarCubesUI> {
  resetPage();
  try {
    await loadUi();
    if (!loadedUi) throw new Error('Prepared UI module did not create its owned service graph.');
    return loadedUi;
  } finally {
    retirePage();
  }
}

const preparedUi = await prepareEntrypoint();
beforeEach(resetPage);
afterEach(retirePage);

describe('host settings and public library APIs', () => {
  test('setup registers settings without the old sidebar or its browser resources', async () => {
    const restore = silenceConsole();
    const registerSidebarTab = jest.fn();
    app.extensionManager.registerSidebarTab = registerSidebarTab;
    try {
      await setupExtension();
      expect(loadedUi).not.toBe(preparedUi);
      expect(window.SugarCubes.readDesktopFrame()).toBeNull();
      expect(registerSidebarTab).not.toHaveBeenCalled();
      expect(registeredSettings.length).toBeGreaterThan(0);
      expect(document.querySelector('.sugarcubes-sidebar-panel, .sugarcubes-browser')).toBeNull();
      expect(document.querySelector('style[id*="sugarcubes-browser"]')).toBeNull();
      expect(loadedUi?.modelSuggestions.getModelSuggestions()).toEqual(['sdxl', 'sd']);
    } finally {
      restore();
    }
  });

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

  test('page teardown releases appearance adapters and their owned stylesheets', async () => {
    const restore = silenceConsole();
    await setupExtension();
    expect(document.getElementById('sugarcubes-cube-icon-appearance')).not.toBeNull();
    expect(document.getElementById('sugarcubes-accent-theme')).not.toBeNull();
    window.dispatchEvent(new Event('pagehide'));
    expect(document.getElementById('sugarcubes-cube-icon-appearance')).toBeNull();
    expect(document.getElementById('sugarcubes-accent-theme')).toBeNull();
    restore();
  });

  test('host settings rows sit on the Python pages, and ComfyUI draws none of them', async () => {
    const restore = silenceConsole();
    await setupExtension();
    const catalog = settingsCatalogFor(app);
    expect(catalog.row('library.page')).not.toBeNull();
    expect(catalog.row('library.authoring.claimed_owner')?.title).toBe('Claimed GitHub owner');
    expect(catalog.row('controls.graph.proximity_links')?.title).toBe('Proximity links');
    // Every SugarCubes value ComfyUI keeps is hidden: the workspace row is the one ComfyUI draws.
    expect(
      registeredSettings
        .filter(
          (entry) => String(entry.id).startsWith('SugarCubes.') && String(entry.type) !== 'hidden',
        )
        .map((entry) => entry.id),
    ).toEqual(['SugarCubes.Settings']);
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
    const library = await openLibrary();
    const summary = requiredElement(library, '[data-row="library.summary"]');
    expect(summary.textContent).toContain('1 tracked pack');
    expect(summary.textContent).toContain('up to date');
    expect(button(summary, 'Update all').disabled).toBe(true);
    expect(library.textContent).toContain('Base-Cubes');
    expect(library.textContent).not.toContain('Edit source');
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
    const library = await openLibrary();
    // Each pack is an expander card: its details show when it opens.
    const pack = requiredElement(
      library,
      '[data-row="library.pack.Artificial-Sweetener/Base-Cubes"]',
    );
    expect(pack.hasAttribute('data-open')).toBe(false);
    requiredElement(pack, '.sc-sw-card-head').click();
    expect(pack.hasAttribute('data-open')).toBe(true);
    expect(pack.textContent).toContain('Artificial-Sweetener/Base-Cubes');
    expect(pack.textContent).not.toContain('Branch:');
    expect(pack.textContent).not.toContain('Edit source');
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
    const library = await openLibrary(4);

    expect(repoReadCount).toBeGreaterThan(1);
    expect(library.textContent).toContain('New-Pack');
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
    await openLibrary();
    const ownerRow = await renderRowAsync('library.authoring.claimed_owner');

    expect(ownerRow.textContent).toContain('Artificial-Sweetener · set by the environment');
    expect(button(ownerRow, 'Claim…').disabled).toBe(true);
    expect(button(ownerRow, 'Clear').disabled).toBe(true);
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
    await openLibrary();
    const ownerRow = await renderRowAsync('library.authoring.claimed_owner');

    expect(ownerRow.textContent).toContain('ExampleUser');
    expect(ownerRow.textContent).not.toContain('set by the environment');
    expect(button(ownerRow, 'Claim…').disabled).toBe(false);
    expect(button(ownerRow, 'Clear').disabled).toBe(false);
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
    const library = await openLibrary();
    button(library, 'Add…').click();
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
    expect(library.textContent).toContain('Base-Cubes');
    expect(library.textContent).not.toContain('Branch:');
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
    const library = await openLibrary();
    button(library, 'Add…').click();
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
    const library = await openLibrary();
    requiredElement(
      library,
      '[data-row="library.pack.Artificial-Sweetener/Base-Cubes"] .sc-sw-card-head',
    ).click();
    expect(library.textContent).not.toContain('Edit source');
    expect(library.textContent).not.toContain('Branch:');
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
    const library = await openLibrary();
    requiredElement(
      library,
      '[data-row="library.pack.Artificial-Sweetener/Base-Cubes"] .sc-sw-card-head',
    ).click();
    requiredElement(
      library,
      '[data-row="library.pack.Artificial-Sweetener/Base-Cubes.auto_update"] [role="switch"]',
    ).click();
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

  test('proximity setting row updates enabled state', async () => {
    const restore = silenceConsole();
    await setupExtension();

    const row = await renderRowAsync('controls.graph.proximity_links');
    const toggle = requiredElement(row, '[role="switch"]');
    expect(toggle.getAttribute('aria-checked')).toBe('true');
    expect(row.textContent).toContain('connect automatically during queueing');

    toggle.click();
    await flushPromises();

    expect(toggle.getAttribute('aria-checked')).toBe('false');
    expect(loadedUi?.overlayManager.proximity.settings.enabled).toBe(false);
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
    // Startup reads of its own (such as SugarCubes' running version) are not this call.
    calls.length = 0;

    await window.SugarCubes.previewCube('cube/id/\u6d4b\u8bd5');
    expect(calls[0]).toBe('/sugarcubes/preview?cube_id=cube%2Fid%2F%E6%B5%8B%E8%AF%95');
    restore();
  });
});
