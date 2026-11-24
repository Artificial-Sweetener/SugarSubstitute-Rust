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
 * Own the tracked Cube Packs and their settings rows: the Library page as Python presents it
 * (the Cube Library summary with Update all, Add Cube Pack, and each tracked pack as an expander
 * with its switches, details and actions), the claimed GitHub owner, and proximity links.
 */
import { HostSettingsRepositoryController } from './HostSettingsRepositoryController.js';
import {
  createRepoPanelState,
  type RepoPanelState,
  type ToastSeverity,
  type TrackedRepo,
} from './HostSettingsContracts.js';
import type { ComfyAdapter } from '../core/ComfyAdapter.js';
import type { CubeLibraryApi } from '../core/CubeLibraryApi.js';
import type { SugarCubesUI } from '../SugarCubesUI.js';
import { settingsCatalogFor } from './workspace/SettingsCatalog.js';
import { renderSettingsRow, type SettingsCardView } from './workspace/SettingsCards.js';
import { SettingsState, type SettingsFact, type SettingsRow } from './workspace/SettingsModel.js';

interface HostSettingsDependencies {
  adapter: ComfyAdapter;
  appRef: unknown;
  cubeApi: CubeLibraryApi;
  ui: SugarCubesUI;
  logger: Pick<Console, 'warn'>;
  pushToast(severity: ToastSeverity, summary: string, detail: string): void;
  readErrorMessage(error: unknown): string;
  invalidateDependentCatalogs(): void;
}

export interface HostSettingsController {
  register(): void;
  refresh(options?: { checkForUpdates?: boolean }): Promise<void>;
  refreshUi(): void;
}

