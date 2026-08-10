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
/** Coordinate backend-owned default review before implementation persistence. */
import { isRecord } from '../types/common.js';
/** Own preview transport, response validation, and final decision attachment. */
export class CubeDefaultReviewService {
    #api;
    #dialogs;
    /** Bind the backend policy boundary to its single presentation collaborator. */
    constructor(options) {
        this.#api = options.api;
        this.#dialogs = options.dialogs;
    }
    /** Return a reviewed save request, or null when the author cancels. */
    async review(request) {
        const { response, data } = await this.#api.previewImplementation(JSON.stringify(request), {
            headers: { 'Content-Type': 'application/json' },
        });
        if (!response.ok || data.error) {
            throw new Error(readApiError(data) || response.statusText || 'Cube default review failed');
        }
        const reviews = readReviews(data.reviews);
        const requiringReview = reviews.filter(({ requiresDefaultDecision }) => requiresDefaultDecision);
        const decisions = requiringReview.length
            ? await this.#dialogs.reviewImplementationDefaults(requiringReview)
            : {};
        if (decisions === null)
            return null;
        return attachDecisions(request, reviews, decisions);
    }
}
/** Validate the host-neutral backend review response. */
function readReviews(value) {
    if (!Array.isArray(value))
        throw new TypeError('Cube default review response is invalid');
    return value.map((entry) => {
        if (!isRecord(entry))
            throw new TypeError('Cube default review entry is invalid');
        const cubeId = readString(entry.cube_id);
        const displayName = readString(entry.display_name);
        const fingerprint = readString(entry.fingerprint);
        const overwriteDefaultCount = readCount(entry.overwrite_default_count);
        const promptDefaultCount = readCount(entry.prompt_default_count);
        if (!cubeId || !displayName || !fingerprint || !Array.isArray(entry.changes)) {
            throw new TypeError('Cube default review identity is invalid');
        }
        return {
            cubeId,
            displayName,
            fingerprint,
            requiresDefaultDecision: entry.requires_default_decision === true,
            overwriteDefaultCount,
            promptDefaultCount,
            changes: entry.changes.map(readChange),
        };
    });
}
/** Validate one change row without interpreting backend policy. */
function readChange(value) {
    if (!isRecord(value))
        throw new TypeError('Cube implementation change is invalid');
    const section = readString(value.section);
    const label = readString(value.label);
    const decision = readDecision(value.decision);
    if (!section || !label || !decision) {
        throw new TypeError('Cube implementation change identity is missing');
    }
    return {
        section,
        label,
        path: readString(value.path),
        previousValue: value.previous_value,
        proposedValue: value.proposed_value,
        previousExists: value.previous_exists === true,
        proposedExists: value.proposed_exists === true,
        decision,
    };
}
/** Attach decisions to the same cube entries used by backend fingerprinting. */
function attachDecisions(request, reviews, decisions) {
    const reviewById = new Map(reviews.map((review) => [review.cubeId, review]));
    const cubes = Array.isArray(request.cubes) ? request.cubes : [];
    return {
        ...request,
        cubes: cubes.map((entry) => {
            if (!isRecord(entry))
                throw new TypeError('Cube save entry is invalid');
            const cubeId = readString(entry.cube_id);
            const review = reviewById.get(cubeId);
            if (!review)
                throw new TypeError(`Cube default review missing for '${cubeId}'`);
            const decision = decisions[cubeId];
            return {
                ...entry,
                default_review: {
                    fingerprint: review.fingerprint,
                    overwrite_defaults: decision?.overwriteDefaults === true,
                    save_prompt_fields: decision?.savePromptFields === true,
                },
            };
        }),
    };
}
function readString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
function readCount(value) {
    if (typeof value !== 'number' || !Number.isInteger(value) || value < 0) {
        throw new TypeError('Cube default review count is invalid');
    }
    return value;
}
function readDecision(value) {
    return value === 'always' || value === 'overwrite_defaults' || value === 'save_prompt_fields'
        ? value
        : null;
}
function readApiError(data) {
    if (typeof data.error === 'string')
        return data.error.trim();
    return isRecord(data.error) && typeof data.error.message === 'string'
        ? data.error.message.trim()
        : '';
}
