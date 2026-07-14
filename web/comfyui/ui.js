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
 * Own the SugarCubes host integration layer in `web/comfyui/ui.js`.
 */

import { app } from '/scripts/app.js';
import { api } from '/scripts/api.js';
import { writeWidgetValue } from './ui/graph/Markers.js';
import {
  getGroupSugarcubes,
  normalizeGroupInstanceAlias,
  setGroupSugarcubes,
} from './ui/graph/GroupMetadata.js';
import { coerceVec2, readVector2 } from './ui/graph/VectorUtils.js';
import {
  computePayloadBounds,
  drawGhostRect,
  getPlacementGroupLabel,
  readLayoutFlags,
  readLayoutStyle,
  resolvePreviewRect,
} from './ui/overlays/PlacementHelpers.js';
import { createPublicApi, getSugarCubesUI } from './ui/index.js';
import { computeInnerBounds } from './ui/graph/CubeBounds.js';
import { normalizeSubgraphPayload } from './ui/graph/SubgraphSerialization.js';
import { rebindSubgraphWidgetValues } from './ui/graph/WidgetSnapshots.js';
import { TrackedPackManagerDialog } from './ui/settings/TrackedPackManagerDialog.js';
import { applyCubeDefinitionIdentity } from './ui/core/CubeDefinitionKey.js';

const EXTENSION_NAME = 'SugarCubes.UI';
const IMPORT_STORAGE_KEY = 'SugarCubes.Import.LastCube';
const CUBE_INSTANCE_SCHEMA = 5;

const ui = getSugarCubesUI({
  forceNew: true,
  app,
  api,
  applyPreparedImport,
  reportImportOutcome,
  buildShiftedPlacementPayload,
});
const adapter = ui.adapter;
const storage = ui.storage;
const toastService = ui.toast;
const cubeApi = ui.api;
const overlayManager = ui.overlayManager;
const appRef = adapter.getApp();
const windowRef = adapter.getWindow() || {};
const documentRef = adapter.getDocument();
const consoleRef = adapter.getConsole();
const logger = consoleRef || {
  log() {},
  warn() {},
  error() {},
  info() {},
  debug() {},
};

let settingsRegistered = false;

let sidebarTabRegistered = false;
let sidebarRoot = null;
const settingsUiState = {
  claimedOwner: null,
  proximity: null,
  trackedPacks: null,
  manageTrackedPacks: null,
};
const repoPanelState = {
  loading: false,
  checking: false,
  repos: [],
  error: '',
  identityPolicy: {
    claimed_github_owner: '',
    allow_system_owner_claim: false,
    has_claimed_github_owner: false,
    claimed_github_owner_source: 'default',
    allow_system_owner_claim_source: 'default',
    env_override_active: false,
  },
};
const trackedPackManagerDialog = new TrackedPackManagerDialog({ adapter });

const LAST_CUBE_STORAGE_KEYS = Object.freeze([IMPORT_STORAGE_KEY]);

function persistLastCubeId(value) {
  if (value == null) {
    return;
  }
  const trimmed = typeof value === 'string' ? value.trim() : '';
  if (!trimmed) {
    return;
  }
  try {
    if (!storage) {
      return;
    }
    const seen = new Set();
    for (const key of LAST_CUBE_STORAGE_KEYS) {
      if (!key || seen.has(key)) {
        continue;
      }
      storage.writeValue(key, trimmed);
      seen.add(key);
    }
  } catch (_error) {
    // ignore storage persistence failures
  }
}

function pushToastMessage(severity, summary, detail) {
  toastService?.push?.(severity, summary, detail);
}

function invalidateDependentCatalogs() {
  ui.cubeBrowser.refresh({ force: true }).catch(() => {});
}

function formatRepoTimestamp(value) {
  if (typeof value !== 'string' || !value.trim()) {
    return 'Never';
  }
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }
  return parsed.toLocaleString();
}

