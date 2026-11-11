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
/** Own the page's single allowance for every SugarCubes in-memory cache. */
/** Standalone allowance used until a host supplies its own. */
export const DEFAULT_MEMORY_ALLOWANCE_BYTES = 192 * 1024 * 1024;
const MEMORY_TIERS = [1, 2, 3];
/**
 * Enforce one byte allowance across every registered cache.
 *
 * Eviction always takes the lowest tier first and, within a tier, the least recently used entry
 * across all caches. `setAllowance` applies a one-time reduction for the pressure level, and the
 * level's steady ceiling then holds for later growth:
 *
 * - normal: retain up to the allowance.
 * - elevated: evict to 85% of the allowance and stay there.
 * - high: evict to half of current use (never above the allowance), then hold half the allowance.
 * - critical: keep only pinned entries.
 *
 * A host adapter drives {@link setAllowance} and reports {@link usage}; listeners registered with
 * {@link subscribe} learn of usage changes without polling.
 */
export class MemoryBudgetRegistry {
    #consumers = new Map();
    #listeners = new Set();
    #allowance;
    #level = 'normal';
    #clock = 0;
    #enforcing = false;
    #notifyQueued = false;
    /** Start with the standalone allowance. */
    constructor(allowanceBytes = DEFAULT_MEMORY_ALLOWANCE_BYTES) {
        this.#allowance = requireBytes(allowanceBytes);
    }
    /** Return a monotonically increasing use stamp shared by every consumer. */
    stamp() {
        this.#clock += 1;
        return this.#clock;
    }
    /** Register one consumer; the returned release removes it without evicting its entries. */
    register(consumer) {
        if (this.#consumers.has(consumer.id))
            throw new Error(`Memory consumer '${consumer.id}' is already registered.`);
        this.#consumers.set(consumer.id, consumer);
        this.reconcile();
        return () => {
            if (this.#consumers.get(consumer.id) !== consumer)
                return;
            this.#consumers.delete(consumer.id);
            this.#queueNotification();
        };
    }
    /** Apply a host allowance and pressure level, evicting immediately to the level's target. */
    setAllowance(allowanceBytes, level) {
        this.#allowance = requireBytes(allowanceBytes);
        this.#level = level;
        const target = level === 'high' ? Math.min(this.#used() * 0.5, this.#allowance) : this.#ceiling();
        this.#evictTo(target);
        this.#queueNotification();
    }
    /** Enforce the current ceiling after a consumer retained new bytes. */
    reconcile() {
        this.#evictTo(this.#ceiling());
        this.#queueNotification();
    }
    /** Report retained bytes by consumer. */
    usage() {
        const consumers = [...this.#consumers.values()].map((consumer) => ({
            id: consumer.id,
            bytes: consumer.bytes(),
        }));
        return {
            usedBytes: consumers.reduce((total, consumer) => total + consumer.bytes, 0),
            consumers,
        };
    }
    /** Observe coalesced usage changes until the returned release is called. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Resolve the steady ceiling the current level holds after its one-time reduction. */
    #ceiling() {
        switch (this.#level) {
            case 'normal':
                return this.#allowance;
            case 'elevated':
                return this.#allowance * 0.85;
            case 'high':
                return this.#allowance * 0.5;
            case 'critical':
                return 0;
        }
    }
    /** Evict lowest tier first, least recently used across consumers, until within `target`. */
    #evictTo(target) {
        // Eviction callbacks may release entries that report back; never re-enter the loop.
        if (this.#enforcing)
            return;
        this.#enforcing = true;
        try {
            let used = this.#used();
            for (const tier of MEMORY_TIERS) {
                while (used > target) {
                    const consumer = this.#oldest(tier);
                    if (!consumer)
                        break;
                    const released = consumer.evict(1, tier);
                    if (released <= 0 && consumer.oldestEvictable(tier) !== null)
                        break;
                    used = this.#used();
                }
                if (used <= target)
                    return;
            }
        }
        finally {
            this.#enforcing = false;
        }
    }
    /** Find the consumer holding the least recently used evictable entry of one tier. */
    #oldest(tier) {
        let oldest = null;
        let stamp = Number.POSITIVE_INFINITY;
        for (const consumer of this.#consumers.values()) {
            const candidate = consumer.oldestEvictable(tier);
            if (candidate !== null && candidate < stamp) {
                stamp = candidate;
                oldest = consumer;
            }
        }
        return oldest;
    }
    /** Sum the bytes every consumer currently retains. */
    #used() {
        let used = 0;
        for (const consumer of this.#consumers.values())
            used += consumer.bytes();
        return used;
    }
    /** Publish one usage report per microtask regardless of how many changes occurred. */
    #queueNotification() {
        if (this.#notifyQueued || !this.#listeners.size)
            return;
        this.#notifyQueued = true;
        queueMicrotask(() => {
            this.#notifyQueued = false;
            const usage = this.usage();
            for (const listener of [...this.#listeners])
                listener(usage);
        });
    }
}
/** Accept only finite non-negative byte quantities. */
function requireBytes(value) {
    if (!Number.isFinite(value) || value < 0)
        throw new RangeError('Memory allowance must be a non-negative byte count.');
    return value;
}
