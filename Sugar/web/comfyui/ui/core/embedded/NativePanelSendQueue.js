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
/** Hold canvas requests the native bridge cannot yet queue, delivering them in order as it frees capacity. */
import { isRecord } from '../../types/common.js';
/** The bridge's refusal code when its bounded queue of waiting canvas requests is full. */
const CAPACITY_REFUSAL = 'panel.transport.capacity';
/** Poll cadence while the bridge reports no capacity and no host message has arrived to wake the queue. */
const RETRY_MILLISECONDS = 25;
/** Bound page-side waiting work so a stalled host fails loudly instead of growing memory. */
const MAXIMUM_HELD = 256;
/**
 * Keep the page within the window the native bridge admits.
 *
 * The bridge queues a handful of canvas requests behind the one the host is serving and refuses
 * the rest, while the canvas client may have many outstanding. A refused request is not a failure:
 * it is held here, with every later canvas message behind it so a cancellation can never overtake
 * its request, and delivered in order once the bridge frees room. Control messages, which the
 * bridge admits on their own lane, are never delayed behind canvas work.
 */
export class NativePanelSendQueue {
    options;
    #held = [];
    #stopRetry = null;
    #failure = null;
    constructor(options) {
        this.options = options;
    }
    /** Deliver or hold one message; a failure of earlier held delivery surfaces on the next send. */
    send(message) {
        if (this.#failure) {
            const failure = this.#failure;
            this.#failure = null;
            throw failure;
        }
        if (message.protocol !== 'sugarcubes.canvas') {
            this.options.send(message);
            return;
        }
        if (this.#held.length === 0) {
            try {
                this.options.send(message);
                return;
            }
            catch (error) {
                if (!capacityRefused(error))
                    throw error;
            }
        }
        if (this.#held.length >= MAXIMUM_HELD)
            throw new RangeError('Native canvas transport backlog exceeded its bound.');
        this.#held.push(message);
        this.#schedule();
    }
    /** Retry held messages now; the host just made progress, which is when capacity frees. */
    progressed() {
        this.#deliver();
    }
    /** Deliver held messages in order until the bridge refuses again or the queue empties. */
    #deliver() {
        this.#stopRetry?.();
        this.#stopRetry = null;
        while (this.#held.length > 0) {
            const next = this.#held[0];
            if (!next)
                return;
            try {
                this.options.send(next);
            }
            catch (error) {
                if (capacityRefused(error)) {
                    this.#schedule();
                    return;
                }
                this.#held.length = 0;
                this.#failure =
                    error instanceof Error ? error : new Error('Native canvas transport failed.');
                return;
            }
            this.#held.shift();
        }
    }
    /** Wait for the next host message or the retry cadence, whichever frees capacity first. */
    #schedule() {
        this.#stopRetry ??= this.options.later(() => this.#deliver(), RETRY_MILLISECONDS);
    }
}
/** Recognize the bridge's typed capacity refusal without trusting its message text. */
function capacityRefused(error) {
    const cause = isRecord(error) ? error.cause : null;
    return isRecord(cause) && cause.code === CAPACITY_REFUSAL;
}
