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
 * Own SugarCubes host settings and tracked-pack management.
 */

import { TrackedPackManagerDialog } from './TrackedPackManagerDialog.js';
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
import type { UnknownRecord } from '../types/common.js';

type StatusTone = 'neutral' | 'muted' | 'error' | 'accent';

interface ClaimedOwnerRow {
  root: HTMLDivElement;
  summary: HTMLDivElement;
  value: HTMLSpanElement;
  claimOwnerButton: HTMLButtonElement;
  clearClaimButton: HTMLButtonElement;
}

interface ProximityRow {
  root: HTMLDivElement;
  summary: HTMLDivElement;
  toggle: HTMLInputElement;
  state: HTMLSpanElement;
}

interface SummaryRow {
  root: HTMLDivElement;
  summary: HTMLDivElement;
}

interface ManagerRow extends SummaryRow {
  openButton: HTMLButtonElement;
}

interface SettingsUiState {
  claimedOwner: ClaimedOwnerRow | null;
  proximity: ProximityRow | null;
  trackedPacks: SummaryRow | null;
  manageTrackedPacks: ManagerRow | null;
}

export interface SettingsManager {
  addSetting(setting: UnknownRecord): void;
}

interface SettingsHostApp {
  ui?: { settings?: SettingsManager };
}

interface HostSettingsDependencies {
  adapter: ComfyAdapter;
  appRef: SettingsHostApp | null;
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
  const { adapter, appRef, cubeApi, ui, logger } = dependencies;
  const documentRef = adapter.getDocument();
  const overlayManager = ui.overlayManager;
  const pushToastMessage = dependencies.pushToast;
  const readErrorMessage = dependencies.readErrorMessage;
  const invalidateDependentCatalogs = dependencies.invalidateDependentCatalogs;
  let registeredSettingsManager: SettingsManager | null = null;
  const settingsUiState: SettingsUiState = {
    claimedOwner: null,
    proximity: null,
    trackedPacks: null,
    manageTrackedPacks: null,
  };
  const repoPanelState: RepoPanelState = createRepoPanelState();
  const trackedPackManagerDialog = new TrackedPackManagerDialog({ adapter });
  const repositoryCommands = new HostSettingsRepositoryController({
    adapter,
    cubeApi,
    ui,
    state: repoPanelState,
    pushToast: pushToastMessage,
    readErrorMessage,
    invalidateDependentCatalogs,
    refreshUi: refreshSugarCubesSettingsUi,
  });

  function formatRepoTimestamp(value: unknown): string {
    if (typeof value !== 'string' || !value.trim()) {
      return 'Never';
    }
    const parsed = new Date(value);
    if (Number.isNaN(parsed.getTime())) {
      return value;
    }
    return parsed.toLocaleString();
  }

  function summarizePackStatus(repo: TrackedRepo | null | undefined): {
    label: string;
    tone: StatusTone;
  } {
    if (!repo || typeof repo !== 'object') {
      return { label: 'Never checked', tone: 'neutral' };
    }
    if (!repo.enabled) {
      return { label: 'Disabled', tone: 'muted' };
    }
    if (typeof repo.last_sync_status === 'string' && repo.last_sync_status === 'updating') {
      return { label: 'Updating', tone: 'neutral' };
    }
    if (
      (typeof repo.last_check_status === 'string' && repo.last_check_status === 'error') ||
      (typeof repo.last_sync_status === 'string' && repo.last_sync_status === 'error')
    ) {
      return { label: 'Error', tone: 'error' };
    }
    if (repo.update_available) {
      return { label: 'Update available', tone: 'accent' };
    }
    if (typeof repo.last_checked_at === 'string' && repo.last_checked_at) {
      return { label: 'Up to date', tone: 'neutral' };
    }
    return { label: 'Never checked', tone: 'muted' };
  }

