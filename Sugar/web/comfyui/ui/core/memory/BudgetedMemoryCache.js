/**
 * Retain values under one registry consumer.
 *
 * Map order is use order, so the first unpinned entry of a tier is its least recently used.
 * Local bounds and the shared allowance both evict only unpinned entries.
 */
export class BudgetedMemoryCache {
    id;
    #options;
    #slots = new Map();
    #unregister;
    #bytes = 0;
    /** Register with the page allowance for the cache's whole lifetime. */
    constructor(options) {
        this.id = options.id;
        this.#options = options;
        this.#unregister = options.registry.register(this);
    }
    /** Count retained entries. */
    get size() {
        return this.#slots.size;
    }
    /** Return a value and mark it most recently used. */
    get(key) {
        const slot = this.#slots.get(key);
        if (!slot)
            return undefined;
        this.#slots.delete(key);
        slot.used = this.#options.registry.stamp();
        this.#slots.set(key, slot);
        return slot.value;
    }
    /** Return whether a key is retained without changing its use order. */
    has(key) {
        return this.#slots.has(key);
    }
    /** Retain one value, releasing a different value it replaces, then enforce every bound. */
    set(key, value, entry) {
        const previous = this.#slots.get(key);
        if (previous) {
            this.#slots.delete(key);
            this.#bytes -= previous.bytes;
            if (previous.value !== value)
                this.#options.release?.(previous.value, key);
        }
        const bytes = requireEntryBytes(entry.bytes);
        this.#slots.set(key, {
            value,
            tier: entry.tier,
            bytes,
            used: this.#options.registry.stamp(),
        });
        this.#bytes += bytes;
        this.#enforceLocalBounds();
        this.#options.registry.reconcile();
    }
    /** Update one retained entry's size once its real cost is known. */
    resize(key, bytes) {
        const slot = this.#slots.get(key);
        if (!slot)
            return;
        const next = requireEntryBytes(bytes);
        this.#bytes += next - slot.bytes;
        slot.bytes = next;
        this.#enforceLocalBounds();
        this.#options.registry.reconcile();
    }
    /** Remove and release one entry. */
    delete(key) {
        const slot = this.#slots.get(key);
        if (!slot)
            return false;
        this.#remove(key, slot);
        return true;
    }
    /** Remove one entry without releasing it, transferring ownership of its value to the caller. */
    take(key) {
        const slot = this.#slots.get(key);
        if (!slot)
            return undefined;
        this.#slots.delete(key);
        this.#bytes -= slot.bytes;
        return slot.value;
    }
    /** Remove and release every entry matching a predicate. */
    deleteWhere(matches) {
        for (const [key, slot] of [...this.#slots])
            if (matches(key, slot.value))
                this.#remove(key, slot);
    }
    /** Remove and release every entry. */
    clear() {
        for (const [key, slot] of [...this.#slots])
            this.#remove(key, slot);
    }
    /** Release every entry and leave the page allowance. */
    dispose() {
        this.clear();
        this.#unregister();
    }
    /** Report every retained byte, pinned entries included. */
    bytes() {
        return this.#bytes;
    }
    /** Report the least recently used evictable entry of one tier. */
    oldestEvictable(tier) {
        for (const [key, slot] of this.#slots)
            if (slot.tier === tier && !this.#pinned(key, slot))
                return slot.used;
        return null;
    }
    /** Evict least recently used unpinned entries of one tier until `targetBytes` are released. */
    evict(targetBytes, tier) {
        let released = 0;
        for (const [key, slot] of [...this.#slots]) {
            if (released >= targetBytes)
                break;
            if (slot.tier !== tier || this.#pinned(key, slot))
                continue;
            released += slot.bytes;
            this.#remove(key, slot);
        }
        return released;
    }
    /** Apply the cache's own entry and byte bounds, oldest unpinned entries first. */
    #enforceLocalBounds() {
        const { maxEntries = Number.POSITIVE_INFINITY, maxBytes = Number.POSITIVE_INFINITY } = this.#options;
        for (const [key, slot] of [...this.#slots]) {
            if (this.#slots.size <= maxEntries && this.#bytes <= maxBytes)
                return;
            if (!this.#pinned(key, slot))
                this.#remove(key, slot);
        }
    }
    /** Read one entry's in-use state from its owner. */
    #pinned(key, slot) {
        return this.#options.pinned?.(slot.value, key) ?? false;
    }
    /** Drop one entry and release its platform resources exactly once. */
    #remove(key, slot) {
        if (this.#slots.get(key) !== slot)
            return;
        this.#slots.delete(key);
        this.#bytes -= slot.bytes;
        this.#options.release?.(slot.value, key);
    }
}
/** Accept only finite non-negative entry sizes. */
function requireEntryBytes(bytes) {
    if (!Number.isFinite(bytes) || bytes < 0)
        throw new RangeError('Cache entry size must be a non-negative byte count.');
    return bytes;
}
