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
/** Translate queue observations into validated accepted submission identities. */
import { isRecord } from '../../types/common.js';
import { promptId } from '../domain/ExecutionIdentity.js';
import { decodeComfyPromptSources } from './ComfyPromptSourceDecoder.js';
import { decodeComfySubmittedSeeds } from './ComfySubmittedSeeds.js';
/** Resolve only observed or explicitly associated prompts and contain diagnostic failures in the bridge. */
export class ComfyPromptSubmissionObserver {
    options;
    logger;
    /** Bind typed association publication rather than introducing a second queue interceptor. */
    constructor(options) {
        this.options = options;
        this.logger = options.logger;
    }
    /** Capture immutable entry-time ownership before preparation or transformation can await. */
    capture(payload) {
        if (!isRecord(payload))
            return null;
        const captured = this.options.contexts.read(payload);
        if (!captured)
            return null;
        const submission = this.options.submitting(captured);
        let context = submission;
        let preparedPayload = payload;
        let active = true;
        return {
            prepared: (prepared) => {
                if (!active)
                    return;
                preparedPayload = prepared;
                const sources = decodeComfyPromptSources(prepared, captured.sources);
                if (sources.length)
                    context = Object.freeze({ ...submission, sources });
            },
            accepted: (response) => {
                if (!active)
                    return;
                active = false;
                const id = isRecord(response) ? response.prompt_id : null;
                if (typeof id !== 'string' || !id.trim() || id.length > 4096) {
                    this.options.rejected?.(submission);
                    this.logger.warn('SugarCubes cannot associate a queue response without a valid prompt ID.', {
                        sessionId: context.session.sessionId,
                    });
                    return;
                }
                const executionSources = isRecord(response)
                    ? decodeComfyPromptSources({ output: response.execution_prompt }, context.sources)
                    : [];
                if (executionSources.length)
                    context = Object.freeze({ ...submission, sources: executionSources });
                const prompt = isRecord(response) && Object.hasOwn(response, 'execution_prompt')
                    ? response.execution_prompt
                    : isRecord(preparedPayload)
                        ? preparedPayload.output
                        : null;
                this.options.seeds?.record(context.session, context.order, decodeComfySubmittedSeeds(prompt));
                this.options.accepted(context, promptId(id));
            },
            rejected: () => {
                if (active)
                    this.options.rejected?.(submission);
                active = false;
            },
            cancelled: () => {
                if (active)
                    this.options.rejected?.(submission);
                active = false;
            },
        };
    }
}
