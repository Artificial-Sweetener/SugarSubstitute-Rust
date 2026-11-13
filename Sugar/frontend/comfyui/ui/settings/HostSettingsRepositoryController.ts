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
/** Own identity-policy and tracked-pack commands for host settings. */

import { isRecord } from '../types/common.js';
import type { ApiJsonResult } from '../core/CubeLibraryApi.js';
import {
  readIdentityPolicy,
  type HostSettingsCommandDependencies,
  type PackApiPayload,
  type TrackedRepo,
} from './HostSettingsContracts.js';
import type { UnknownRecord } from '../types/common.js';

/** Execute remote settings commands and maintain repository state. */
export class HostSettingsRepositoryController {
  constructor(private readonly dependencies: HostSettingsCommandDependencies) {}

  /** Refresh tracked repositories and optionally apply automatic updates. */
  async refresh({ checkForUpdates = true }: { checkForUpdates?: boolean } = {}): Promise<void> {
    const { state, cubeApi } = this.dependencies;
    state.loading = true;
    state.checking = false;
    state.error = '';
    this.dependencies.refreshUi();
    try {
      const { response, data: rawData } = await cubeApi.listCubePacks();
      const data = rawData as PackApiPayload;
      if (!response.ok || data.error) {
        state.error = data.error?.message || response.statusText || 'Failed to load cube packs';
        state.repos = [];
        state.identityPolicy = readIdentityPolicy(null);
        this.dependencies.refreshUi();
        return;
      }
      state.repos = readTrackedRepos(data.repos);
      state.identityPolicy = readIdentityPolicy(data.identity_policy);
      if (checkForUpdates && state.repos.length) {
        state.checking = true;
        this.dependencies.refreshUi();
        const checkResult = await cubeApi.checkAllCubePacks(
          JSON.stringify({ apply_auto_updates: true }),
          { headers: { 'Content-Type': 'application/json' } },
        );
        const checkData = checkResult.data as PackApiPayload;
        if (!checkResult.response.ok || checkData.error) {
          state.error =
            checkData.error?.message ||
            checkResult.response.statusText ||
            'Failed to check cube packs';
        } else {
          state.repos = checkData.repos ? readTrackedRepos(checkData.repos) : state.repos;
          state.identityPolicy = readIdentityPolicy(
            checkData.identity_policy || state.identityPolicy,
          );
        }
      }
    } catch (error: unknown) {
      state.error = this.dependencies.readErrorMessage(error);
      state.repos = [];
      state.identityPolicy = readIdentityPolicy(null);
    } finally {
      state.loading = false;
      state.checking = false;
      this.dependencies.refreshUi();
    }
  }

  /** Prompt for and persist the claimed GitHub owner. */
  async claimGithubOwner(): Promise<void> {
    const { state, ui } = this.dependencies;
    const currentOwner = state.identityPolicy.claimed_github_owner || '';
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
    if (!normalizedOwner || normalizedOwner === currentOwner) return;
    if (
      normalizedOwner.toLowerCase() === 'artificial-sweetener' &&
      !state.identityPolicy.allow_system_owner_claim
    ) {
      throw new Error(
        'Artificial-Sweetener can only be claimed when SUGARCUBES_ALLOW_SYSTEM_OWNER_CLAIM is enabled in .env or the process environment.',
      );
    }
    if (
      currentOwner &&
      !(await this.confirm({
        title: 'Change Claimed Owner?',
        message: [
          `Change your claimed GitHub owner from ${currentOwner} to ${normalizedOwner}?`,
          'This changes which tracked repos SugarCubes treats as writable.',
        ],
        confirmLabel: 'Change',
      }))
    ) {
      return;
    }
    await this.updateIdentityPolicy({ claimed_github_owner: normalizedOwner });
    this.dependencies.pushToast(
      'success',
      'Authoring access updated',
      `Claimed owner is now ${normalizedOwner}.`,
    );
  }

  /** Clear the persisted GitHub owner after confirmation. */
  async clearGithubOwner(): Promise<void> {
    const currentOwner = this.dependencies.state.identityPolicy.claimed_github_owner || '';
    if (!currentOwner) return;
    const confirmed = await this.confirm({
      title: 'Clear Claimed Owner?',
      message: [
        `Clear your claimed GitHub owner (${currentOwner})?`,
        'Tracked GitHub repos will return to read-only. local/... remains writable.',
      ],
      confirmLabel: 'Clear',
    });
    if (!confirmed) return;
    await this.updateIdentityPolicy({ claimed_github_owner: '' });
    this.dependencies.pushToast(
      'success',
      'Authoring access updated',
      'Tracked GitHub repos are read-only again.',
    );
  }

