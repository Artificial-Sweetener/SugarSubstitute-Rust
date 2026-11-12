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
/** Create the empty state used before the first repository refresh. */
export function createRepoPanelState() {
    return {
        loading: false,
        checking: false,
        repos: [],
        error: '',
        identityPolicy: readIdentityPolicy(null),
    };
}
/** Normalize the backend identity-policy shape. */
export function readIdentityPolicy(payload) {
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
    const record = payload;
    const claimedOwner = typeof record.claimed_github_owner === 'string' ? record.claimed_github_owner.trim() : '';
    return {
        claimed_github_owner: claimedOwner,
        allow_system_owner_claim: Boolean(record.allow_system_owner_claim),
        has_claimed_github_owner: Boolean(claimedOwner),
        claimed_github_owner_source: typeof record.claimed_github_owner_source === 'string' && record.claimed_github_owner_source
            ? record.claimed_github_owner_source
            : 'default',
        allow_system_owner_claim_source: typeof record.allow_system_owner_claim_source === 'string' &&
            record.allow_system_owner_claim_source
            ? record.allow_system_owner_claim_source
            : 'default',
        env_override_active: Boolean(record.env_override_active),
    };
}