  function describeRepoOwnership(
    repo: TrackedRepo | null | undefined,
  ): { label: string; tone: StatusTone } | null {
    if (!repo || typeof repo !== 'object') {
      return null;
    }
    if (repo.is_writable) {
      return { label: 'My Pack', tone: 'accent' };
    }
    if (repo.is_system_pack) {
      return { label: 'System Pack', tone: 'muted' };
    }
    return { label: 'Tracked Pack', tone: 'neutral' };
  }

  function isEnvManagedPolicySource(source: unknown): boolean {
    return source === 'dotenv' || source === 'process_env';
  }

  function applySettingsRowShellStyle(element: HTMLElement): void {
    Object.assign(element.style, {
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'flex-end',
      gap: '4px',
      minWidth: '0',
      textAlign: 'right',
    });
  }

  function applySettingsSummaryStyle(element: HTMLElement): void {
    Object.assign(element.style, {
      fontSize: '11px',
      lineHeight: '1.45',
      whiteSpace: 'pre-wrap',
      opacity: '0.72',
      maxWidth: '28rem',
    });
  }

  function applySettingsControlsStyle(element: HTMLElement): void {
    Object.assign(element.style, {
      display: 'inline-flex',
      alignItems: 'center',
      justifyContent: 'flex-end',
      gap: '8px',
      flexWrap: 'wrap',
    });
  }

  function applySettingsValueStyle(element: HTMLElement): void {
    Object.assign(element.style, {
      fontSize: '12px',
      lineHeight: '1.4',
    });
  }

  function applySettingsActionGroupStyle(element: HTMLElement): void {
    Object.assign(element.style, {
      display: 'flex',
      alignItems: 'center',
      gap: '8px',
      flexWrap: 'wrap',
    });
  }

  function applySettingsListStyle(element: HTMLElement): void {
    Object.assign(element.style, {
      display: 'flex',
      flexDirection: 'column',
      gap: '10px',
    });
  }

  function createSettingsActionButton(
    label: string,
    onClick: EventListener,
    variant: 'primary' | 'secondary' = 'secondary',
  ): HTMLButtonElement {
    if (!documentRef) {
      throw new Error('Document unavailable while creating SugarCubes settings controls.');
    }
    const button = documentRef.createElement('button');
    button.type = 'button';
    button.className =
      variant === 'primary'
        ? 'p-button p-component p-button-sm'
        : 'p-button p-component p-button-sm p-button-text p-button-secondary';
    const text = documentRef.createElement('span');
    text.className = 'p-button-label';
    text.textContent = label;
    button.appendChild(text);
    button.addEventListener('click', onClick);
    return button;
  }

  function setSettingsActionButtonLabel(button: HTMLButtonElement, label: string): void {
    const text = button?.querySelector?.('.p-button-label');
    if (text) {
      text.textContent = label;
      return;
    }
    if (button) {
      button.textContent = label;
    }
  }

  function appendSettingsSummaryLine(container: HTMLElement, text: string): void {
    if (!documentRef) {
      return;
    }
    const line = documentRef.createElement('div');
    line.textContent = text;
    container.appendChild(line);
  }

  function refreshClaimedGithubOwnerSettingRow(): void {
    const row = settingsUiState.claimedOwner;
    if (!row) {
      return;
    }
    const claimedOwner = repoPanelState.identityPolicy.claimed_github_owner;
    const ownerEnvManaged = isEnvManagedPolicySource(
      repoPanelState.identityPolicy.claimed_github_owner_source,
    );
    row.value.textContent = claimedOwner || 'Not claimed';
    row.claimOwnerButton.disabled = repoPanelState.loading || ownerEnvManaged;
    setSettingsActionButtonLabel(row.claimOwnerButton, claimedOwner ? 'Change' : 'Claim');
    row.clearClaimButton.disabled = repoPanelState.loading || ownerEnvManaged;
    row.clearClaimButton.hidden = !claimedOwner;
    row.summary.replaceChildren();
    appendSettingsSummaryLine(
      row.summary,
      claimedOwner
        ? 'SugarCubes will treat matching tracked repos as writable. local/... always remains writable.'
        : 'No GitHub owner is currently claimed. Only local/... stays writable.',
    );
    if (!repoPanelState.identityPolicy.allow_system_owner_claim) {
      appendSettingsSummaryLine(
        row.summary,
        'Artificial-Sweetener can only be claimed when SUGARCUBES_ALLOW_SYSTEM_OWNER_CLAIM is enabled in .env or the process environment.',
      );
    }
    if (repoPanelState.identityPolicy.env_override_active) {
      appendSettingsSummaryLine(
        row.summary,
        'Managed by environment configuration (.env or process environment).',
      );
    } else {
      appendSettingsSummaryLine(
        row.summary,
        'Changing this updates which tracked repos SugarCubes treats as writable on this install.',
      );
    }
  }