  /** Prompt for, preflight, and add one tracked Cube Pack. */
  async addTrackedRepo(): Promise<void> {
    const values = await this.dependencies.ui.dialogs?.openForm?.({
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
    if (!values) return;
    const { owner, repo } = parseRepoReference(values.repoRef);
    const payload = JSON.stringify({ owner, repo, enabled: true, auto_update: false });
    const preflight = await this.dependencies.cubeApi.preflightCubePack(payload, {
      headers: { 'Content-Type': 'application/json' },
    });
    const preflightData = preflight.data as PackApiPayload;
    if (!preflight.response.ok || preflightData.error) {
      throw new Error(
        preflightData.error?.message ||
          preflight.response.statusText ||
          'Failed to verify Cube Pack',
      );
    }
    const result = await this.dependencies.cubeApi.addCubePack(payload, {
      headers: { 'Content-Type': 'application/json' },
    });
    const data = result.data as PackApiPayload;
    if (!result.response.ok || data.error) {
      throw new Error(
        data.error?.message || result.response.statusText || 'Failed to add Cube Pack',
      );
    }
    const cubeCount = Number(data.preflight?.cube_count ?? preflightData.preflight?.cube_count);
    const detail = Number.isFinite(cubeCount)
      ? ` Found ${cubeCount} cube${cubeCount === 1 ? '' : 's'}.`
      : '';
    this.dependencies.pushToast(
      'success',
      'Cube Pack added',
      `${owner}/${repo} is now tracked.${detail}`,
    );
    await this.refresh();
    this.dependencies.invalidateDependentCatalogs();
  }

  /** Apply the available update for one tracked repository. */
  async syncTrackedRepo(repo: TrackedRepo): Promise<void> {
    await this.mutate(
      this.dependencies.cubeApi.updateCubePackNow(
        JSON.stringify({ owner: repo.owner, repo: repo.repo }),
        { headers: { 'Content-Type': 'application/json' } },
      ),
      'Failed to update Cube Pack',
    );
    this.dependencies.pushToast(
      'success',
      'Cube Pack updated',
      `${repo.owner}/${repo.repo} is up to date.`,
    );
    await this.refresh();
    this.dependencies.invalidateDependentCatalogs();
  }

  /** Apply every available tracked-repository update. */
  async syncAllTrackedRepos(): Promise<void> {
    await this.mutate(
      this.dependencies.cubeApi.updateAllCubePacks(),
      'Failed to update Cube Packs',
    );
    this.dependencies.pushToast(
      'success',
      'Cube Packs updated',
      'Available pack updates were applied.',
    );
    await this.refresh();
    this.dependencies.invalidateDependentCatalogs();
  }

  /** Enable or disable one tracked repository. */
  async toggleTrackedRepo(repo: TrackedRepo): Promise<void> {
    await this.updateRepo(repo, { enabled: !repo.enabled });
    this.dependencies.pushToast(
      'success',
      'Cube Pack updated',
      `${repo.owner}/${repo.repo} ${repo.enabled ? 'disabled' : 'enabled'}.`,
    );
    await this.refresh();
    this.dependencies.invalidateDependentCatalogs();
  }

  /** Change automatic update policy for one tracked repository. */
  async setAutoUpdate(repo: TrackedRepo, enabled: boolean): Promise<void> {
    await this.updateRepo(repo, { auto_update: Boolean(enabled) });
    this.dependencies.pushToast(
      'success',
      'Auto-update updated',
      `${repo.owner}/${repo.repo} auto-update ${enabled ? 'enabled' : 'disabled'}.`,
    );
    await this.refresh({ checkForUpdates: false });
  }

  /** Remove one tracked repository after confirmation. */
  async removeTrackedRepo(repo: TrackedRepo): Promise<void> {
    const confirmed = await this.confirm({
      title: 'Remove Cube Pack?',
      message: [
        `Stop tracking ${repo.owner}/${repo.repo}?`,
        'Local checkout files will remain on disk.',
      ],
      confirmLabel: 'Remove',
    });
    if (!confirmed) return;
    await this.mutate(
      this.dependencies.cubeApi.removeCubePack({ owner: repo.owner, repo: repo.repo }),
      'Failed to remove Cube Pack',
    );
    this.dependencies.pushToast(
      'success',
      'Cube Pack removed',
      `${repo.owner}/${repo.repo} removed from tracking.`,
    );
    await this.refresh();
    this.dependencies.invalidateDependentCatalogs();
  }

  private async updateIdentityPolicy(payload: UnknownRecord): Promise<void> {
    const result = await this.dependencies.cubeApi.updateIdentityPolicy(JSON.stringify(payload), {
      headers: { 'Content-Type': 'application/json' },
    });
    const data = result.data as PackApiPayload;
    if (!result.response.ok || data.error) {
      throw new Error(
        data.error?.message || result.response.statusText || 'Failed to update identity policy',
      );
    }
    this.dependencies.state.identityPolicy = readIdentityPolicy(data);
    await this.refresh({ checkForUpdates: false });
  }

  private async updateRepo(repo: TrackedRepo, updates: UnknownRecord): Promise<void> {
    await this.mutate(
      this.dependencies.cubeApi.updateCubePack(
        JSON.stringify({ owner: repo.owner, repo: repo.repo, ...updates }),
        { headers: { 'Content-Type': 'application/json' } },
      ),
      'Failed to update Cube Pack',
    );
  }

  private async mutate(resultPromise: Promise<ApiJsonResult>, fallback: string): Promise<void> {
    const result = await resultPromise;
    const data = result.data as PackApiPayload;
    if (!result.response.ok || data.error) {
      throw new Error(data.error?.message || result.response.statusText || fallback);
    }
  }

  private async confirm(options: {
    title: string;
    message: string | string[];
    confirmLabel: string;
  }): Promise<boolean> {
    const { ui, adapter } = this.dependencies;
    if (ui.dialogs?.confirm) return ui.dialogs.confirm(options);
    if (ui.confirmDialog?.open) return ui.confirmDialog.open(options);
    const confirmRef = adapter.getWindow?.()?.confirm || globalThis.confirm;
    if (typeof confirmRef !== 'function') return false;
    const lines = Array.isArray(options.message) ? options.message : [options.message];
    return Boolean(confirmRef(lines.filter(Boolean).join('\n')));
  }
}

function parseRepoReference(value: unknown): { owner: string; repo: string } {
  const normalized = typeof value === 'string' ? value.trim() : '';
  const [owner = '', repo = ''] = normalized.split('/', 2);
  return { owner: owner.trim(), repo: repo.trim() };
}

function validateRepoReference(value: unknown): string {
  const { owner, repo } = parseRepoReference(value);
  return owner && repo ? '' : 'Cube Pack source must use owner/repo format.';
}

function readTrackedRepos(value: unknown): TrackedRepo[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((entry) => {
    if (!isRecord(entry) || typeof entry.owner !== 'string' || typeof entry.repo !== 'string')
      return [];
    return [
      {
        owner: entry.owner,
        repo: entry.repo,
        ...(typeof entry.repo_ref === 'string' ? { repo_ref: entry.repo_ref } : {}),
        enabled: entry.enabled !== false,
        ...(typeof entry.auto_update === 'boolean' ? { auto_update: entry.auto_update } : {}),
        ...(typeof entry.update_available === 'boolean'
          ? { update_available: entry.update_available }
          : {}),
        ...(typeof entry.is_writable === 'boolean' ? { is_writable: entry.is_writable } : {}),
        ...(typeof entry.is_system_pack === 'boolean'
          ? { is_system_pack: entry.is_system_pack }
          : {}),
        ...(typeof entry.default_base_repo === 'boolean'
          ? { default_base_repo: entry.default_base_repo }
          : {}),
        ...(typeof entry.last_sync_status === 'string'
          ? { last_sync_status: entry.last_sync_status }
          : {}),
        ...(typeof entry.last_check_status === 'string'
          ? { last_check_status: entry.last_check_status }
          : {}),
        ...(typeof entry.last_checked_at === 'string'
          ? { last_checked_at: entry.last_checked_at }
          : {}),
        ...(typeof entry.last_sync_at === 'string' ? { last_sync_at: entry.last_sync_at } : {}),
        ...(typeof entry.last_check_error === 'string'
          ? { last_check_error: entry.last_check_error }
          : {}),
        ...(typeof entry.last_sync_error === 'string'
          ? { last_sync_error: entry.last_sync_error }
          : {}),
        ...(typeof entry.write_block_reason === 'string'
          ? { write_block_reason: entry.write_block_reason }
          : {}),
      },
    ];
  });
}