/** Create the repository-scoped host settings controller. */
export function createHostSettingsController(
  dependencies: HostSettingsDependencies,
): HostSettingsController {
  const { adapter, appRef, cubeApi, ui } = dependencies;
  const overlayManager = ui.overlayManager;
  const readErrorMessage = dependencies.readErrorMessage;
  const repoPanelState: RepoPanelState = createRepoPanelState();
  /** Bumped whenever the packs or policy change, so live rows read them again. */
  const revision = new SettingsState(0);
  const changed: SettingsFact<number>['subscribe'] = (listener) => revision.subscribe(listener);
  const repositoryCommands = new HostSettingsRepositoryController({
    adapter,
    cubeApi,
    ui,
    state: repoPanelState,
    pushToast: dependencies.pushToast,
    readErrorMessage,
    invalidateDependentCatalogs: dependencies.invalidateDependentCatalogs,
    refreshUi,
  });
  let registered = false;
  let library: HTMLElement | null = null;
  const libraryCards: SettingsCardView[] = [];

  function refreshUi(): void {
    revision.set(revision.read() + 1);
    renderLibrary();
  }

  /** Run a pack command, turning a failure into a thrown error the card reports. */
  const command = (run: () => Promise<unknown>) => async (): Promise<void> => {
    await run();
  };

  function formatTimestamp(value: unknown): string {
    if (typeof value !== 'string' || !value.trim()) return 'Never';
    const parsed = new Date(value);
    return Number.isNaN(parsed.getTime()) ? value : parsed.toLocaleString();
  }

  function packStatus(repo: TrackedRepo): string {
    if (!repo.enabled) return 'Disabled';
    if (repo.last_sync_status === 'updating') return 'Updating';
    if (repo.last_check_status === 'error' || repo.last_sync_status === 'error') return 'Error';
    if (repo.update_available) return 'Update available';
    return typeof repo.last_checked_at === 'string' && repo.last_checked_at
      ? 'Up to date'
      : 'Never checked';
  }

  function packOwnership(repo: TrackedRepo): string {
    if (repo.is_writable) return 'My Pack';
    if (repo.is_system_pack) return 'System Pack';
    return 'Tracked Pack';
  }

  function librarySummary(): string {
    if (repoPanelState.loading)
      return repoPanelState.checking ? 'Checking tracked packs…' : 'Loading tracked packs…';
    if (repoPanelState.error) return repoPanelState.error;
    const count = repoPanelState.repos.length;
    if (!count) return 'No Cube Packs tracked.';
    const updates = repoPanelState.repos.filter(
      (repo) => repo.enabled && repo.update_available,
    ).length;
    return `${count} tracked pack${count === 1 ? '' : 's'} · ${
      updates ? `${updates} update${updates === 1 ? '' : 's'} available` : 'up to date'
    }`;
  }

  const idle: SettingsFact<boolean> = { read: () => !repoPanelState.loading, subscribe: changed };

  /** One tracked pack as Python's pack expander: status, switches, details and actions. */
  function packRow(repo: TrackedRepo): SettingsRow {
    const detail =
      (typeof repo.last_check_error === 'string' && repo.last_check_error) ||
      (typeof repo.last_sync_error === 'string' && repo.last_sync_error) ||
      (typeof repo.write_block_reason === 'string' && repo.write_block_reason) ||
      '';
    const details: SettingsRow[] = [
      {
        id: `library.pack.${repo.owner}/${repo.repo}.auto_update`,
        title: 'Auto-update',
        description: 'Update this pack when a newer version is published.',
        controls: [
          {
            kind: 'switch',
            value: {
              read: () => Boolean(repo.auto_update),
              write: (next) => repositoryCommands.setAutoUpdate(repo, next),
              subscribe: changed,
            },
          },
        ],
        enabled: { read: () => !repoPanelState.loading && repo.enabled, subscribe: changed },
      },
      {
        id: `library.pack.${repo.owner}/${repo.repo}.source`,
        title: 'Source',
        description: `${repo.owner}/${repo.repo} · ${packOwnership(repo)}${repo.default_base_repo ? ' · Base Pack' : ''}`,
      },
      {
        id: `library.pack.${repo.owner}/${repo.repo}.checked`,
        title: 'Last checked',
        description: `${formatTimestamp(repo.last_checked_at)} · last updated ${formatTimestamp(repo.last_sync_at)} · ${repo.is_writable ? 'writable' : 'read-only'}`,
      },
      ...(detail
        ? [
            {
              id: `library.pack.${repo.owner}/${repo.repo}.detail`,
              title: 'Detail',
              description: detail,
            },
          ]
        : []),
    ];
    return {
      id: `library.pack.${repo.owner}/${repo.repo}`,
      title: repo.repo || repo.repo_ref || 'Tracked pack',
      description: packStatus(repo),
      icon: 'cube-multiple',
      wrap: 640,
      controls: [
        ...(repo.enabled && repo.update_available
          ? [
              {
                kind: 'button' as const,
                button: {
                  label: 'Update',
                  run: command(() => repositoryCommands.syncTrackedRepo(repo)),
                  enabled: idle,
                },
              },
            ]
          : []),
        ...(repo.default_base_repo
          ? []
          : [
              {
                kind: 'switch' as const,
                value: {
                  read: () => repo.enabled,
                  write: () => repositoryCommands.toggleTrackedRepo(repo),
                  subscribe: changed,
                },
              },
              {
                kind: 'button' as const,
                button: {
                  label: 'Remove',
                  run: command(() => repositoryCommands.removeTrackedRepo(repo)),
                  enabled: idle,
                },
              },
            ]),
      ],
      expander: { rows: details },
    };
  }

  /** The Library page's cards, drawn again whenever the packs change. */
  function renderLibrary(): void {
    if (!library) return;
    const document = library.ownerDocument;
    for (const view of libraryCards.splice(0)) view.dispose();
    const report = (error: unknown): void =>
      dependencies.pushToast('error', 'Cube Pack update failed', readErrorMessage(error));
    const sections: { title: string; rows: SettingsRow[] }[] = [
      {
        title: 'Cube Library',
        rows: [
          {
            id: 'library.summary',
            title: 'Cube Library',
            description: librarySummary(),
            icon: 'library',
            controls: [
              {
                kind: 'button',
                button: {
                  label: 'Update all',
                  run: command(() => repositoryCommands.syncAllTrackedRepos()),
                  enabled: {
                    read: () =>
                      !repoPanelState.loading &&
                      repoPanelState.repos.some((repo) => repo.enabled && repo.update_available),
                    subscribe: changed,
                  },
                },
              },
            ],
          },
          {
            id: 'library.add',
            title: 'Add Cube Pack',
            description: 'Track a Cube Pack from its GitHub repository.',
            icon: 'link-add',
            controls: [
              {
                kind: 'button',
                button: {
                  label: 'Add…',
                  run: command(() => repositoryCommands.addTrackedRepo()),
                  enabled: idle,
                  primary: true,
                },
              },
            ],
          },
        ],
      },
      { title: 'Tracked Cube Packs', rows: repoPanelState.repos.map(packRow) },
    ];
    const blocks = sections
      .filter((section) => section.rows.length)
      .map((section) => {
        const block = document.createElement('section');
        block.className = 'sc-sw-section';
        const title = document.createElement('h2');
        title.className = 'sc-sw-section-title';
        title.textContent = section.title;
        const cards = document.createElement('div');
        cards.className = 'sc-sw-cards';
        for (const row of section.rows) {
          const view = renderSettingsRow(document, row, report);
          libraryCards.push(view);
          cards.append(view.element);
        }
        block.append(title, cards);
        return block;
      });
    library.replaceChildren(...blocks);
  }

  function register(): void {
    if (registered) return;
    registered = true;
    const catalog = settingsCatalogFor(appRef);
    catalog.contribute('library.page', {
      body: (document) => {
        library ??= document.createElement('div');
        library.className = 'sc-sw-library';
        renderLibrary();
        if (!repoPanelState.loading)
          repositoryCommands.refresh({ checkForUpdates: false }).catch((error: unknown) => {
            dependencies.logger.warn('SugarCubes: failed to refresh tracked packs', error);
          });
        return library;
      },
    });
    const envManaged = (): boolean =>
      repoPanelState.identityPolicy.claimed_github_owner_source === 'dotenv' ||
      repoPanelState.identityPolicy.claimed_github_owner_source === 'process_env';
    catalog.contribute('library.authoring.claimed_owner', {
      controls: [
        {
          kind: 'status',
          text: {
            read: () => {
              const owner = repoPanelState.identityPolicy.claimed_github_owner || 'Not claimed';
              return envManaged() ? `${owner} · set by the environment` : owner;
            },
            subscribe: changed,
          },
        },
        {
          kind: 'button',
          button: {
            label: 'Claim…',
            run: command(() => repositoryCommands.claimGithubOwner()),
            enabled: { read: () => !repoPanelState.loading && !envManaged(), subscribe: changed },
          },
        },
        {
          kind: 'button',
          button: {
            label: 'Clear',
            run: command(() => repositoryCommands.clearGithubOwner()),
            enabled: {
              read: () =>
                !repoPanelState.loading &&
                !envManaged() &&
                Boolean(repoPanelState.identityPolicy.claimed_github_owner),
              subscribe: changed,
            },
          },
        },
      ],
    });
    catalog.contribute('controls.graph.proximity_links', {
      controls: [
        {
          kind: 'switch',
          value: {
            read: () => Boolean(overlayManager?.proximity?.settings?.enabled),
            write: (enabled) => {
              overlayManager?.proximity?.setEnabled?.(enabled);
              refreshUi();
            },
            subscribe: changed,
          },
        },
      ],
    });
  }

  return Object.freeze({
    register,
    refresh: (options?: { checkForUpdates?: boolean }) => repositoryCommands.refresh(options),
    refreshUi,
  });
}
