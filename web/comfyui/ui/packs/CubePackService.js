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
 * Own claimed-owner and writable-pack selection for authoring workflows.
 */
import { isRecord } from '../types/common.js';
function readApiError(data) {
    const error = isRecord(data.error) ? data.error : {};
    return typeof error.message === 'string' ? error.message : '';
}
const CREATE_PACK_VALUE = '__create_pack__';
/** Coordinate claimed identity and authoring pack setup through SugarCubes dialogs. */
export class CubePackService {
    api;
    dialogs;
    toast;
    constructor({ api, dialogs, toast } = {}) {
        if (!api) {
            throw new Error('Cube library API is required.');
        }
        this.api = api;
        this.dialogs = dialogs ?? null;
        this.toast = toast ?? null;
    }
    /** Return a selected writable pack, creating or claiming prerequisites when requested. */
    async chooseWritablePack() {
        let catalog = await this.loadCatalog();
        const owner = await this.ensureClaimedOwner(catalog.identityPolicy);
        if (!owner) {
            return null;
        }
        if (owner !== catalog.identityPolicy?.claimed_github_owner) {
            catalog = await this.loadCatalog();
        }
        const matchingPacks = catalog.packs.filter((pack) => pack.owner.toLowerCase() === owner.toLowerCase());
        const selected = await this.dialogs?.selectItem?.({
            title: 'Choose Cube Pack',
            message: ['Select the managed pack that will own this SugarCube.'],
            confirmLabel: 'Continue',
            items: [
                ...matchingPacks.map((pack) => ({
                    value: pack.repoRef,
                    label: pack.repo,
                    description: pack.owner,
                })),
                {
                    value: CREATE_PACK_VALUE,
                    label: 'Create a new pack',
                    description: `Create a writable pack owned by ${owner}.`,
                },
            ],
        });
        if (!selected) {
            return null;
        }
        if (selected === CREATE_PACK_VALUE) {
            return this.createAuthoringPack(owner);
        }
        return matchingPacks.find((pack) => pack.repoRef === selected) || null;
    }
    /** Load writable packs and the authoritative claimed-owner policy. */
    async loadCatalog() {
        const { response, data } = await this.api.listCubePacks();
        if (!response.ok || data.error) {
            throw new Error(readApiError(data) || response.statusText || 'Failed to load cube packs');
        }
        const packs = (Array.isArray(data.repos) ? data.repos : [])
            .filter(isRecord)
            .filter((repo) => repo.enabled && repo.is_writable && repo.owner && repo.repo)
            .map((repo) => ({
            owner: String(repo.owner).trim(),
            repo: String(repo.repo).trim(),
            repoRef: `${String(repo.owner).trim()}/${String(repo.repo).trim()}`,
        }));
        return { packs, identityPolicy: isRecord(data.identity_policy) ? data.identity_policy : {} };
    }
    /** Prompt once for the GitHub owner whose packs this installation manages. */
    async ensureClaimedOwner(identityPolicy = {}) {
        const current = String(identityPolicy.claimed_github_owner || '').trim();
        if (current) {
            return current;
        }
        const values = await this.dialogs?.openForm?.({
            title: 'Claim Your Pack Owner',
            message: [
                'Enter the GitHub owner used for packs you manage. Your personal cube stays local until promotion completes.',
            ],
            confirmLabel: 'Claim Owner',
            fields: [
                {
                    key: 'owner',
                    label: 'GitHub owner',
                    placeholder: 'YourGitHubName',
                    required: true,
                    normalizeValue: (value) => value.trim(),
                },
            ],
        });
        if (typeof values?.owner !== 'string' || !values.owner) {
            return '';
        }
        const { response, data } = await this.api.updateIdentityPolicy(JSON.stringify({ claimed_github_owner: values.owner }), { headers: { 'Content-Type': 'application/json' } });
        if (!response.ok || data.error) {
            throw new Error(readApiError(data) || response.statusText || 'Failed to claim owner');
        }
        return String(data?.claimed_github_owner || values.owner).trim();
    }
    /** Create one initialized writable pack for the claimed owner. */
    async createAuthoringPack(owner) {
        const values = await this.dialogs?.openForm?.({
            title: 'Create Cube Pack',
            message: [`Create a managed pack owned by ${owner}.`],
            confirmLabel: 'Create Pack',
            fields: [
                {
                    key: 'repo',
                    label: 'Pack name',
                    placeholder: 'My-Cubes',
                    required: true,
                    normalizeValue: (value) => value.trim(),
                },
            ],
        });
        if (typeof values?.repo !== 'string' || !values.repo) {
            return null;
        }
        const { response, data } = await this.api.createAuthoringCubePack(JSON.stringify({ owner, repo: values.repo, enabled: true }), { headers: { 'Content-Type': 'application/json' } });
        if (!response.ok || data.error) {
            throw new Error(readApiError(data) || response.statusText || 'Failed to create cube pack');
        }
        const repo = isRecord(data.repo) ? data.repo : {};
        if (!repo.owner || !repo.repo || !repo.is_writable) {
            throw new Error('Created cube pack is not writable.');
        }
        const pack = {
            owner: String(repo.owner).trim(),
            repo: String(repo.repo).trim(),
            repoRef: `${String(repo.owner).trim()}/${String(repo.repo).trim()}`,
        };
        this.toast?.push('success', 'Cube Pack created', `${pack.repoRef} is ready.`);
        return pack;
    }
}