  function createClaimedGithubOwnerSettingRow(): HTMLDivElement | null {
    if (!documentRef) {
      return null;
    }
    const root = documentRef.createElement('div');
    root.className = 'sugarcubes-settings-row sugarcubes-settings-row--claimed-owner';
    applySettingsRowShellStyle(root);

    const controls = documentRef.createElement('div');
    applySettingsControlsStyle(controls);

    const value = documentRef.createElement('span');
    applySettingsValueStyle(value);

    const actions = documentRef.createElement('div');
    applySettingsActionGroupStyle(actions);

    const claimOwnerButton = createSettingsActionButton('Claim', () => {
      repositoryCommands.claimGithubOwner().catch((error: unknown) => {
        pushToastMessage('error', 'Settings update failed', readErrorMessage(error));
      });
    });
    const clearClaimButton = createSettingsActionButton('Clear', () => {
      repositoryCommands.clearGithubOwner().catch((error: unknown) => {
        pushToastMessage('error', 'Settings update failed', readErrorMessage(error));
      });
    });

    actions.append(claimOwnerButton, clearClaimButton);
    controls.append(value, actions);

    const summary = documentRef.createElement('div');
    applySettingsSummaryStyle(summary);

    root.append(controls, summary);

    settingsUiState.claimedOwner = {
      root,
      summary,
      value,
      claimOwnerButton,
      clearClaimButton,
    };
    refreshClaimedGithubOwnerSettingRow();
    return root;
  }

  function refreshProximitySettingRow(): void {
    const row = settingsUiState.proximity;
    if (!row) {
      return;
    }
    const enabled = Boolean(overlayManager?.proximity?.settings?.enabled);
    row.toggle.checked = enabled;
    row.state.textContent = enabled ? 'Enabled' : 'Disabled';
    row.summary.textContent = enabled
      ? 'Nearby compatible cube markers connect automatically during queueing, and the overlay preview remains active.'
      : 'Queueing will not auto-connect nearby compatible cube markers while proximity links are disabled.';
  }

  function createProximitySettingRow(): HTMLDivElement | null {
    if (!documentRef) {
      return null;
    }
    const root = documentRef.createElement('div');
    root.className = 'sugarcubes-settings-row sugarcubes-settings-row--proximity';
    applySettingsRowShellStyle(root);

    const controls = documentRef.createElement('div');
    applySettingsControlsStyle(controls);

    const toggleWrapper = documentRef.createElement('label');
    Object.assign(toggleWrapper.style, {
      display: 'inline-flex',
      alignItems: 'center',
      gap: '8px',
      fontSize: '12px',
    });
    const toggle = documentRef.createElement('input');
    toggle.type = 'checkbox';
    toggle.addEventListener('change', () => {
      overlayManager?.proximity?.setEnabled?.(toggle.checked);
      refreshProximitySettingRow();
      pushToastMessage(
        'success',
        'Proximity links updated',
        toggle.checked
          ? 'SugarCubes proximity links are enabled.'
          : 'SugarCubes proximity links are disabled.',
      );
    });
    const state = documentRef.createElement('span');
    applySettingsValueStyle(state);
    toggleWrapper.append(toggle, state);

    const summary = documentRef.createElement('div');
    applySettingsSummaryStyle(summary);

    controls.appendChild(toggleWrapper);
    root.append(controls, summary);

    settingsUiState.proximity = { root, summary, toggle, state };
    refreshProximitySettingRow();
    return root;
  }

