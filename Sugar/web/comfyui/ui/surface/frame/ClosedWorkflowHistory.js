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
/** Retain workflows closed from the frame so a close can be undone. */
/** Most workflows a person can step back through. */
const MAXIMUM_RECORDS = 20;
/** Most serialized text retained across every record, in UTF-16 code units. */
const MAXIMUM_CHARACTERS = 32 * 1024 * 1024;
/**
 * Keep the latest closed workflows, newest last, within a count and size budget.
 *
 * A close is requested before the host decides it: the host may ask to save, and the person may
 * cancel. The frame therefore captures every open workflow when a close is requested, and a
 * captured workflow becomes a record only once the host no longer has it open. Workflows are
 * identified by the host's own workflow objects, which the history never reads or retains beyond
 * the pending capture.
 */
export class ClosedWorkflowHistory {
    #records = [];
    #pending = new Map();
    /** Whether a closed workflow can be reopened. */
    get available() {
        return this.#records.length > 0;
    }
    /** Replace the pending capture with the workflows open as a close is requested. */
    capture(open) {
        this.#pending = new Map(open);
    }
    /** Record every captured workflow the host has closed; captures still open stay pending. */
    settle(open) {
        for (const [workflow, record] of this.#pending) {
            if (open.has(workflow))
                continue;
            this.#pending.delete(workflow);
            this.#push(record);
        }
    }
    /** Remove and return the most recently closed workflow. */
    take() {
        return this.#records.pop() ?? null;
    }
    /** Put back a workflow whose reopening failed, so it can be tried again. */
    restore(record) {
        this.#push(record);
    }
    /** Append one record, dropping the oldest until the budgets hold; an oversized one is refused. */
    #push(record) {
        if (record.graphJson.length > MAXIMUM_CHARACTERS)
            return;
        this.#records.push(record);
        const size = () => this.#records.reduce((total, item) => total + item.graphJson.length, 0);
        while (this.#records.length > MAXIMUM_RECORDS || size() > MAXIMUM_CHARACTERS)
            this.#records.shift();
    }
}
