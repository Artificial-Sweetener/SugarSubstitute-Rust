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
/** Define shared host-settings state and collaboration contracts. */

import type { ComfyAdapter } from '../core/ComfyAdapter.js';
import type { CubeLibraryApi } from '../core/CubeLibraryApi.js';
import type { SugarCubesUI } from '../SugarCubesUI.js';
import type { UnknownRecord } from '../types/common.js';

export type ToastSeverity = 'success' | 'info' | 'warn' | 'error';

export interface IdentityPolicy {
  claimed_github_owner: string;
  allow_system_owner_claim: boolean;
  has_claimed_github_owner: boolean;
  claimed_github_owner_source: string;
  allow_system_owner_claim_source: string;
  env_override_active: boolean;
}

export interface TrackedRepo {
  owner: string;
  repo: string;
  repo_ref?: string;
  enabled: boolean;
  auto_update?: boolean;
  update_available?: boolean;
  is_writable?: boolean;
  is_system_pack?: boolean;
  default_base_repo?: boolean;
  last_sync_status?: string;
  last_check_status?: string;
  last_checked_at?: string;
  last_sync_at?: string;
  last_check_error?: string;
  last_sync_error?: string;
  write_block_reason?: string;
}

export interface RepoPanelState {
  loading: boolean;
  checking: boolean;
  repos: TrackedRepo[];
  error: string;
  identityPolicy: IdentityPolicy;
}

export interface PackApiPayload extends UnknownRecord {
  error?: { message?: string };
  repos?: unknown[];
  identity_policy?: unknown;
  preflight?: { cube_count?: unknown };
}

export interface HostSettingsCommandDependencies {
  adapter: ComfyAdapter;
  cubeApi: CubeLibraryApi;
  ui: SugarCubesUI;
  state: RepoPanelState;
  pushToast(severity: ToastSeverity, summary: string, detail: string): void;
  readErrorMessage(error: unknown): string;
  invalidateDependentCatalogs(): void;
  refreshUi(): void;
}

/** Create the empty state used before the first repository refresh. */
export function createRepoPanelState(): RepoPanelState {
  return {
    loading: false,
    checking: false,
    repos: [],
    error: '',
    identityPolicy: readIdentityPolicy(null),
  };
}

/** Normalize the backend identity-policy shape. */
export function readIdentityPolicy(payload: unknown): IdentityPolicy {
  if (!payload || typeof payload !== 'object' || Array.isArray(payload)) {
    return {
      claimed_github_owner: '',
      allow_system_owner_claim: false,
      has_claimed_github_owner: false,
      claimed_github_owner_source: 'default',
      allow_system_owner_claim_source: 'default',
      env_override_active: false,
    };
  }
  const record = payload as UnknownRecord;
  const claimedOwner =
    typeof record.claimed_github_owner === 'string' ? record.claimed_github_owner.trim() : '';
  return {
    claimed_github_owner: claimedOwner,
    allow_system_owner_claim: Boolean(record.allow_system_owner_claim),
    has_claimed_github_owner: Boolean(claimedOwner),
    claimed_github_owner_source:
      typeof record.claimed_github_owner_source === 'string' && record.claimed_github_owner_source
        ? record.claimed_github_owner_source
        : 'default',
    allow_system_owner_claim_source:
      typeof record.allow_system_owner_claim_source === 'string' &&
      record.allow_system_owner_claim_source
        ? record.allow_system_owner_claim_source
        : 'default',
    env_override_active: Boolean(record.env_override_active),
  };
}