  function refreshSugarCubesSettingsUi(): void {
    refreshClaimedGithubOwnerSettingRow();
    refreshProximitySettingRow();
    refreshTrackedPacksSettingRow();
    refreshManageTrackedPacksSettingRow();
    refreshTrackedPackManagerDialog();
  }

  function buildTrackedPackSummaryText(): string {
    if (repoPanelState.loading) {
      return repoPanelState.checking ? 'Checking tracked packs...' : 'Loading tracked packs...';
    }
    if (repoPanelState.error) {
      return repoPanelState.error;
    }
    if (!repoPanelState.repos.length) {
      return 'No tracked packs configured.';
    }
    const packCount = repoPanelState.repos.length;
    const updateCount = repoPanelState.repos.filter(
      (repo) => repo.enabled && repo.update_available,
    ).length;
    const writableCount = repoPanelState.repos.filter((repo) => repo.is_writable).length;
    const summaryParts = [
      `${packCount} tracked pack${packCount === 1 ? '' : 's'}.`,
      `${writableCount} writable.`,
    ];
    summaryParts.push(
      updateCount > 0
        ? `${updateCount} update${updateCount === 1 ? '' : 's'} available.`
        : 'No updates available.',
    );
    return summaryParts.join(' ');
  }

  function buildTrackedPackManagerSummaryText(): string {
    if (repoPanelState.loading) {
      return repoPanelState.checking ? 'Checking tracked packs…' : 'Loading tracked packs…';
    }
    if (repoPanelState.error) {
      return 'Resolve the load error before editing tracked packs.';
    }
    if (!repoPanelState.repos.length) {
      return 'Add a tracked pack to keep shared cubes available in the library.';
    }
    const updateCount = repoPanelState.repos.filter(
      (repo) => repo.enabled && repo.update_available,
    ).length;
    if (updateCount > 0) {
      return `${updateCount} tracked pack update${updateCount === 1 ? '' : 's'} available.`;
    }
    return 'Add, update, remove, and configure auto-update.';
  }

  function refreshTrackedPacksSettingRow(): void {
    const row = settingsUiState.trackedPacks;
    if (!row) {
      return;
    }
    row.summary.textContent = buildTrackedPackSummaryText();
  }

  function createTrackedPacksSettingRow(): HTMLDivElement | null {
    if (!documentRef) {
      return null;
    }
    const root = documentRef.createElement('div');
    root.className = 'sugarcubes-settings-row sugarcubes-settings-row--tracked-packs';
    applySettingsRowShellStyle(root);

    const summary = documentRef.createElement('div');
    applySettingsSummaryStyle(summary);
    root.appendChild(summary);

    settingsUiState.trackedPacks = { root, summary };
    refreshTrackedPacksSettingRow();
    if (!repoPanelState.loading) {
      repositoryCommands.refresh({ checkForUpdates: false }).catch((error: unknown) => {
        logger.warn('SugarCubes: failed to refresh tracked pack summary', error);
      });
    }
    return root;
  }

  async function openTrackedPackManagerDialog(): Promise<void> {
    if (!documentRef || trackedPackManagerDialog.isOpen) {
      return;
    }
    await repositoryCommands.refresh({ checkForUpdates: false });
    if (trackedPackManagerDialog.isOpen) {
      return;
    }
    void trackedPackManagerDialog.open({
      title: 'Manage tracked packs',
      description: ['Add, update, enable, disable, remove, and configure tracked packs here.'],
      body: createTrackedPackManagerDialogBody(),
    });
    refreshTrackedPackManagerDialog();
  }

  function refreshManageTrackedPacksSettingRow(): void {
    const row = settingsUiState.manageTrackedPacks;
    if (!row) {
      return;
    }
    row.summary.textContent = buildTrackedPackManagerSummaryText();
    row.openButton.disabled = repoPanelState.loading && !repoPanelState.repos.length;
  }