function summarizePackStatus(repo) {
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

async function requestConfirmation({ title, message, confirmLabel }) {
  if (ui.dialogs?.confirm) {
    return ui.dialogs.confirm({ title, message, confirmLabel });
  }
  if (ui.confirmDialog?.open) {
    return ui.confirmDialog.open({ title, message, confirmLabel });
  }
  const confirmRef = adapter.getWindow?.()?.confirm || globalThis.confirm;
  if (typeof confirmRef !== 'function') {
    return false;
  }
  const lines = Array.isArray(message) ? message : [message];
  return Boolean(confirmRef(lines.filter(Boolean).join('\n')));
}

function parseRepoReference(value) {
  const normalized = typeof value === 'string' ? value.trim() : '';
  if (!normalized) {
    return { owner: '', repo: '' };
  }
  const [owner = '', repo = ''] = normalized.split('/', 2);
  return { owner: owner.trim(), repo: repo.trim() };
}

function validateRepoReference(value) {
  const { owner, repo } = parseRepoReference(value);
  if (!owner || !repo) {
    return 'Cube Pack source must use owner/repo format.';
  }
  return '';
}

function normalizeOwnerIdentity(value) {
  return typeof value === 'string' ? value.trim().toLowerCase() : '';
}

function readIdentityPolicyFromPayload(payload) {
  if (!payload || typeof payload !== 'object') {
    return {
      claimed_github_owner: '',
      allow_system_owner_claim: false,
      has_claimed_github_owner: false,
      claimed_github_owner_source: 'default',
      allow_system_owner_claim_source: 'default',
      env_override_active: false,
    };
  }
  const claimedOwner =
    typeof payload.claimed_github_owner === 'string' ? payload.claimed_github_owner.trim() : '';
  const allowSystemOwnerClaim = Boolean(payload.allow_system_owner_claim);
  const claimedOwnerSource =
    typeof payload.claimed_github_owner_source === 'string' && payload.claimed_github_owner_source
      ? payload.claimed_github_owner_source
      : 'default';
  const allowSystemOwnerClaimSource =
    typeof payload.allow_system_owner_claim_source === 'string' &&
    payload.allow_system_owner_claim_source
      ? payload.allow_system_owner_claim_source
      : 'default';
  return {
    claimed_github_owner: claimedOwner,
    allow_system_owner_claim: allowSystemOwnerClaim,
    has_claimed_github_owner: Boolean(claimedOwner),
    claimed_github_owner_source: claimedOwnerSource,
    allow_system_owner_claim_source: allowSystemOwnerClaimSource,
    env_override_active: Boolean(payload.env_override_active),
  };
}

function describeRepoOwnership(repo) {
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

function isEnvManagedPolicySource(source) {
  return source === 'dotenv' || source === 'process_env';
}

function applySettingsRowShellStyle(element) {
  Object.assign(element.style, {
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'flex-end',
    gap: '4px',
    minWidth: '0',
    textAlign: 'right',
  });
}

function applySettingsSummaryStyle(element) {
  Object.assign(element.style, {
    fontSize: '11px',
    lineHeight: '1.45',
    whiteSpace: 'pre-wrap',
    opacity: '0.72',
    maxWidth: '28rem',
  });
}

function applySettingsControlsStyle(element) {
  Object.assign(element.style, {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'flex-end',
    gap: '8px',
    flexWrap: 'wrap',
  });
}

function applySettingsValueStyle(element) {
  Object.assign(element.style, {
    fontSize: '12px',
    lineHeight: '1.4',
  });
}

function applySettingsActionGroupStyle(element) {
  Object.assign(element.style, {
    display: 'flex',
    alignItems: 'center',
    gap: '8px',
    flexWrap: 'wrap',
  });
}

function applySettingsListStyle(element) {
  Object.assign(element.style, {
    display: 'flex',
    flexDirection: 'column',
    gap: '10px',
  });
}

function createSettingsActionButton(label, onClick, variant = 'secondary') {
  if (!documentRef) {
    return null;
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

function setSettingsActionButtonLabel(button, label) {
  const text = button?.querySelector?.('.p-button-label');
  if (text) {
    text.textContent = label;
    return;
  }
  if (button) {
    button.textContent = label;
  }
}

function appendSettingsSummaryLine(container, text) {
  if (!documentRef) {
    return;
  }
  const line = documentRef.createElement('div');
  line.textContent = text;
  container.appendChild(line);
}

function refreshClaimedGithubOwnerSettingRow() {
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

function createClaimedGithubOwnerSettingRow() {
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
    handleClaimGithubOwner().catch((error) => {
      pushToastMessage('error', 'Settings update failed', error?.message || String(error));
    });
  });
  const clearClaimButton = createSettingsActionButton('Clear', () => {
    handleClearGithubOwnerClaim().catch((error) => {
      pushToastMessage('error', 'Settings update failed', error?.message || String(error));
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

function refreshProximitySettingRow() {
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

function createProximitySettingRow() {
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

function refreshSugarCubesSettingsUi() {
  refreshClaimedGithubOwnerSettingRow();
  refreshProximitySettingRow();
  refreshTrackedPacksSettingRow();
  refreshManageTrackedPacksSettingRow();
  refreshTrackedPackManagerDialog();
}

function buildTrackedPackSummaryText() {
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

function buildTrackedPackManagerSummaryText() {
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

function refreshTrackedPacksSettingRow() {
  const row = settingsUiState.trackedPacks;
  if (!row) {
    return;
  }
  row.summary.textContent = buildTrackedPackSummaryText();
}

function createTrackedPacksSettingRow() {
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
    refreshTrackedRepoPanel({ checkForUpdates: false }).catch((error) => {
      logger.warn('SugarCubes: failed to refresh tracked pack summary', error);
    });
  }
  return root;
}

async function openTrackedPackManagerDialog() {
  if (!documentRef || trackedPackManagerDialog.isOpen) {
    return;
  }
  await refreshTrackedRepoPanel({ checkForUpdates: false });
  if (trackedPackManagerDialog.isOpen) {
    return;
  }
  trackedPackManagerDialog.open({
    title: 'Manage tracked packs',
    description: ['Add, update, enable, disable, remove, and configure tracked packs here.'],
    body: createTrackedPackManagerDialogBody(),
  });
  refreshTrackedPackManagerDialog();
}

function refreshManageTrackedPacksSettingRow() {
  const row = settingsUiState.manageTrackedPacks;
  if (!row) {
    return;
  }
  row.summary.textContent = buildTrackedPackManagerSummaryText();
  row.openButton.disabled = repoPanelState.loading && !repoPanelState.repos.length;
}

function createTrackedPackManagerSettingRow() {
  if (!documentRef) {
    return null;
  }
  const root = documentRef.createElement('div');
  root.className = 'sugarcubes-settings-row sugarcubes-settings-row--tracked-pack-manager';
  applySettingsRowShellStyle(root);

  const controls = documentRef.createElement('div');
  applySettingsControlsStyle(controls);
  const openButton = createSettingsActionButton('Open Manager', () => {
    openTrackedPackManagerDialog().catch((error) => {
      pushToastMessage(
        'error',
        'Unable to open Cube Pack manager',
        error?.message || String(error),
      );
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

function registerSugarCubesSettings() {
  if (settingsRegistered) {
    return;
  }
  const settingsManager = appRef?.ui?.settings;
  if (!settingsManager?.addSetting || !documentRef) {
    return;
  }
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
  settingsRegistered = true;
  refreshSugarCubesSettingsUi();
}

function buildTrackedPackStatusSummary(repo) {
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

function createTrackedPackManagerDialogBody() {
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
      handleAddTrackedRepo().catch((error) => {
        const message = error?.message || String(error);
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
        handleSyncAllTrackedRepos().catch((error) => {
          const message = error?.message || String(error);
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
      handleSetPackAutoUpdate(repo, autoUpdateToggle.checked).catch((error) => {
        autoUpdateToggle.checked = Boolean(repo.auto_update);
        const message = error?.message || String(error);
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
        handleSyncTrackedRepo(repo).catch((error) => {
          const message = error?.message || String(error);
          pushToastMessage('error', 'Pack update failed', message);
        });
      });
      updateButton.disabled = repoPanelState.loading;
      actionCluster.appendChild(updateButton);
    }
    if (!repo.default_base_repo) {
      const toggleButton = createSettingsActionButton(repo.enabled ? 'Disable' : 'Enable', () => {
        handleToggleTrackedRepo(repo).catch((error) => {
          const message = error?.message || String(error);
          pushToastMessage('error', 'Pack update failed', message);
        });
      });
      toggleButton.disabled = repoPanelState.loading;
      actionCluster.appendChild(toggleButton);
    }
    if (!repo.default_base_repo) {
      const removeButton = createSettingsActionButton('Remove', () => {
        handleRemoveTrackedRepo(repo).catch((error) => {
          const message = error?.message || String(error);
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

function refreshTrackedPackManagerDialog() {
  if (!trackedPackManagerDialog.isOpen) {
    return;
  }
  trackedPackManagerDialog.update({
    body: createTrackedPackManagerDialogBody(),
  });
}

async function refreshTrackedRepoPanel({ checkForUpdates = true } = {}) {
  repoPanelState.loading = true;
  repoPanelState.checking = false;
  repoPanelState.error = '';
  refreshSugarCubesSettingsUi();
  try {
    const { response, data } = await cubeApi.listCubePacks();
    if (!response.ok || data?.error) {
      const message = data?.error?.message || response.statusText || 'Failed to load cube packs';
      repoPanelState.error = message;
      repoPanelState.repos = [];
      repoPanelState.identityPolicy = readIdentityPolicyFromPayload(null);
      refreshSugarCubesSettingsUi();
      return;
    }
    repoPanelState.repos = Array.isArray(data?.repos) ? data.repos : [];
    repoPanelState.identityPolicy = readIdentityPolicyFromPayload(data?.identity_policy);
    repoPanelState.error = '';
    if (checkForUpdates && repoPanelState.repos.length) {
      repoPanelState.checking = true;
      refreshSugarCubesSettingsUi();
      const checkResult = await cubeApi.checkAllCubePacks(
        JSON.stringify({ apply_auto_updates: true }),
        { headers: { 'Content-Type': 'application/json' } },
      );
      if (!checkResult.response.ok || checkResult.data?.error) {
        const message =
          checkResult.data?.error?.message ||
          checkResult.response.statusText ||
          'Failed to check cube packs';
        repoPanelState.error = message;
      } else {
        repoPanelState.repos = Array.isArray(checkResult.data?.repos)
          ? checkResult.data.repos
          : repoPanelState.repos;
        repoPanelState.identityPolicy = readIdentityPolicyFromPayload(
          checkResult.data?.identity_policy || repoPanelState.identityPolicy,
        );
      }
    }
  } catch (error) {
    repoPanelState.error = error?.message || String(error);
    repoPanelState.repos = [];
    repoPanelState.identityPolicy = readIdentityPolicyFromPayload(null);
  } finally {
    repoPanelState.loading = false;
    repoPanelState.checking = false;
    refreshSugarCubesSettingsUi();
  }
}

async function updateIdentityPolicy(payload) {
  const { response, data } = await cubeApi.updateIdentityPolicy(JSON.stringify(payload), {
    headers: { 'Content-Type': 'application/json' },
  });
  if (!response.ok || data?.error) {
    throw new Error(
      data?.error?.message || response.statusText || 'Failed to update identity policy',
    );
  }
  repoPanelState.identityPolicy = readIdentityPolicyFromPayload(data);
  await refreshTrackedRepoPanel({ checkForUpdates: false });
}

async function handleClaimGithubOwner() {
  const currentOwner = repoPanelState.identityPolicy.claimed_github_owner || '';
  const nextOwner = await ui.dialogs?.promptText?.({
    title: 'Claim GitHub Owner',
    message: [
      'Claim exactly one GitHub owner to unlock writes for matching tracked repos.',
      'local/... stays writable either way.',
    ],
    label: 'GitHub owner',
    initialValue: currentOwner,
    placeholder: 'example-user',
    confirmLabel: 'Save Claim',
    normalizeValue: (value) => value.trim(),
  });
  const normalizedOwner = typeof nextOwner === 'string' ? nextOwner.trim() : '';
  if (!normalizedOwner || normalizedOwner === currentOwner) {
    return;
  }
  if (
    normalizeOwnerIdentity(normalizedOwner) === 'artificial-sweetener' &&
    !repoPanelState.identityPolicy.allow_system_owner_claim
  ) {
    throw new Error(
      'Artificial-Sweetener can only be claimed when SUGARCUBES_ALLOW_SYSTEM_OWNER_CLAIM is enabled in .env or the process environment.',
    );
  }
  if (currentOwner) {
    const confirmed = await requestConfirmation({
      title: 'Change Claimed Owner?',
      message: [
        `Change your claimed GitHub owner from ${currentOwner} to ${normalizedOwner}?`,
        'This changes which tracked repos SugarCubes treats as writable.',
      ],
      confirmLabel: 'Change',
    });
    if (!confirmed) {
      return;
    }
  }
  await updateIdentityPolicy({ claimed_github_owner: normalizedOwner });
  pushToastMessage(
    'success',
    'Authoring access updated',
    `Claimed owner is now ${normalizedOwner}.`,
  );
}

async function handleClearGithubOwnerClaim() {
  const currentOwner = repoPanelState.identityPolicy.claimed_github_owner || '';
  if (!currentOwner) {
    return;
  }
  const confirmed = await requestConfirmation({
    title: 'Clear Claimed Owner?',
    message: [
      `Clear your claimed GitHub owner (${currentOwner})?`,
      'Tracked GitHub repos will return to read-only. local/... remains writable.',
    ],
    confirmLabel: 'Clear',
  });
  if (!confirmed) {
    return;
  }
  await updateIdentityPolicy({ claimed_github_owner: '' });
  pushToastMessage(
    'success',
    'Authoring access updated',
    'Tracked GitHub repos are read-only again.',
  );
}

async function handleAddTrackedRepo() {
  const values = await ui.dialogs?.openForm?.({
    title: 'Add Cube Pack',
    message: [
      'Track a shared SugarCube library and keep it available in the browser.',
      'SugarCubes will verify the repository contains .cube files before tracking it.',
    ],
    confirmLabel: 'Add Pack',
    fields: [
      {
        key: 'repoRef',
        label: 'Source repository',
        placeholder: 'Artificial-Sweetener/Base-Cubes',
        initialValue: 'Artificial-Sweetener/Base-Cubes',
        required: true,
        normalizeValue: (value) => value.trim(),
        validate: validateRepoReference,
      },
    ],
  });
  if (!values) {
    return;
  }
  const { owner, repo } = parseRepoReference(values.repoRef);
  const preflightBody = JSON.stringify({
    owner,
    repo,
    enabled: true,
    auto_update: false,
  });
  const { response: preflightResponse, data: preflightData } = await cubeApi.preflightCubePack(
    preflightBody,
    { headers: { 'Content-Type': 'application/json' } },
  );
  if (!preflightResponse.ok || preflightData?.error) {
    throw new Error(
      preflightData?.error?.message || preflightResponse.statusText || 'Failed to verify Cube Pack',
    );
  }
  const { response, data } = await cubeApi.addCubePack(
    JSON.stringify({
      owner,
      repo,
      enabled: true,
      auto_update: false,
    }),
    { headers: { 'Content-Type': 'application/json' } },
  );
  if (!response.ok || data?.error) {
    throw new Error(data?.error?.message || response.statusText || 'Failed to add Cube Pack');
  }
  const cubeCount = Number(data?.preflight?.cube_count ?? preflightData?.preflight?.cube_count);
  const cubeCountText = Number.isFinite(cubeCount)
    ? ` Found ${cubeCount} cube${cubeCount === 1 ? '' : 's'}.`
    : '';
  pushToastMessage(
    'success',
    'Cube Pack added',
    `${owner}/${repo} is now tracked.${cubeCountText}`,
  );
  await refreshTrackedRepoPanel();
  invalidateDependentCatalogs();
}

async function handleSyncTrackedRepo(repo) {
  const { response, data } = await cubeApi.updateCubePackNow(
    JSON.stringify({ owner: repo.owner, repo: repo.repo }),
    { headers: { 'Content-Type': 'application/json' } },
  );
  if (!response.ok || data?.error) {
    throw new Error(data?.error?.message || response.statusText || 'Failed to update Cube Pack');
  }
  pushToastMessage('success', 'Cube Pack updated', `${repo.owner}/${repo.repo} is up to date.`);
  await refreshTrackedRepoPanel();
  invalidateDependentCatalogs();
}

async function handleSyncAllTrackedRepos() {
  const { response, data } = await cubeApi.updateAllCubePacks();
  if (!response.ok || data?.error) {
    throw new Error(data?.error?.message || response.statusText || 'Failed to update Cube Packs');
  }
  pushToastMessage('success', 'Cube Packs updated', 'Available pack updates were applied.');
  await refreshTrackedRepoPanel();
  invalidateDependentCatalogs();
}

async function handleToggleTrackedRepo(repo) {
  const { response, data } = await cubeApi.updateCubePack(
    JSON.stringify({ owner: repo.owner, repo: repo.repo, enabled: !repo.enabled }),
    { headers: { 'Content-Type': 'application/json' } },
  );
  if (!response.ok || data?.error) {
    throw new Error(data?.error?.message || response.statusText || 'Failed to update Cube Pack');
  }
  pushToastMessage(
    'success',
    'Cube Pack updated',
    `${repo.owner}/${repo.repo} ${repo.enabled ? 'disabled' : 'enabled'}.`,
  );
  await refreshTrackedRepoPanel();
  invalidateDependentCatalogs();
}

async function handleSetPackAutoUpdate(repo, enabled) {
  const { response, data } = await cubeApi.updateCubePack(
    JSON.stringify({ owner: repo.owner, repo: repo.repo, auto_update: Boolean(enabled) }),
    { headers: { 'Content-Type': 'application/json' } },
  );
  if (!response.ok || data?.error) {
    throw new Error(data?.error?.message || response.statusText || 'Failed to update Cube Pack');
  }
  pushToastMessage(
    'success',
    'Auto-update updated',
    `${repo.owner}/${repo.repo} auto-update ${enabled ? 'enabled' : 'disabled'}.`,
  );
  await refreshTrackedRepoPanel({ checkForUpdates: false });
}

async function handleRemoveTrackedRepo(repo) {
  const confirmed = await requestConfirmation({
    title: 'Remove Cube Pack?',
    message: [
      `Stop tracking ${repo.owner}/${repo.repo}?`,
      'Local checkout files will remain on disk.',
    ],
    confirmLabel: 'Remove',
  });
  if (!confirmed) {
    return;
  }
  const { response, data } = await cubeApi.removeCubePack({
    owner: repo.owner,
    repo: repo.repo,
  });
  if (!response.ok || data?.error) {
    throw new Error(data?.error?.message || response.statusText || 'Failed to remove Cube Pack');
  }
  pushToastMessage(
    'success',
    'Cube Pack removed',
    `${repo.owner}/${repo.repo} removed from tracking.`,
  );
  await refreshTrackedRepoPanel();
  invalidateDependentCatalogs();
}

function registerSidebarTab() {
  if (sidebarTabRegistered) {
    return;
  }
  const extensionManager = appRef?.extensionManager;
  if (!extensionManager?.registerSidebarTab) {
    logger.warn('SugarCubes: extension manager unavailable; sidebar tab not registered.');
    return;
  }

  extensionManager.registerSidebarTab({
    id: 'sugarcubes',
    title: 'SugarCubes',
    tooltip: 'SugarCubes',
    icon: 'mdi mdi-cube',
    type: 'custom',
    render: (container) => {
      renderSidebarPanel(container);
    },
    destroy: () => {
      if (sidebarRoot && sidebarRoot.parentElement) {
        sidebarRoot.parentElement.removeChild(sidebarRoot);
      }
    },
  });
  sidebarTabRegistered = true;
}

function renderSidebarPanel(container) {
  if (!container) {
    return;
  }
  if (!documentRef) {
    return;
  }

  if (!sidebarRoot) {
    sidebarRoot = documentRef.createElement('div');
    sidebarRoot.className = 'sugarcubes-sidebar-panel';
    Object.assign(sidebarRoot.style, {
      display: 'flex',
      flexDirection: 'column',
      gap: '12px',
      padding: '12px',
      color: 'var(--fg-color, #e8e8e8)',
      fontFamily: 'sans-serif',
    });

    const header = documentRef.createElement('div');
    header.textContent = 'SugarCubes';
    Object.assign(header.style, {
      fontSize: '14px',
      fontWeight: '600',
      letterSpacing: '0.02em',
    });
    sidebarRoot.appendChild(header);

    const tabContent = documentRef.createElement('div');
    tabContent.className = 'sugarcubes-sidebar-panel__content';
    Object.assign(tabContent.style, {
      display: 'flex',
      flexDirection: 'column',
      gap: '12px',
    });
    sidebarRoot.appendChild(tabContent);

    const librarySection = documentRef.createElement('div');
    librarySection.className = 'sugarcubes-sidebar-panel__library';
    Object.assign(librarySection.style, {
      display: 'flex',
      flexDirection: 'column',
      gap: '8px',
    });

    const browserSection = documentRef.createElement('div');
    browserSection.className = 'sugarcubes-sidebar-panel__browser';
    Object.assign(browserSection.style, {
      display: 'flex',
      flexDirection: 'column',
      gap: '8px',
    });
    librarySection.appendChild(browserSection);

    tabContent.appendChild(librarySection);

    ui.cubeBrowser.mountEmbedded(browserSection);
  }

  container.replaceChildren(sidebarRoot);
}

function convertCanvasPoint(canvasInstance, point) {
  if (!canvasInstance || !Array.isArray(point)) {
    return null;
  }
  try {
    if (typeof canvasInstance.convertCanvasToOffset === 'function') {
      const converted = canvasInstance.convertCanvasToOffset(point);
      if (Array.isArray(converted) && converted.length >= 2) {
        const x = Number(converted[0]);
        const y = Number(converted[1]);
        if (Number.isFinite(x) && Number.isFinite(y)) {
          return [x, y];
        }
      }
    }
    const ds = canvasInstance.ds;
    const scale = Number(ds?.scale) || 1;
    const offset = Array.isArray(ds?.offset) ? ds.offset : [0, 0];
    const x = point[0] / scale - offset[0];
    const y = point[1] / scale - offset[1];
    if (Number.isFinite(x) && Number.isFinite(y)) {
      return [x, y];
    }
  } catch (error) {
    logger.warn('SugarCubes -> convertCanvasPoint failed', error);
  }
  return null;
}

function computeDropOrigin() {
  const canvasInstance = appRef.canvas ?? appRef.graph?.canvas ?? null;
  if (!canvasInstance) {
    return [0, 0];
  }

  const lastMouse = canvasInstance.last_mouse_position;
  if (Array.isArray(lastMouse) && Number.isFinite(lastMouse[0]) && Number.isFinite(lastMouse[1])) {
    const converted = convertCanvasPoint(canvasInstance, lastMouse);
    if (converted) {
      return converted;
    }
  }

  try {
    const canvasElement = canvasInstance.canvas ?? null;
    if (canvasElement && typeof canvasElement.getBoundingClientRect === 'function') {
      const rect = canvasElement.getBoundingClientRect();
      const relative = [rect.width / 2, rect.height / 2];
      const converted = convertCanvasPoint(canvasInstance, relative);
      if (converted) {
        return converted;
      }
    }
  } catch (_error) {
    // ignore viewport conversion issues
  }

  const ds = canvasInstance.ds ?? null;
  if (ds) {
    const offset = Array.isArray(ds.offset) ? ds.offset : [0, 0];
    const x = -Number(offset[0] ?? 0);
    const y = -Number(offset[1] ?? 0);
    if (Number.isFinite(x) && Number.isFinite(y)) {
      return [x, y];
    }
  }

  return [0, 0];
}

function buildImportSummary(data) {
  const nodeCount = Array.isArray(data?.nodes) ? data.nodes.length : 0;
  const markerCount = Array.isArray(data?.markers) ? data.markers.length : 0;
  const connectionCount = Array.isArray(data?.connections) ? data.connections.length : 0;
  return `Nodes: ${nodeCount}, markers: ${markerCount}, connections: ${connectionCount}`;
}

function collectCubeIdsFromPayload(payload) {
  const cubeIds = new Set();
  if (!payload || typeof payload !== 'object') {
    return cubeIds;
  }
  const layoutGroups = Array.isArray(payload?.layout?.groups) ? payload.layout.groups : [];
  for (const group of layoutGroups) {
    const cubeId =
      typeof group?.sugarcubes?.cube_id === 'string' ? group.sugarcubes.cube_id.trim() : '';
    if (cubeId) {
      cubeIds.add(cubeId);
    }
  }
  const payloadCubeId =
    typeof payload?.cube?.cube_id === 'string' ? payload.cube.cube_id.trim() : '';
  if (payloadCubeId) {
    cubeIds.add(payloadCubeId);
  }
  return cubeIds;
}

function createRuntimeInstanceId() {
  const cryptoRef =
    (windowRef && typeof windowRef === 'object' ? windowRef.crypto : null) ||
    (typeof globalThis !== 'undefined' ? globalThis.crypto : null);
  if (cryptoRef && typeof cryptoRef.randomUUID === 'function') {
    const generated = cryptoRef.randomUUID();
    if (typeof generated === 'string' && generated.trim()) {
      return generated;
    }
  }
  const time = Date.now().toString(36);
  const rand = Math.random().toString(36).slice(2, 10);
  return `inst_${time}_${rand}`;
}

function readGroupMarkerIds(markers) {
  if (!markers || typeof markers !== 'object') {
    return [];
  }
  return [
    ...(Array.isArray(markers.inputs) ? markers.inputs : []),
    ...(Array.isArray(markers.outputs) ? markers.outputs : []),
  ].map((value) => String(value));
}

function remapPlacementInstanceIds(layout, markers) {
  if (!layout || typeof layout !== 'object') {
    return { layout, markers };
  }
  if (!Array.isArray(layout.groups) || !Array.isArray(markers) || markers.length === 0) {
    return { layout, markers };
  }

  const markerInstanceIds = new Map();
  const oldToNewInstanceIds = new Map();
  const managedGroupInstanceIds = [];
  const groups = layout.groups.map((group) => {
    if (!group || typeof group !== 'object') {
      return group;
    }
    const sugarcubes =
      group.sugarcubes && typeof group.sugarcubes === 'object' ? { ...group.sugarcubes } : null;
    if (!sugarcubes || sugarcubes.managed === false) {
      return group;
    }
    const oldInstanceId =
      typeof sugarcubes.instance_id === 'string' ? sugarcubes.instance_id.trim() : '';
    const nextInstanceId = createRuntimeInstanceId();
    sugarcubes.instance_id = nextInstanceId;
    if (oldInstanceId && !oldToNewInstanceIds.has(oldInstanceId)) {
      oldToNewInstanceIds.set(oldInstanceId, nextInstanceId);
    }
    managedGroupInstanceIds.push(nextInstanceId);
    for (const markerId of readGroupMarkerIds(sugarcubes.markers)) {
      markerInstanceIds.set(markerId, nextInstanceId);
    }
    return { ...group, sugarcubes };
  });

  const defaultInstanceId = managedGroupInstanceIds.length === 1 ? managedGroupInstanceIds[0] : '';
  const remappedMarkers = markers.map((entry) => {
    if (!entry || typeof entry !== 'object') {
      return entry;
    }
    const markerId = entry?.layout?.id ?? entry?.id;
    const markerKey = markerId != null ? String(markerId) : '';
    const widgetValues =
      entry.widget_values && typeof entry.widget_values === 'object'
        ? { ...entry.widget_values }
        : null;

    let nextInstanceId = markerKey ? markerInstanceIds.get(markerKey) : null;
    if (!nextInstanceId && widgetValues) {
      const existingInstanceId =
        typeof widgetValues.instance_id === 'string' ? widgetValues.instance_id.trim() : '';
      if (existingInstanceId && oldToNewInstanceIds.has(existingInstanceId)) {
        nextInstanceId = oldToNewInstanceIds.get(existingInstanceId);
      }
    }
    if (!nextInstanceId && defaultInstanceId) {
      nextInstanceId = defaultInstanceId;
    }
    if (!nextInstanceId) {
      return entry;
    }
    return {
      ...entry,
      widget_values: { ...(widgetValues || {}), instance_id: nextInstanceId },
    };
  });

  return {
    layout: { ...layout, groups },
    markers: remappedMarkers,
  };
}

/**
 * Prepare a SugarCubes import payload for insertion into the live LiteGraph.
 */
function prepareGraphInsertionPayload(
  payload,
  { shift = [0, 0], targetOrigin = null, remapInstanceIds = true } = {},
) {
  if (!payload || typeof payload !== 'object') {
    return null;
  }
  const [shiftX, shiftY] = Array.isArray(shift) ? shift : [0, 0];
  const existingOrigin = Array.isArray(payload?.layout?.origin) ? payload.layout.origin : [0, 0];
  const nextOrigin = readVector2(
    Array.isArray(targetOrigin) ? targetOrigin : existingOrigin,
    existingOrigin[0],
    existingOrigin[1],
  );
  const shiftEntry = (entry) => {
    if (!entry || typeof entry !== 'object') {
      return entry;
    }
    const layout = entry.layout && typeof entry.layout === 'object' ? { ...entry.layout } : null;
    if (layout && Array.isArray(layout.pos)) {
      layout.pos = [Number(layout.pos[0]) + shiftX, Number(layout.pos[1]) + shiftY];
    }
    if (layout && Array.isArray(layout.size)) {
      layout.size = [Number(layout.size[0]), Number(layout.size[1])];
    }
    return { ...entry, layout };
  };
  const nodes = Array.isArray(payload.nodes) ? payload.nodes.map(shiftEntry) : [];
  let markers = Array.isArray(payload.markers) ? payload.markers.map(shiftEntry) : [];
  let layout = payload.layout && typeof payload.layout === 'object' ? { ...payload.layout } : null;
  if (layout) {
    layout = { ...layout, origin: nextOrigin };
    if (Array.isArray(layout.groups)) {
      layout.groups = layout.groups.map((group) => {
        if (!group || typeof group !== 'object') {
          return group;
        }
        const shiftedGroup = { ...group };
        if (group.sugarcubes && typeof group.sugarcubes === 'object') {
          const sugarcubes = { ...group.sugarcubes };
          if (sugarcubes.bounds && typeof sugarcubes.bounds === 'object') {
            const bounds = { ...sugarcubes.bounds };
            const boundX = Number(bounds.x);
            const boundY = Number(bounds.y);
            if (Number.isFinite(boundX)) {
              bounds.x = boundX + shiftX;
            }
            if (Number.isFinite(boundY)) {
              bounds.y = boundY + shiftY;
            }
            sugarcubes.bounds = bounds;
          }
          shiftedGroup.sugarcubes = sugarcubes;
        }
        return shiftedGroup;
      });
    }
    if (remapInstanceIds) {
      const remapped = remapPlacementInstanceIds(layout, markers);
      layout = remapped.layout;
      markers = remapped.markers;
    }
  }
  return {
    ...payload,
    nodes,
    markers,
    layout,
  };
}

/**
 * Shift a placement preview payload into its committed graph location.
 */
function buildShiftedPlacementPayload(payload, shift, targetOrigin) {
  return prepareGraphInsertionPayload(payload, {
    shift,
    targetOrigin,
    remapInstanceIds: true,
  });
}

function reportImportOutcome(defaultAlias, backendWarnings, importResult, payload, options = {}) {
  if (backendWarnings.length) {
    pushToastMessage('warn', 'SugarCube import warnings', backendWarnings.join('\n'));
  }

  const frontendWarnings = Array.isArray(importResult?.warnings)
    ? importResult.warnings.filter(Boolean)
    : [];
  if (Array.isArray(importResult?.missingTypes) && importResult.missingTypes.length) {
    frontendWarnings.push(`Missing node types: ${importResult.missingTypes.join(', ')}`);
  }
  if (importResult?.message && importResult.success) {
    frontendWarnings.push(importResult.message);
  }
  if (frontendWarnings.length) {
    pushToastMessage('warn', 'SugarCube import notes', frontendWarnings.join('\n'));
  }

  const summary = importResult?.summary ?? buildImportSummary(payload);
  if (!importResult?.success) {
    const detail = importResult?.message || summary;
    pushToastMessage('warn', `SugarCube ${defaultAlias} import incomplete`, detail);
    return;
  }

  pushToastMessage('success', `Imported ${defaultAlias}`, summary);
  const shouldFocus = options.focus !== false;
  if (shouldFocus) {
    const graphInstance = appRef.graph;
    if (graphInstance && importResult?.primaryNodeId != null) {
      const focusNode = graphInstance.getNodeById(importResult.primaryNodeId);
      if (focusNode && typeof appRef.canvas?.centerOnNode === 'function') {
        try {
          appRef.canvas.centerOnNode(focusNode);
        } catch (_error) {
          // ignore focus failures
        }
      }
    }
  }
}

/**
 * Index legacy subgraph import hints by wrapper id from the prepared import payload.
 */
function buildImportedSubgraphHintLookup(payload) {
  const lookup = new Map();
  const nodeEntries = Array.isArray(payload?.nodes) ? payload.nodes : [];
  for (const entry of nodeEntries) {
    const classType = typeof entry?.class_type === 'string' ? entry.class_type.trim() : '';
    if (!classType) {
      continue;
    }
    const title =
      (typeof entry?.layout?.title === 'string' && entry.layout.title.trim()) ||
      (typeof entry?.extras?._meta?.title === 'string' && entry.extras._meta.title.trim()) ||
      '';
    const inputs = entry?.inputs && typeof entry.inputs === 'object' ? entry.inputs : {};
    const expectedInputNames = Object.keys(inputs);
    const existing = lookup.get(classType) || {};
    lookup.set(classType, {
      fallbackName: title || existing.fallbackName || '',
      expectedInputNames: expectedInputNames.length
        ? expectedInputNames
        : Array.isArray(existing.expectedInputNames)
          ? existing.expectedInputNames
          : [],
    });
  }
  return lookup;
}

function registerSubgraphs(payload, result) {
  const subgraphs = Array.isArray(payload?.subgraphs) ? payload.subgraphs : [];
  if (!subgraphs.length) {
    return;
  }
  const graph = appRef?.graph;
  if (!graph || typeof graph.createSubgraph !== 'function') {
    result?.warnings?.push('Subgraph registration unavailable; skipping subgraph definitions.');
    return;
  }
  const subgraphMap = graph._subgraphs instanceof Map ? graph._subgraphs : null;
  const hintLookup = buildImportedSubgraphHintLookup(payload);
  for (const entry of subgraphs) {
    if (!entry || typeof entry !== 'object') {
      result?.warnings?.push('Subgraph entry is not an object; skipping.');
      continue;
    }
    const subId = entry.id;
    if (typeof subId !== 'string' || !subId) {
      result?.warnings?.push('Subgraph entry missing id; skipping.');
      continue;
    }
    if (subgraphMap && subgraphMap.has(subId)) {
      continue;
    }
    try {
      const hint = hintLookup.get(subId) || {};
      const normalized = normalizeSubgraphPayload(entry, subId, {
        fallbackName: hint.fallbackName || '',
        expectedInputNames: Array.isArray(hint.expectedInputNames) ? hint.expectedInputNames : [],
      });
      if (!normalized) {
        result?.warnings?.push(`Subgraph '${subId}' could not be normalized; skipping.`);
        continue;
      }
      rebindSubgraphWidgetValues(normalized, (classType) =>
        globalThis.LiteGraph?.createNode?.(classType),
      );
      const subgraph = graph.createSubgraph(normalized);
      if (subgraph && typeof subgraph.configure === 'function') {
        subgraph.configure(normalized);
      }
    } catch (error) {
      const message = error?.message ? String(error.message) : String(error);
      result?.warnings?.push(`Failed to register subgraph '${subId}': ${message}`);
    }
  }
}

/**
 * Record every serialized ID that may identify a created node.
 */
function recordCreatedNodeId(idMap, sourceIds, createdId) {
  if (!idMap || createdId == null) {
    return;
  }
  for (const sourceId of sourceIds) {
    if (sourceId == null) {
      continue;
    }
    const sourceKey = String(sourceId);
    if (sourceKey) {
      idMap.set(sourceKey, createdId);
    }
  }
}

/**
 * Remap an imported metadata ID list to the actual LiteGraph IDs.
 */
function remapImportedIdList(values, idMap) {
  if (!Array.isArray(values)) {
    return [];
  }
  const remapped = [];
  for (const value of values) {
    const mapped = idMap?.get?.(String(value));
    if (mapped != null) {
      remapped.push(mapped);
    }
  }
  return remapped;
}

/**
 * Remap SugarCubes group metadata using created node and marker IDs.
 */
function remapImportedGroupMetadata(metadata, { nodeIdMap, markerIdMap } = {}) {
  if (!metadata || typeof metadata !== 'object') {
    return metadata;
  }
  const next = { ...metadata };
  if (metadata.markers && typeof metadata.markers === 'object') {
    next.markers = {
      ...metadata.markers,
      inputs: remapImportedIdList(metadata.markers.inputs, markerIdMap),
      outputs: remapImportedIdList(metadata.markers.outputs, markerIdMap),
    };
  }
  if (Array.isArray(metadata.nodes)) {
    next.nodes = remapImportedIdList(metadata.nodes, nodeIdMap);
  }
  return next;
}

/**
 * Remap imported layout group metadata before groups are created.
 */
function remapImportedLayoutIds(layout, { nodeIdMap, markerIdMap } = {}) {
  if (!layout || typeof layout !== 'object' || !Array.isArray(layout.groups)) {
    return layout;
  }
  return {
    ...layout,
    groups: layout.groups.map((group) => {
      if (!group || typeof group !== 'object') {
        return group;
      }
      if (!group.sugarcubes || typeof group.sugarcubes !== 'object') {
        return group;
      }
      return {
        ...group,
        sugarcubes: remapImportedGroupMetadata(group.sugarcubes, { nodeIdMap, markerIdMap }),
      };
    }),
  };
}

async function applyPreparedImport(payload, options = {}) {
  const result = {
    success: false,
    summary: '',
    message: '',
    warnings: [],
    missingTypes: [],
    nodesAdded: 0,
    markersAdded: 0,
    connectionsMade: 0,
    primaryNodeId: null,
    bounds: null,
  };

  if (!payload || typeof payload !== 'object') {
    result.message = 'Importer payload missing';
    return result;
  }

  const graph = appRef?.graph;
  if (!graph) {
    result.message = 'Graph unavailable';
    return result;
  }

  const LiteGraphRef = adapter.getLiteGraph?.() || null;
  if (!LiteGraphRef || typeof LiteGraphRef.createNode !== 'function') {
    result.message = 'LiteGraph unavailable';
    return result;
  }

  registerSubgraphs(payload, result);

  const nodeEntries = Array.isArray(payload.nodes) ? payload.nodes : [];
  const markerEntries = Array.isArray(payload.markers) ? payload.markers : [];
  const connectionEntries = Array.isArray(payload.connections) ? payload.connections : [];

  const connectedInputs = new Map();
  for (const connection of connectionEntries) {
    const toSymbol = typeof connection?.to?.symbol === 'string' ? connection.to.symbol : null;
    const inputName = typeof connection?.to?.input === 'string' ? connection.to.input : null;
    if (!toSymbol || !inputName) {
      continue;
    }
    const set = connectedInputs.get(toSymbol) ?? new Set();
    set.add(inputName);
    connectedInputs.set(toSymbol, set);
  }

  const existingIds = new Set();
  const knownNodes = Array.isArray(graph._nodes)
    ? graph._nodes
    : Array.isArray(graph.nodes)
      ? graph.nodes
      : [];
  for (const node of knownNodes) {
    if (node && node.id != null) {
      existingIds.add(node.id);
    }
  }
  const usedIds = new Set(existingIds);

  const createdNodes = new Map();
  const createdMarkers = new Map();
  const nodeIdMap = new Map();
  const markerIdMap = new Map();
  const bounds = { minX: Infinity, minY: Infinity, maxX: -Infinity, maxY: -Infinity };
  const hasBounds = () =>
    Number.isFinite(bounds.minX) &&
    Number.isFinite(bounds.minY) &&
    Number.isFinite(bounds.maxX) &&
    Number.isFinite(bounds.maxY);

  const dropOriginVec = readVector2(
    Array.isArray(options?.dropOrigin) ? options.dropOrigin : payload?.layout?.origin,
    0,
    0,
  );

  let fallbackIndex = 0;
  const nextFallbackPosition = () => {
    const pos = computeGridPosition(dropOriginVec, fallbackIndex);
    fallbackIndex += 1;
    return pos;
  };

  const resolvePosition = (layout) => {
    if (layout && Array.isArray(layout.pos)) {
      return readVector2(layout.pos, dropOriginVec[0], dropOriginVec[1]);
    }
    return nextFallbackPosition();
  };

  if (typeof graph.beforeChange === 'function') {
    graph.beforeChange();
  }

  try {
    for (const entry of nodeEntries) {
      const symbol = typeof entry?.symbol === 'string' ? entry.symbol : null;
      const classType = typeof entry?.class_type === 'string' ? entry.class_type : null;
      if (!symbol || !classType) {
        result.warnings.push('Node entry missing symbol or class_type; skipping.');
        continue;
      }

      const liteNode = LiteGraphRef.createNode(classType);
      if (!liteNode) {
        result.missingTypes.push(classType);
        result.warnings.push(`Node type '${classType}' is unavailable; skipping '${symbol}'.`);
        continue;
      }

      const layout = entry?.layout || {};
      const [posX, posY] = resolvePosition(layout);
      const sizeVec = readVector2(
        layout?.size,
        Array.isArray(liteNode.size) ? liteNode.size[0] : 140,
        Array.isArray(liteNode.size) ? liteNode.size[1] : 60,
      );

      if (Array.isArray(liteNode.pos)) {
        liteNode.pos[0] = posX;
        liteNode.pos[1] = posY;
      } else {
        liteNode.pos = [posX, posY];
      }

      if (Array.isArray(liteNode.size)) {
        if (Number.isFinite(sizeVec[0])) {
          liteNode.size[0] = sizeVec[0];
        }
        if (Number.isFinite(sizeVec[1])) {
          liteNode.size[1] = sizeVec[1];
        }
      } else {
        liteNode.size = [sizeVec[0], sizeVec[1]];
      }

      if (typeof layout?.title === 'string' && layout.title) {
        liteNode.title = layout.title;
      } else if (typeof entry?.extras?._meta?.title === 'string' && entry.extras._meta.title) {
        liteNode.title = entry.extras._meta.title;
      }

      const desiredIdRaw = layout?.id ?? entry?.extras?.original_id;
      const desiredId = Number(desiredIdRaw);
      if (Number.isInteger(desiredId) && !usedIds.has(desiredId)) {
        liteNode.id = desiredId;
        usedIds.add(desiredId);
      } else {
        liteNode.id = -1;
      }

      graph.add(liteNode);
      usedIds.add(liteNode.id);
      recordCreatedNodeId(nodeIdMap, [layout?.id, entry?.extras?.original_id], liteNode.id);

      if (!liteNode.properties || typeof liteNode.properties !== 'object') {
        liteNode.properties = {};
      }
      liteNode.properties.sugarcubes_symbol = symbol;

      applyLayoutPresentation(liteNode, layout, result);
      applyExecutionMode(liteNode, entry?.mode ?? entry?.extras?.mode);

      createdNodes.set(symbol, liteNode);
      updateBoundsWithNode(bounds, liteNode);
      result.nodesAdded += 1;
      if (result.primaryNodeId == null) {
        result.primaryNodeId = liteNode.id;
      }

      const inputs = entry?.inputs && typeof entry.inputs === 'object' ? entry.inputs : {};
      const linkedInputs = connectedInputs.get(symbol);
      for (const [inputName, inputValue] of Object.entries(inputs)) {
        if (linkedInputs?.has(inputName)) {
          continue;
        }
        applyInputValueToNode(liteNode, inputName, inputValue);
      }

      const extras = entry?.extras;
      if (extras && typeof extras === 'object') {
        applyExtrasToNode(liteNode, extras);
      }
    }

    for (const entry of markerEntries) {
      const alias = typeof entry?.alias === 'string' ? entry.alias : null;
      const classType = typeof entry?.class_type === 'string' ? entry.class_type : null;
      if (!alias || !classType) {
        result.warnings.push('Marker entry missing alias or class_type; skipping.');
        continue;
      }

      const markerNode = LiteGraphRef.createNode(classType);
      if (!markerNode) {
        result.missingTypes.push(classType);
        result.warnings.push(`Marker type '${classType}' is unavailable; skipping '${alias}'.`);
        continue;
      }

      const layout = entry?.layout || {};
      const [posX, posY] = resolvePosition(layout);
      const sizeVec = readVector2(
        layout?.size,
        Array.isArray(markerNode.size) ? markerNode.size[0] : 120,
        Array.isArray(markerNode.size) ? markerNode.size[1] : 40,
      );

      if (Array.isArray(markerNode.pos)) {
        markerNode.pos[0] = posX;
        markerNode.pos[1] = posY;
      } else {
        markerNode.pos = [posX, posY];
      }

      if (Array.isArray(markerNode.size)) {
        if (Number.isFinite(sizeVec[0])) {
          markerNode.size[0] = sizeVec[0];
        }
        if (Number.isFinite(sizeVec[1])) {
          markerNode.size[1] = sizeVec[1];
        }
      } else {
        markerNode.size = [sizeVec[0], sizeVec[1]];
      }

      if (typeof layout?.title === 'string' && layout.title) {
        markerNode.title = layout.title;
      }

      const desiredId = Number(layout?.id);
      if (Number.isInteger(desiredId) && !usedIds.has(desiredId)) {
        markerNode.id = desiredId;
        usedIds.add(desiredId);
      } else {
        markerNode.id = -1;
      }

      graph.add(markerNode);
      usedIds.add(markerNode.id);
      recordCreatedNodeId(markerIdMap, [layout?.id, entry?.id], markerNode.id);

      if (!markerNode.properties || typeof markerNode.properties !== 'object') {
        markerNode.properties = {};
      }
      markerNode.properties.sugarcubes_symbol = alias;
      const versionIdentity = readPreparedCubeIdentity(payload);
      if (versionIdentity.cubeVersion) {
        markerNode.properties.sugarcubes_cube_version = versionIdentity.cubeVersion;
      }
      if (versionIdentity.revisionRef) {
        markerNode.properties.sugarcubes_cube_revision_ref = versionIdentity.revisionRef;
      }

      applyLayoutPresentation(markerNode, layout, result);

      const widgetValues =
        entry?.widget_values && typeof entry.widget_values === 'object' ? entry.widget_values : {};
      for (const [widgetName, widgetValue] of Object.entries(widgetValues)) {
        writeWidgetValue(markerNode, widgetName, widgetValue);
      }

      createdMarkers.set(alias, markerNode);
      updateBoundsWithNode(bounds, markerNode);
      result.markersAdded += 1;
      if (result.primaryNodeId == null) {
        result.primaryNodeId = markerNode.id;
      }
    }

    const resolveCreatedNode = (symbol) => createdNodes.get(symbol) ?? createdMarkers.get(symbol);

    for (const connection of connectionEntries) {
      const fromSymbol =
        typeof connection?.from?.symbol === 'string' ? connection.from.symbol : null;
      const toSymbol = typeof connection?.to?.symbol === 'string' ? connection.to.symbol : null;
      const inputName = typeof connection?.to?.input === 'string' ? connection.to.input : null;
      if (!fromSymbol || !toSymbol || !inputName) {
        continue;
      }

      const fromNode = resolveCreatedNode(fromSymbol);
      const toNode = resolveCreatedNode(toSymbol);
      if (!fromNode || !toNode) {
        result.warnings.push(
          `Skipping connection '${fromSymbol}' -> '${toSymbol}.${inputName}' (node missing).`,
        );
        continue;
      }

      if (!Array.isArray(fromNode.outputs) || fromNode.outputs.length === 0) {
        result.warnings.push(
          `Skipping connection '${fromSymbol}' -> '${toSymbol}.${inputName}' (no outputs).`,
        );
        continue;
      }

      let slotIndex = resolveOutputSlotIndex(fromNode, connection?.from?.slot);
      if (!Number.isInteger(slotIndex)) {
        slotIndex = 0;
      }
      if (slotIndex < 0) {
        slotIndex = 0;
      }
      if (slotIndex >= fromNode.outputs.length) {
        slotIndex = fromNode.outputs.length - 1;
      }

      if (!ensureInputSlot(toNode, inputName)) {
        result.warnings.push(
          `Skipping connection '${fromSymbol}' -> '${toSymbol}.${inputName}' (input unavailable).`,
        );
        continue;
      }

      const inputIndex = resolveInputSlotIndex(toNode, inputName);
      if (inputIndex === -1) {
        result.warnings.push(
          `Skipping connection '${fromSymbol}' -> '${toSymbol}.${inputName}' (input unresolved).`,
        );
        continue;
      }

      try {
        fromNode.connect(slotIndex, toNode, inputIndex);
        result.connectionsMade += 1;
      } catch (error) {
        const message = error?.message ? String(error.message) : String(error);
        result.warnings.push(
          `Failed to connect '${fromSymbol}' -> '${toSymbol}.${inputName}': ${message}`,
        );
      }
    }
  } catch (error) {
    const message = error?.message ? String(error.message) : String(error);
    result.message = message;
    result.warnings.push(`Importer error: ${message}`);
  } finally {
    if (typeof graph.afterChange === 'function') {
      graph.afterChange();
    }
  }

  if (typeof appRef.graph?.setDirtyCanvas === 'function') {
    appRef.graph.setDirtyCanvas(true, true);
  }
  if (typeof appRef.canvas?.setDirty === 'function') {
    appRef.canvas.setDirty(true, true);
  }

  if (hasBounds()) {
    result.bounds = bounds;
  }

  const remappedLayout = remapImportedLayoutIds(payload?.layout, { nodeIdMap, markerIdMap });
  recreateLayoutGroups(remappedLayout, {
    instanceAlias: options?.instanceAlias,
    dropOrigin: dropOriginVec,
    graph,
    bounds: result.bounds,
    cube: payload?.cube,
    revision: payload?.revision,
  });
  result.summary = `nodes ${result.nodesAdded}, markers ${result.markersAdded}, links ${result.connectionsMade}`;
  result.success = result.nodesAdded + result.markersAdded > 0;
  ui.instanceManager.refresh({ graph, reason: 'import', force: true });
  ui.instanceManager.scheduleRefresh({ graph, reason: 'import', force: true });
  if (result.success) {
    const cubeIds = Array.from(collectCubeIdsFromPayload(payload));
    if (cubeIds.length) {
      ui.dirtyManager.markLocalBaseline({ graph, cubeIds });
    }
  }
  ui.dirtyManager.requestRefresh({ graph, reason: 'import' });

  if (result.missingTypes.length) {
    result.missingTypes = Array.from(new Set(result.missingTypes));
  }

  if (!result.success && !result.message) {
    result.message = 'No nodes were created';
  }

  return result;
}

function recreateLayoutGroups(layout, context = {}) {
  if (!layout || typeof layout !== 'object') {
    return;
  }
  const graph = context.graph || appRef.graph;
  if (!graph || typeof graph.add !== 'function') {
    return;
  }

  const dropOrigin = readVector2(
    Array.isArray(context.dropOrigin) ? context.dropOrigin : [0, 0],
    0,
    0,
  );
  const baseOrigin = readVector2(layout.origin, dropOrigin[0], dropOrigin[1]);
  const groups = Array.isArray(layout.groups) ? layout.groups : [];

  const createGroup = (groupPayload, options = {}) => {
    const sugarcubesPayload =
      groupPayload?.sugarcubes && typeof groupPayload.sugarcubes === 'object'
        ? groupPayload.sugarcubes
        : null;
    const cubeMetadata =
      context.cube?.metadata && typeof context.cube.metadata === 'object'
        ? context.cube.metadata
        : {};
    const cubeIcon =
      (context.cube?.icon && typeof context.cube.icon === 'object' && context.cube.icon) ||
      (cubeMetadata.icon && typeof cubeMetadata.icon === 'object' && cubeMetadata.icon) ||
      null;
    const canonicalDefaultAlias =
      (typeof context.cube?.default_alias === 'string' && context.cube.default_alias.trim()) ||
      (typeof cubeMetadata.default_alias === 'string' && cubeMetadata.default_alias.trim()) ||
      '';
    const instanceAliasSeed =
      (typeof context.instanceAlias === 'string' && context.instanceAlias.trim()) ||
      canonicalDefaultAlias ||
      (typeof sugarcubesPayload?.default_alias === 'string' &&
        sugarcubesPayload.default_alias.trim()) ||
      (typeof context.cube?.cube_id === 'string' && context.cube.cube_id.trim()) ||
      '';
    const title =
      typeof options.title === 'string' && options.title
        ? options.title
        : instanceAliasSeed
          ? instanceAliasSeed
          : 'SugarCube';
    const liteGraph = adapter.getLiteGraph?.() || null;
    if (!liteGraph?.LGraphGroup) {
      return null;
    }
    const group = new liteGraph.LGraphGroup(title);
    graph.add(group);

    const bounding = Array.isArray(groupPayload?.bounding) ? groupPayload.bounding : null;
    if (bounding && bounding.length === 4) {
      const [bx, by, bw, bh] = bounding.map((value) => Number(value) || 0);
      group.pos = [baseOrigin[0] + bx, baseOrigin[1] + by];
      group.size = [bw, bh];
    } else if (context.bounds) {
      const { minX, minY, maxX, maxY } = context.bounds;
      group.pos = [minX, minY];
      group.size = [maxX - minX, maxY - minY];
    } else {
      group.pos = [baseOrigin[0], baseOrigin[1]];
      group.size = [640, 480];
    }

    if (groupPayload?.color) {
      group.color = groupPayload.color;
    }
    if (groupPayload?.bgcolor) {
      group.bgcolor = groupPayload.bgcolor;
    }
    if (groupPayload?.font_size) {
      group.font_size = groupPayload.font_size;
    }
    if (groupPayload?.flags && typeof groupPayload.flags === 'object') {
      group.flags = { ...group.flags, ...groupPayload.flags };
    }

    if (sugarcubesPayload) {
      let next = applyCubeDefinitionIdentity(
        { ...sugarcubesPayload },
        {
          cubeId: context.cube?.cube_id,
          version: context.cube?.version,
          revisionRef: context.revision?.revision_ref,
        },
      );
      if (context.cube?.target_model) {
        next.target_model = context.cube.target_model;
      }
      if (canonicalDefaultAlias) {
        next.default_alias = canonicalDefaultAlias;
      }
      if (cubeIcon) {
        next.icon = cubeIcon;
      }
      const flavorMetadata = ui.flavorService?.buildImportedMetadata?.(context.cube) || null;
      if (flavorMetadata) {
        Object.assign(next, flavorMetadata);
      }
      if (!next.schema) {
        next.schema = CUBE_INSTANCE_SCHEMA;
      }
      if (instanceAliasSeed) {
        next.instance_alias = instanceAliasSeed;
      } else {
        delete next.instance_alias;
      }
      const normalized = normalizeGroupInstanceAlias(group, next, instanceAliasSeed);
      if (normalized?.metadata) {
        setGroupSugarcubes(group, normalized.metadata);
        Object.defineProperty(group, '__sugarcubes_imported', {
          value: true,
          writable: true,
          configurable: true,
          enumerable: false,
        });
      }
    }

    return group;
  };

  if (groups.length) {
    for (const groupPayload of groups) {
      if (!groupPayload || typeof groupPayload !== 'object') {
        continue;
      }
      createGroup(groupPayload);
    }
    return;
  }

  const bounds = context.bounds || null;
  const syntheticPayload = {
    title: context.instanceAlias || layout?.cube?.name || 'SugarCube',
    bounding: bounds
      ? [bounds.minX, bounds.minY, bounds.maxX - bounds.minX, bounds.maxY - bounds.minY]
      : null,
    color: '#3f789e',
    bgcolor: '#3f5159',
    sugarcubes: {
      managed: true,
      cube_id: context.cube?.cube_id || '',
      default_alias:
        (typeof context.cube?.metadata?.default_alias === 'string' &&
          context.cube.metadata.default_alias.trim()) ||
        context.cube?.default_alias ||
        context.instanceAlias ||
        context.cube?.cube_id ||
        '',
      target_model: context.cube?.target_model || '',
    },
  };
  createGroup(syntheticPayload, { title: syntheticPayload.title });
}

function readPreparedCubeIdentity(payload) {
  const cube = payload?.cube && typeof payload.cube === 'object' ? payload.cube : {};
  const revision =
    payload?.revision && typeof payload.revision === 'object' ? payload.revision : {};
  return {
    cubeId: typeof cube.cube_id === 'string' ? cube.cube_id.trim() : '',
    cubeVersion: typeof cube.version === 'string' ? cube.version.trim() : '',
    revisionRef:
      typeof revision.revision_ref === 'string' && revision.revision_ref.trim()
        ? revision.revision_ref.trim()
        : 'WORKTREE',
  };
}

function applyLayoutPresentation(node, layout, result) {
  if (!node || !layout) {
    return;
  }
  const flags = readLayoutFlags(layout);
  if (flags && flags.collapsed === true && typeof node.collapse === 'function') {
    const alreadyCollapsed = Boolean(node.flags?.collapsed);
    if (!alreadyCollapsed) {
      try {
        node.collapse(true);
      } catch (error) {
        const message = error?.message ? String(error.message) : String(error);
        const label = node.title || node.name || node.type || `node-${node.id ?? '?'}`;
        result?.warnings?.push?.(`Failed to collapse '${label}': ${message}`);
      }
    }
  }

  const style = readLayoutStyle(layout);
  if (style) {
    if (typeof style.color === 'string' && style.color) {
      node.color = style.color;
    }
    if (typeof style.bgcolor === 'string' && style.bgcolor) {
      node.bgcolor = style.bgcolor;
    }
    if (style.shape !== undefined) {
      node.shape = style.shape;
    }
  }
}

function computeGridPosition(origin, index) {
  const base = Array.isArray(origin) ? origin : [0, 0];
  const baseX = Number(base[0]) || 0;
  const baseY = Number(base[1]) || 0;
  const columns = 3;
  const spacingX = 320;
  const spacingY = 240;
  const column = index % columns;
  const row = Math.floor(index / columns);
  return [baseX + column * spacingX, baseY + row * spacingY];
}

function updateBoundsWithNode(bounds, node) {
  if (!node) {
    return;
  }
  const pos = Array.isArray(node.pos) ? node.pos : [0, 0];
  const size = Array.isArray(node.size) ? node.size : [140, 60];
  const x = Number(pos[0]) || 0;
  const y = Number(pos[1]) || 0;
  const width = Number(size[0]) || 140;
  const height = Number(size[1]) || 60;
  bounds.minX = Math.min(bounds.minX, x);
  bounds.minY = Math.min(bounds.minY, y);
  bounds.maxX = Math.max(bounds.maxX, x + width);
  bounds.maxY = Math.max(bounds.maxY, y + height);
}

function resolveOutputSlotIndex(node, slotSpec) {
  if (typeof slotSpec === 'number' && Number.isFinite(slotSpec)) {
    return slotSpec;
  }
  if (typeof slotSpec === 'string' && slotSpec) {
    const outputs = Array.isArray(node?.outputs) ? node.outputs : [];
    const byName = outputs.findIndex((output) => output && output.name === slotSpec);
    if (byName !== -1) {
      return byName;
    }
    const parsed = Number(slotSpec);
    if (Number.isFinite(parsed)) {
      return parsed;
    }
  }
  return 0;
}

function resolveInputSlotIndex(node, inputName) {
  if (!node || !Array.isArray(node.inputs) || typeof inputName !== 'string') {
    return -1;
  }
  return node.inputs.findIndex((input) => input && input.name === inputName);
}

function ensureInputSlot(node, inputName) {
  if (!node || typeof inputName !== 'string') {
    return false;
  }
  const existing = resolveInputSlotIndex(node, inputName);
  if (existing !== -1) {
    return true;
  }
  const widget = Array.isArray(node.widgets)
    ? node.widgets.find((entry) => entry && entry.name === inputName)
    : null;
  if (widget && typeof node.convertWidgetToInput === 'function') {
    try {
      const converted = node.convertWidgetToInput(widget);
      if (converted !== false) {
        return true;
      }
    } catch (_error) {
      // ignore conversion failures
    }
    return resolveInputSlotIndex(node, inputName) !== -1;
  }
  return false;
}

function applyInputValueToNode(node, inputName, value) {
  if (!node || typeof inputName !== 'string') {
    return false;
  }
  if (writeWidgetValue(node, inputName, value)) {
    return true;
  }
  if (typeof node.setProperty === 'function') {
    try {
      node.setProperty(inputName, value);
      return true;
    } catch (_error) {
      // ignore setProperty failures
    }
  }
  if (!node.properties || typeof node.properties !== 'object') {
    node.properties = {};
  }
  if (Object.prototype.hasOwnProperty.call(node.properties, inputName)) {
    node.properties[inputName] = value;
    if (typeof node.onPropertyChanged === 'function') {
      try {
        node.onPropertyChanged(inputName, value);
      } catch (_error) {
        // ignore property change failures
      }
    }
    return true;
  }
  return false;
}

function applyExtrasToNode(node, extras) {
  if (!node || !extras || typeof extras !== 'object') {
    return;
  }
  if (Array.isArray(extras.widgets_values) && Array.isArray(node.widgets)) {
    for (let i = 0; i < node.widgets.length && i < extras.widgets_values.length; i += 1) {
      const widget = node.widgets[i];
      if (!widget) {
        continue;
      }
      const value = extras.widgets_values[i];
      widget.value = value;
      if (typeof widget.callback === 'function') {
        try {
          widget.callback(value);
        } catch (_error) {
          // ignore widget callback failures
        }
      }
    }
  }
  if (extras.properties && typeof extras.properties === 'object') {
    for (const [key, propertyValue] of Object.entries(extras.properties)) {
      if (typeof node.setProperty === 'function') {
        try {
          node.setProperty(key, propertyValue);
          continue;
        } catch (_error) {
          // ignore setProperty failures
        }
      }
      if (!node.properties || typeof node.properties !== 'object') {
        node.properties = {};
      }
      node.properties[key] = propertyValue;
      if (typeof node.onPropertyChanged === 'function') {
        try {
          node.onPropertyChanged(key, propertyValue);
        } catch (_error) {
          // ignore property change failures
        }
      }
    }
  }
}

function applyExecutionMode(node, value) {
  if (!node) {
    return;
  }
  if (!Number.isInteger(value) || value <= 0) {
    return;
  }
  node.mode = value;
}

async function importCubeById(rawCubeId, options = {}) {
  const cubeId = typeof rawCubeId === 'string' ? rawCubeId.trim() : '';
  if (!cubeId) {
    return { success: false, reason: 'empty' };
  }

  persistLastCubeId(cubeId);

  const dropOrigin =
    Array.isArray(options.dropOrigin) && options.dropOrigin.length === 2
      ? options.dropOrigin
      : computeDropOrigin();
  const originPayload = { x: dropOrigin[0], y: dropOrigin[1] };

  const setBusy =
    typeof options.setBusy === 'function'
      ? options.setBusy
      : (busy) => {
          if (options.button) {
            options.button.enabled = !busy;
            options.button.element.classList.toggle('sugarcubes-import--busy', Boolean(busy));
          }
        };

  setBusy(true);
  try {
    const { response, data } = await cubeApi.load(
      JSON.stringify({ cube_id: cubeId, origin: originPayload }),
      { headers: { 'Content-Type': 'application/json' } },
    );
    if (!response.ok || data?.error) {
      const errorPayload = data?.error || {};
      const message = errorPayload.message || response.statusText || 'Import failed';
      const detail =
        typeof errorPayload.detail === 'string' && errorPayload.detail ? errorPayload.detail : '';
      pushToastMessage('error', message, detail);
      return {
        success: false,
        reason: 'error',
        message,
        detail,
        status: response.status,
        payload: data,
      };
    }

    const preparedData =
      prepareGraphInsertionPayload(data, {
        targetOrigin: Array.isArray(data?.layout?.origin) ? data.layout.origin : dropOrigin,
        remapInstanceIds: true,
      }) || data;
    const importResult = await applyPreparedImport(preparedData, {
      instanceAlias:
        preparedData?.cube?.default_alias || preparedData?.cube?.display_name || cubeId,
      dropOrigin,
    });

    const backendWarnings = Array.isArray(data?.warnings) ? data.warnings.filter(Boolean) : [];
    if (backendWarnings.length) {
      pushToastMessage('warn', 'SugarCube import warnings', backendWarnings.join('\n'));
    }

    const frontendWarnings = Array.isArray(importResult?.warnings)
      ? importResult.warnings.filter(Boolean)
      : [];
    if (Array.isArray(importResult?.missingTypes) && importResult.missingTypes.length) {
      frontendWarnings.push(`Missing node types: ${importResult.missingTypes.join(', ')}`);
    }
    if (importResult?.message && importResult.success) {
      frontendWarnings.push(importResult.message);
    }
    if (frontendWarnings.length) {
      pushToastMessage('warn', 'SugarCube import notes', frontendWarnings.join('\n'));
    }

    const summary = importResult?.summary ?? buildImportSummary(preparedData);
    if (!importResult?.success) {
      const detail = importResult?.message || summary;
      pushToastMessage('warn', `SugarCube ${cubeId} import incomplete`, detail);
    } else {
      pushToastMessage('success', `Imported ${cubeId}`, summary);

      const graphInstance = appRef?.graph;
      if (graphInstance && importResult?.primaryNodeId != null) {
        const focusNode = graphInstance.getNodeById(importResult.primaryNodeId);
        if (focusNode && typeof appRef?.canvas?.centerOnNode === 'function') {
          try {
            appRef.canvas.centerOnNode(focusNode);
          } catch (_error) {
            // ignore focus failures
          }
        }
      }
    }

    return {
      success: Boolean(importResult?.success),
      cubeId,
      summary,
      backendWarnings,
      frontendWarnings,
      response: data,
      result: importResult,
    };
  } catch (error) {
    const message = error && error.message ? error.message : String(error);
    pushToastMessage('error', 'Import failed', message);
    return { success: false, reason: 'exception', message, error };
  } finally {
    setBusy(false);
  }
}

async function importCubeRevision(rawCubeId, rawRevisionRef, options = {}) {
  const cubeId = typeof rawCubeId === 'string' ? rawCubeId.trim() : '';
  const revisionRef =
    typeof rawRevisionRef === 'string' && rawRevisionRef.trim() ? rawRevisionRef.trim() : '';
  if (!cubeId || !revisionRef) {
    return { success: false, reason: 'empty' };
  }
  if (revisionRef === 'WORKTREE') {
    return importCubeById(cubeId, options);
  }

  persistLastCubeId(cubeId);

  const dropOrigin =
    Array.isArray(options.dropOrigin) && options.dropOrigin.length === 2
      ? options.dropOrigin
      : computeDropOrigin();
  const originPayload = { x: dropOrigin[0], y: dropOrigin[1] };

  const setBusy =
    typeof options.setBusy === 'function'
      ? options.setBusy
      : (busy) => {
          if (options.button) {
            options.button.enabled = !busy;
            options.button.element.classList.toggle('sugarcubes-import--busy', Boolean(busy));
          }
        };

  setBusy(true);
  try {
    const { response, data } = await cubeApi.loadRevision(
      JSON.stringify({
        cube_id: cubeId,
        revision_ref: revisionRef,
        origin: originPayload,
      }),
      { headers: { 'Content-Type': 'application/json' } },
    );
    if (!response.ok || data?.error) {
      const errorPayload = data?.error || {};
      const message = errorPayload.message || response.statusText || 'Revision import failed';
      const detail =
        typeof errorPayload.detail === 'string' && errorPayload.detail ? errorPayload.detail : '';
      pushToastMessage('error', message, detail);
      return {
        success: false,
        reason: 'error',
        message,
        detail,
        status: response.status,
        payload: data,
      };
    }

    const preparedData =
      prepareGraphInsertionPayload(data, {
        targetOrigin: Array.isArray(data?.layout?.origin) ? data.layout.origin : dropOrigin,
        remapInstanceIds: true,
      }) || data;
    const importResult = await applyPreparedImport(preparedData, {
      instanceAlias:
        preparedData?.cube?.default_alias || preparedData?.cube?.display_name || cubeId,
      dropOrigin,
    });

    const backendWarnings = Array.isArray(data?.warnings) ? data.warnings.filter(Boolean) : [];
    if (backendWarnings.length) {
      pushToastMessage('warn', 'SugarCube revision import warnings', backendWarnings.join('\n'));
    }

    const frontendWarnings = Array.isArray(importResult?.warnings)
      ? importResult.warnings.filter(Boolean)
      : [];
    if (Array.isArray(importResult?.missingTypes) && importResult.missingTypes.length) {
      frontendWarnings.push(`Missing node types: ${importResult.missingTypes.join(', ')}`);
    }
    if (importResult?.message && importResult.success) {
      frontendWarnings.push(importResult.message);
    }
    if (frontendWarnings.length) {
      pushToastMessage('warn', 'SugarCube revision import notes', frontendWarnings.join('\n'));
    }

    const summary = importResult?.summary ?? buildImportSummary(preparedData);
    if (!importResult?.success) {
      const detail = importResult?.message || summary;
      pushToastMessage('warn', `SugarCube ${cubeId} revision import incomplete`, detail);
    } else {
      pushToastMessage('success', `Imported ${cubeId} revision`, summary);

      const graphInstance = appRef?.graph;
      if (graphInstance && importResult?.primaryNodeId != null) {
        const focusNode = graphInstance.getNodeById(importResult.primaryNodeId);
        if (focusNode && typeof appRef?.canvas?.centerOnNode === 'function') {
          try {
            appRef.canvas.centerOnNode(focusNode);
          } catch (_error) {
            // ignore focus failures
          }
        }
      }
    }

    return {
      success: Boolean(importResult?.success),
      cubeId,
      revisionRef,
      summary,
      backendWarnings,
      frontendWarnings,
      response: data,
      result: importResult,
    };
  } catch (error) {
    const message = error && error.message ? error.message : String(error);
    pushToastMessage('error', 'Revision import failed', message);
    return { success: false, reason: 'exception', message, error };
  } finally {
    setBusy(false);
  }
}

ui.cubeBrowser.configure({
  actions: {
    computeDropOrigin,
    importCubeByName: importCubeById,
    importCubeRevision,
    onCubesUpdated: (cubes) => ui.dirtyManager.updateKnownCubes(cubes),
    openConfirmDialog: (options) => ui.confirmDialog.open(options),
    promoteCube: (cube) => ui.promotionService.promote(cube),
    reconcileCubeIdentity: (identity) => ui.identityReconciler.reconcile(identity),
    startCubePlacement: (cubeId, options) => overlayManager.placement.start(cubeId, options),
  },
  helpers: {
    coerceVec2,
    computePayloadBounds: (entries, ctx) =>
      computePayloadBounds(entries, ctx, adapter.getLiteGraph?.()),
    drawGhostRect,
    getPlacementGroupLabel: (defaultAlias, group) =>
      getPlacementGroupLabel(defaultAlias, group, getGroupSugarcubes),
    readVector2,
    resolvePreviewRect: (entry, pos, size, ctx) =>
      resolvePreviewRect(entry, pos, size, ctx, adapter.getLiteGraph?.()),
  },
  placement: {
    commit: () => overlayManager.placement.commit(),
    computeOriginFromEvent: (event) => overlayManager.placement.computeOriginFromEvent(event),
    getState: () => overlayManager.placement.getState(),
    isPointerOverCanvas: (event) => overlayManager.placement.isPointerOverCanvas(event),
    setCommitInProgress: (value) => overlayManager.placement.setCommitInProgress(value),
    setDirty: () => overlayManager.placement.setDirty(),
    setOrigin: (origin) => overlayManager.placement.setOrigin(origin),
    start: (cubeId, options) => overlayManager.placement.start(cubeId, options),
    stop: (reason) => overlayManager.placement.stop(reason),
  },
});

appRef.registerExtension({
  name: EXTENSION_NAME,
  async setup() {
    try {
      registerSidebarTab();
      registerSugarCubesSettings();
      await ui.setup();
      await refreshTrackedRepoPanel({ checkForUpdates: false });
      overlayManager.proximity.refreshOverlayState({ recompute: true, graph: appRef.graph });
      refreshSugarCubesSettingsUi();
      ui.instanceManager.scheduleRefresh({ graph: appRef.graph, reason: 'setup' });
      ui.dirtyManager.requestRefresh({ graph: appRef.graph, reason: 'setup' });
    } catch (error) {
      logger.error('SugarCubes: setup failed', error);
      throw error;
    }
  },
  beforeConfigureGraph() {
    overlayManager.proximity.resetOverlayState();
  },
  afterConfigureGraph(_missingNodeTypes, comfyApp) {
    const graph = comfyApp?.graph ?? appRef.graph;
    overlayManager.proximity.refreshOverlayState({ recompute: true, graph });
    ui.instanceManager.scheduleRefresh({ graph, reason: 'configure' });
    ui.dirtyManager.requestRefresh({ graph, reason: 'configure' });
  },
});

windowRef.SugarCubes = createPublicApi(ui);
windowRef.SugarCubesDebug = {
  getDirtyState(instanceId) {
    if (!instanceId) {
      return null;
    }
    const entry = ui?.dirtyManager?.tracker?.instances?.get(instanceId) || null;
    if (!entry) {
      return null;
    }
    const baselineStore = ui?.dirtyManager?.baselineStore || null;
    const definitionHash = baselineStore?.getDefinitionHash(entry.cubeId) || null;
    const localBaselineHash = baselineStore?.getLocalBaselineHash(instanceId) || null;
    const baselineSource =
      localBaselineHash && entry.baselineHash === localBaselineHash
        ? 'local'
        : definitionHash && entry.baselineHash === definitionHash
          ? 'definition'
          : null;
    return {
      instanceId,
      cubeId: entry.cubeId || null,
      baselineHash: entry.baselineHash || null,
      currentHash: entry.currentHash || null,
      baselineSource,
      reasons: Array.isArray(entry.reasons) ? entry.reasons : [],
      dirty: Boolean(entry.dirty),
      dirtyAt: entry.dirtyAt || null,
      initializedAt: entry.initializedAt || null,
    };
  },
  bounds: {
    get(instanceId) {
      if (!instanceId) {
        return null;
      }
      const graph = appRef?.graph || null;
      const index = ui.containmentService?.buildIndex?.(graph) || null;
      const entry = index?.instanceById?.get?.(String(instanceId)) || null;
      if (!entry?.metadata?.bounds) {
        return null;
      }
      return {
        bounds: entry.metadata.bounds,
        inner: computeInnerBounds(entry.metadata.bounds),
      };
    },
    reconcile(instanceId) {
      const graph = appRef?.graph || null;
      if (!graph || !ui.boundsReconciler) {
        return { changed: [] };
      }
      const result = ui.boundsReconciler.reconcileAll({ graph });
      const changed = Array.from(result.changed || []);
      if (instanceId && !changed.includes(String(instanceId))) {
        return { changed: [] };
      }
      return { changed };
    },
    resolveCollisions(instanceId) {
      const graph = appRef?.graph || null;
      if (!graph || !ui.collisionService || !instanceId) {
        return { moved: false };
      }
      const index = ui.containmentService?.buildIndex?.(graph) || null;
      return ui.collisionService.resolveCollisions({
        graph,
        activeInstanceId: instanceId,
        index,
      });
    },
  },
  layout: {
    service: ui.layoutService,
    appendCube: (options) => ui.layoutService?.appendCube?.(options),
    insertBetween: (options) => ui.layoutService?.insertBetween?.(options),
    insertBefore: (options) => ui.layoutService?.insertBefore?.(options),
    swapOrder: (options) => ui.layoutService?.swapOrder?.(options),
    replaceCube: (options) => ui.layoutService?.replaceCube?.(options),
  },
};