  function createTrackedPackManagerSettingRow(): HTMLDivElement | null {
    if (!documentRef) {
      return null;
    }
    const root = documentRef.createElement('div');
    root.className = 'sugarcubes-settings-row sugarcubes-settings-row--tracked-pack-manager';
    applySettingsRowShellStyle(root);

    const controls = documentRef.createElement('div');
    applySettingsControlsStyle(controls);
    const openButton = createSettingsActionButton('Open Manager', () => {
      openTrackedPackManagerDialog().catch((error: unknown) => {
        pushToastMessage('error', 'Unable to open Cube Pack manager', readErrorMessage(error));
      });
    });
    controls.appendChild(openButton);

    const summary = documentRef.createElement('div');
    applySettingsSummaryStyle(summary);

    root.append(controls, summary);

    settingsUiState.manageTrackedPacks = {
      root,
      openButton,
      summary,
    };
    refreshManageTrackedPacksSettingRow();
    return root;
  }

  function registerSugarCubesSettings(): void {
    const settingsManager = appRef?.ui?.settings;
    if (!settingsManager?.addSetting || !documentRef) {
      return;
    }
    if (registeredSettingsManager === settingsManager) {
      return;
    }
    settingsUiState.claimedOwner = null;
    settingsUiState.proximity = null;
    settingsUiState.trackedPacks = null;
    settingsUiState.manageTrackedPacks = null;
    settingsManager.addSetting({
      defaultValue: '',
      id: 'SugarCubes.Authoring.ClaimedGithubOwner',
      category: ['SugarCubes', 'Authoring', 'ClaimedGithubOwner'],
      name: 'Claimed GitHub owner',
      sortOrder: 341,
      tooltip: 'Claim one GitHub owner to unlock writes for matching tracked repos.',
      type() {
        return settingsUiState.claimedOwner?.root || createClaimedGithubOwnerSettingRow();
      },
    });
    settingsManager.addSetting({
      defaultValue: '',
      id: 'SugarCubes.Graph.ProximityLinks',
      category: ['SugarCubes', 'Graph', 'ProximityLinks'],
      name: 'Enable proximity links',
      sortOrder: 342,
      tooltip: 'Enable or disable SugarCubes proximity connections.',
      type() {
        return settingsUiState.proximity?.root || createProximitySettingRow();
      },
    });
    settingsManager.addSetting({
      defaultValue: '',
      id: 'SugarCubes.CubePacks.TrackedPacks',
      category: ['SugarCubes', 'Cube Packs', 'TrackedPacks'],
      name: 'Tracked packs',
      sortOrder: 343,
      tooltip: 'Show the current tracked pack summary.',
      type() {
        return settingsUiState.trackedPacks?.root || createTrackedPacksSettingRow();
      },
    });
    settingsManager.addSetting({
      defaultValue: '',
      id: 'SugarCubes.CubePacks.Manager',
      category: ['SugarCubes', 'Cube Packs', 'Manager'],
      name: 'Manage tracked packs',
      sortOrder: 342.5,
      tooltip: 'Open the tracked pack manager.',
      type() {
        return settingsUiState.manageTrackedPacks?.root || createTrackedPackManagerSettingRow();
      },
    });
    registeredSettingsManager = settingsManager;
    refreshSugarCubesSettingsUi();
  }

  function buildTrackedPackStatusSummary(repo: TrackedRepo): string {
    const statusBits = [summarizePackStatus(repo).label];
    const ownership = describeRepoOwnership(repo);
    if (ownership?.label) {
      statusBits.push(ownership.label);
    }
    if (repo.default_base_repo) {
      statusBits.push('Base Pack');
    }
    return statusBits.join(' | ');
  }

  function createTrackedPackManagerDialogBody(): HTMLDivElement | null {
    if (!documentRef) {
      return null;
    }
    const root = documentRef.createElement('div');
    Object.assign(root.style, {
      display: 'grid',
      gap: '12px',
    });

    const actions = documentRef.createElement('div');
    applySettingsActionGroupStyle(actions);
    const addButton = createSettingsActionButton(
      'Add Pack',
      () => {
        repositoryCommands.addTrackedRepo().catch((error: unknown) => {
          const message = readErrorMessage(error);
          pushToastMessage('error', 'Pack add failed', message);
        });
      },
      'primary',
    );
    addButton.disabled = repoPanelState.loading;
    actions.appendChild(addButton);

    const updateCount = repoPanelState.repos.filter(
      (repo) => repo.enabled && repo.update_available,
    ).length;
    if (updateCount > 0) {
      const updateAllButton = createSettingsActionButton(
        updateCount > 1 ? `Update All (${updateCount})` : 'Update All',
        () => {
          repositoryCommands.syncAllTrackedRepos().catch((error: unknown) => {
            const message = readErrorMessage(error);
            pushToastMessage('error', 'Pack update failed', message);
          });
        },
      );
      updateAllButton.disabled = repoPanelState.loading;
      actions.appendChild(updateAllButton);
    }
    root.appendChild(actions);

    const status = documentRef.createElement('div');
    applySettingsSummaryStyle(status);
    status.style.textAlign = 'left';
    status.style.maxWidth = 'none';
    if (repoPanelState.loading) {
      status.textContent = repoPanelState.checking
        ? 'Checking tracked packs...'
        : 'Loading tracked packs...';
    } else if (repoPanelState.error) {
      status.textContent = repoPanelState.error;
    } else if (!repoPanelState.repos.length) {
      status.textContent = 'No tracked packs configured yet.';
    } else {
      status.textContent = buildTrackedPackSummaryText();
    }
    root.appendChild(status);

    if (!repoPanelState.repos.length) {
      return root;
    }

    const list = documentRef.createElement('div');
    applySettingsListStyle(list);
    for (const repo of repoPanelState.repos) {
      const row = documentRef.createElement('div');
      Object.assign(row.style, {
        display: 'grid',
        gap: '8px',
        padding: '10px 12px',
        borderRadius: '10px',
        border: '1px solid rgba(120, 140, 160, 0.2)',
        background: 'rgba(24, 32, 44, 0.42)',
      });

      const header = documentRef.createElement('div');
      Object.assign(header.style, {
        display: 'flex',
        alignItems: 'flex-start',
        justifyContent: 'space-between',
        gap: '10px',
      });
      const titleBlock = documentRef.createElement('div');
      Object.assign(titleBlock.style, {
        display: 'grid',
        gap: '2px',
        minWidth: '0',
      });
      const title = documentRef.createElement('div');
      title.textContent = repo.repo || repo.repo_ref || 'Tracked pack';
      Object.assign(title.style, {
        fontSize: '12px',
        fontWeight: '600',
      });
      const subtitle = documentRef.createElement('div');
      subtitle.textContent = `${repo.owner}/${repo.repo}`;
      Object.assign(subtitle.style, {
        fontSize: '11px',
        opacity: '0.72',
        wordBreak: 'break-word',
      });
      titleBlock.append(title, subtitle);

      const statusText = documentRef.createElement('div');
      statusText.textContent = buildTrackedPackStatusSummary(repo);
      Object.assign(statusText.style, {
        fontSize: '11px',
        opacity: '0.78',
        textAlign: 'right',
      });
      header.append(titleBlock, statusText);
      row.appendChild(header);

      const metadata = documentRef.createElement('div');
      metadata.textContent = [
        `Last checked: ${formatRepoTimestamp(repo.last_checked_at)}`,
        `Last updated: ${formatRepoTimestamp(repo.last_sync_at)}`,
        `Write access: ${repo.is_writable ? 'Writable' : 'Read-only'}`,
      ].join(' | ');
      Object.assign(metadata.style, {
        fontSize: '11px',
        lineHeight: '1.45',
        opacity: '0.78',
        whiteSpace: 'pre-wrap',
      });
      row.appendChild(metadata);

      const detailText =
        (typeof repo.last_check_error === 'string' && repo.last_check_error) ||
        (typeof repo.last_sync_error === 'string' && repo.last_sync_error) ||
        (typeof repo.write_block_reason === 'string' && repo.write_block_reason) ||
        '';
      if (detailText) {
        const detail = documentRef.createElement('div');
        detail.textContent = detailText;
        Object.assign(detail.style, {
          fontSize: '11px',
          lineHeight: '1.45',
          color:
            repo.last_check_error || repo.last_sync_error ? '#ffb3b3' : 'rgba(232, 232, 232, 0.72)',
          whiteSpace: 'pre-wrap',
        });
        row.appendChild(detail);
      }

      const controls = documentRef.createElement('div');
      Object.assign(controls.style, {
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        gap: '8px',
        flexWrap: 'wrap',
      });

      const autoUpdateLabel = documentRef.createElement('label');
      Object.assign(autoUpdateLabel.style, {
        display: 'inline-flex',
        alignItems: 'center',
        gap: '6px',
        fontSize: '11px',
        opacity: repo.enabled ? '1' : '0.72',
      });
      const autoUpdateToggle = documentRef.createElement('input');
      autoUpdateToggle.type = 'checkbox';
      autoUpdateToggle.checked = Boolean(repo.auto_update);
      autoUpdateToggle.disabled = repoPanelState.loading || !repo.enabled;
      autoUpdateToggle.addEventListener('change', () => {
        repositoryCommands.setAutoUpdate(repo, autoUpdateToggle.checked).catch((error: unknown) => {
          autoUpdateToggle.checked = Boolean(repo.auto_update);
          const message = readErrorMessage(error);
          pushToastMessage('error', 'Pack update failed', message);
        });
      });
      const autoUpdateText = documentRef.createElement('span');
      autoUpdateText.textContent = 'Auto-update';
      autoUpdateLabel.append(autoUpdateToggle, autoUpdateText);
      controls.appendChild(autoUpdateLabel);

      const actionCluster = documentRef.createElement('div');
      applySettingsActionGroupStyle(actionCluster);
      if (repo.enabled && repo.update_available) {
        const updateButton = createSettingsActionButton('Update', () => {
          repositoryCommands.syncTrackedRepo(repo).catch((error: unknown) => {
            const message = readErrorMessage(error);
            pushToastMessage('error', 'Pack update failed', message);
          });
        });
        updateButton.disabled = repoPanelState.loading;
        actionCluster.appendChild(updateButton);
      }
      if (!repo.default_base_repo) {
        const toggleButton = createSettingsActionButton(repo.enabled ? 'Disable' : 'Enable', () => {
          repositoryCommands.toggleTrackedRepo(repo).catch((error: unknown) => {
            const message = readErrorMessage(error);
            pushToastMessage('error', 'Pack update failed', message);
          });
        });
        toggleButton.disabled = repoPanelState.loading;
        actionCluster.appendChild(toggleButton);
      }
      if (!repo.default_base_repo) {
        const removeButton = createSettingsActionButton('Remove', () => {
          repositoryCommands.removeTrackedRepo(repo).catch((error: unknown) => {
            const message = readErrorMessage(error);
            pushToastMessage('error', 'Pack removal failed', message);
          });
        });
        removeButton.disabled = repoPanelState.loading;
        actionCluster.appendChild(removeButton);
      }
      controls.appendChild(actionCluster);
      row.appendChild(controls);
      list.appendChild(row);
    }
    root.appendChild(list);
    return root;
  }

  function refreshTrackedPackManagerDialog(): void {
    if (!trackedPackManagerDialog.isOpen) {
      return;
    }
    trackedPackManagerDialog.update({
      body: createTrackedPackManagerDialogBody(),
    });
  }

  return Object.freeze({
    register: registerSugarCubesSettings,
    refresh: (options?: { checkForUpdates?: boolean }) => repositoryCommands.refresh(options),
    refreshUi: refreshSugarCubesSettingsUi,
  });
}
