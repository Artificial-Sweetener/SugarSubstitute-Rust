import {
  caseFold,
  isWildcardCompletionBody,
  parsePrompt,
  requireValue,
  strip,
  unicodeLowerCase,
  wildcardDelimiters
} from "./chunk-BJZB62WU.js";

// frontend/sugarprompt/services/src/text/substring-index.ts
function grams(value, size) {
  const characters = Array.from(value), result = [];
  for (let start = 0; start <= characters.length - size; start++)
    result.push(characters.slice(start, start + size).join(""));
  return result;
}
var ScalarSubstringIndex = class {
  postings = /* @__PURE__ */ new Map();
  items;
  /** Build once from bounded captured fields, deduplicating each item across metadata and identity aliases. */
  constructor(items, fields) {
    this.items = Object.freeze([...items]);
    const postings = /* @__PURE__ */ new Map();
    for (const item of items) {
      const keys = /* @__PURE__ */ new Set();
      for (const field of fields(item)) {
        for (const size of [1, 2, 3])
          for (const gram of grams(field, size)) keys.add(`${size}:${gram}`);
      }
      for (const key of keys) {
        const posting = postings.get(key);
        if (posting) posting.push(item);
        else postings.set(key, [item]);
      }
    }
    for (const [key, values] of postings) this.postings.set(key, Object.freeze(values));
  }
  /** Empty queries deliberately include every model; other lookups use the rarest scalar gram, with final exact matching downstream. */
  candidates(query) {
    if (!query) return this.items;
    const size = Math.min(3, Array.from(query).length);
    let selected = null;
    for (const gram of grams(query, size)) {
      const posting = this.postings.get(`${size}:${gram}`);
      if (!posting) return [];
      if (!selected || posting.length < selected.length) selected = posting;
    }
    return selected ?? [];
  }
};

// frontend/sugarprompt/services/src/text/scalar.ts
function compareScalarText(left, right) {
  if (left === right) return 0;
  let leftIndex = 0, rightIndex = 0;
  while (leftIndex < left.length && rightIndex < right.length) {
    const a = requireValue(left.codePointAt(leftIndex), "scalar"), b = requireValue(right.codePointAt(rightIndex), "scalar"), difference = a - b;
    if (difference) return difference;
    leftIndex += a > 65535 ? 2 : 1;
    rightIndex += b > 65535 ? 2 : 1;
  }
  let remaining = 0;
  while (leftIndex < left.length) {
    leftIndex += requireValue(left.codePointAt(leftIndex), "scalar") > 65535 ? 2 : 1;
    remaining++;
  }
  while (rightIndex < right.length) {
    rightIndex += requireValue(right.codePointAt(rightIndex), "scalar") > 65535 ? 2 : 1;
    remaining--;
  }
  return remaining;
}

// frontend/sugarprompt/services/src/wildcards/metadata.ts
function record(value, keys) {
  return value !== null && typeof value === "object" && !Array.isArray(value) && Object.keys(value).sort().join() === [...keys].sort().join();
}
function text(value, bound) {
  return typeof value === "string" && value.length <= bound && !value.includes("\0");
}
function wildcardCatalogKey(identifier) {
  const parts = identifier.split("/").filter((part) => part && part !== ".");
  if (!identifier || identifier.startsWith("/") || identifier.includes("\\") || !parts.length || parts.includes(".."))
    throw new TypeError("Invalid wildcard catalog identifier");
  return unicodeLowerCase(parts.join("/"));
}
function captureWildcardMetadata(value) {
  if (!value || typeof value !== "object" || Array.isArray(value))
    throw new TypeError("Invalid wildcard catalog");
  const input = value;
  const ready = input.status === "ready";
  if (!ready && input.status !== "pending" && input.status !== "unavailable" || !record(
    input,
    ready ? ["status", "revision", "textIdentifiers", "csvSources"] : ["status", "revision"]
  ) || !text(input.revision, 256) || !input.revision.length)
    throw new TypeError("Invalid wildcard catalog");
  const textIdentifiers = /* @__PURE__ */ new Set(), csv = /* @__PURE__ */ new Map(), builders = /* @__PURE__ */ new Map();
  let count = 0, length = 0;
  function capture(value2) {
    if (!text(value2, 1024) || ++count > 25e4 || (length += value2.length) > 8e6)
      throw new TypeError("Wildcard catalog exceeds metadata bounds");
    return value2;
  }
  if (ready) {
    if (!Array.isArray(input.textIdentifiers) || !Array.isArray(input.csvSources) || input.textIdentifiers.length + input.csvSources.length > 25e4)
      throw new TypeError("Invalid wildcard catalog rows");
    for (const identifier of input.textIdentifiers)
      textIdentifiers.add(wildcardCatalogKey(capture(identifier)));
    for (const row of input.csvSources) {
      if (!record(row, ["identifier", "columns"]) || !Array.isArray(row.columns) || row.columns.length > 25e4)
        throw new TypeError("Invalid wildcard CSV metadata");
      const key = wildcardCatalogKey(capture(row.identifier)), lookup = builders.get(key) ?? /* @__PURE__ */ new Map();
      builders.set(key, lookup);
      for (const value2 of row.columns) {
        const column = strip(capture(value2));
        if (column && !lookup.has(unicodeLowerCase(column)))
          lookup.set(unicodeLowerCase(column), column);
      }
    }
  }
  for (const [key, lookup] of builders)
    if (lookup.size) csv.set(key, { columns: Object.freeze([...lookup.values()]), lookup });
  return {
    status: input.status,
    revision: input.revision,
    textIdentifiers,
    csv
  };
}

// frontend/sugarprompt/services/src/wildcards/catalog.ts
var noColumns = Object.freeze([]);
var noMatches = Object.freeze({
  status: "ready",
  suggestions: Object.freeze([])
});
function score(row, prefix) {
  if (!prefix) return 100;
  if (row.identifier.startsWith(prefix)) return 0;
  if (row.foldedTag.startsWith(prefix)) return 20;
  if (row.identifier.includes(prefix)) return 200;
  if (row.foldedTag.includes(prefix)) return 220;
  return null;
}
function prepareWildcardCatalog(input) {
  const metadata = captureWildcardMetadata(input), incomplete = metadata.status === "ready" ? null : Object.freeze({ status: metadata.status });
  const rows = [];
  function add(identifier, tag, csv) {
    if (!isWildcardCompletionBody(tag, csv ? "csv" : "simple")) return;
    rows.push({
      identifier: caseFold(identifier),
      foldedTag: caseFold(tag),
      key: csv ? caseFold(tag) : identifier,
      csv,
      suggestion: Object.freeze({
        tag,
        sourceLabel: csv ? "CSV wildcard" : "TXT wildcard"
      })
    });
  }
  for (const identifier of [...metadata.textIdentifiers].sort(compareScalarText))
    add(identifier, identifier, false);
  for (const [identifier, entry] of [...metadata.csv].sort((a, b) => compareScalarText(a[0], b[0])))
    for (const column of entry.columns) add(identifier, `csv:${identifier}:${column}`, true);
  const index = new ScalarSubstringIndex(rows, (row) => [row.identifier, row.foldedTag]), memo = /* @__PURE__ */ new Map();
  return Object.freeze({
    revision: metadata.revision,
    status: metadata.status,
    /** Only completed snapshots establish missing state; CSV spelling is captured at preparation. */
    resolve(reference) {
      if (!reference || reference.form !== "simple" && reference.form !== "csv" || typeof reference.identifier !== "string" || reference.identifier.length > 1024 || (reference.form === "simple" ? reference.csvColumn !== null : typeof reference.csvColumn !== "string" || reference.csvColumn.length > 1024))
        throw new TypeError("Invalid wildcard reference");
      if (incomplete) return incomplete;
      const key = wildcardCatalogKey(reference.identifier);
      if (reference.form === "simple")
        return metadata.textIdentifiers.has(key) ? Object.freeze({
          status: "found",
          matchedCsvColumn: null,
          availableColumns: noColumns
        }) : Object.freeze({
          status: "missing",
          reason: "source",
          availableColumns: noColumns
        });
      const csv = metadata.csv.get(key), matched = csv?.lookup.get(
        unicodeLowerCase(strip(requireValue(reference.csvColumn, "catalog")))
      );
      return matched !== void 0 ? Object.freeze({
        status: "found",
        matchedCsvColumn: matched,
        availableColumns: requireValue(csv, "catalog").columns
      }) : Object.freeze({
        status: "missing",
        reason: csv ? "column" : "source",
        availableColumns: csv?.columns ?? noColumns
      });
    },
    /** One immutable result per folded prefix/limit, capped at 64 entries/3200 suggestion references. */
    search(prefix, limit = 10) {
      if (typeof prefix !== "string" || prefix.length > 4096 || !Number.isSafeInteger(limit) || limit < 0 || limit > 50)
        throw new TypeError("Invalid wildcard search");
      if (incomplete) return incomplete;
      if (!limit) return noMatches;
      const folded = caseFold(prefix), csvOnly = folded.startsWith("csv:"), query = csvOnly ? folded.slice(4) : folded, key = JSON.stringify([folded, limit]), cached = memo.get(key);
      if (cached) {
        memo.delete(key);
        memo.set(key, cached);
        return cached;
      }
      const matches = index.candidates(query).flatMap((row) => {
        if (csvOnly && !row.csv) return [];
        const rank = score(row, query);
        return rank === null ? [] : [{ row, rank: rank + (row.csv ? 10 : 0) }];
      });
      matches.sort((a, b) => a.rank - b.rank || compareScalarText(a.row.key, b.row.key));
      const result = Object.freeze({
        status: "ready",
        suggestions: Object.freeze(matches.slice(0, limit).map((match) => match.row.suggestion))
      });
      memo.set(key, result);
      if (memo.size > 64) memo.delete(requireValue(memo.keys().next().value, "catalog"));
      return result;
    }
  });
}

// frontend/sugarprompt/services/src/wildcards/generation-digest.ts
var constants = [
  1116352408,
  1899447441,
  3049323471,
  3921009573,
  961987163,
  1508970993,
  2453635748,
  2870763221,
  3624381080,
  310598401,
  607225278,
  1426881987,
  1925078388,
  2162078206,
  2614888103,
  3248222580,
  3835390401,
  4022224774,
  264347078,
  604807628,
  770255983,
  1249150122,
  1555081692,
  1996064986,
  2554220882,
  2821834349,
  2952996808,
  3210313671,
  3336571891,
  3584528711,
  113926993,
  338241895,
  666307205,
  773529912,
  1294757372,
  1396182291,
  1695183700,
  1986661051,
  2177026350,
  2456956037,
  2730485921,
  2820302411,
  3259730800,
  3345764771,
  3516065817,
  3600352804,
  4094571909,
  275423344,
  430227734,
  506948616,
  659060556,
  883997877,
  958139571,
  1322822218,
  1537002063,
  1747873779,
  1955562222,
  2024104815,
  2227730452,
  2361852424,
  2428436474,
  2756734187,
  3204031479,
  3329325298
];
function rotate(value, bits) {
  return value >>> bits | value << 32 - bits;
}
function isWildcardUnicode(value) {
  return !/[\uD800-\uDFFF]/u.test(value);
}
function wildcardDigest(text2) {
  if (!isWildcardUnicode(text2)) throw new TypeError("Invalid wildcard digest text");
  const input = new TextEncoder().encode(text2), length = Math.ceil((input.length + 9) / 64) * 64;
  const bytes = new Uint8Array(length);
  bytes.set(input);
  bytes[input.length] = 128;
  const view = new DataView(bytes.buffer);
  view.setBigUint64(length - 8, BigInt(input.length) * 8n);
  const h = [
    1779033703,
    3144134277,
    1013904242,
    2773480762,
    1359893119,
    2600822924,
    528734635,
    1541459225
  ], words = new Uint32Array(64);
  for (let offset = 0; offset < length; offset += 64) {
    for (let i = 0; i < 16; i++) words[i] = view.getUint32(offset + i * 4);
    for (let i = 16; i < 64; i++) {
      const x = requireValue(words[i - 15], "generation-digest"), y = requireValue(words[i - 2], "generation-digest");
      words[i] = requireValue(words[i - 16], "generation-digest") + (rotate(x, 7) ^ rotate(x, 18) ^ x >>> 3) + requireValue(words[i - 7], "generation-digest") + (rotate(y, 17) ^ rotate(y, 19) ^ y >>> 10) >>> 0;
    }
    let [a, b, c, d, e, f, g, z] = h;
    for (let i = 0; i < 64; i++) {
      const t1 = z + (rotate(e, 6) ^ rotate(e, 11) ^ rotate(e, 25)) + (e & f ^ ~e & g) + requireValue(constants[i], "generation-digest") + requireValue(words[i], "generation-digest") >>> 0;
      const t2 = (rotate(a, 2) ^ rotate(a, 13) ^ rotate(a, 22)) + (a & b ^ a & c ^ b & c) >>> 0;
      z = g;
      g = f;
      f = e;
      e = d + t1 >>> 0;
      d = c;
      c = b;
      b = a;
      a = t1 + t2 >>> 0;
    }
    const working = [a, b, c, d, e, f, g, z];
    for (let i = 0; i < 8; i++)
      h[i] = requireValue(h[i], "generation-digest") + requireValue(working[i], "generation-digest") >>> 0;
  }
  return BigInt("0x" + h.map((word) => word.toString(16).padStart(8, "0")).join(""));
}

// frontend/sugarprompt/services/src/wildcards/generation-input.ts
function record2(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}
function validGenerationSeed(value) {
  return typeof value === "string" && value.length <= 80 && /^(?:0|-?[1-9][0-9]*)$/u.test(value);
}
function captureGenerationInput(value) {
  if (!record2(value) || Object.keys(value).some(
    (key) => !["revision", "sources", "seed", "entropySeed", "syntax", "enabled"].includes(key)
  ))
    return null;
  const revision = value.revision, externalSources = value.sources, seed = value.seed, entropySeed = value.entropySeed, enabled = value.enabled, externalSyntax = value.syntax;
  if (typeof revision !== "string" || !revision || revision.length > 256 || !Array.isArray(externalSources) || seed !== null && !validGenerationSeed(seed) || entropySeed !== void 0 && !validGenerationSeed(entropySeed) || seed === null && entropySeed === void 0 || enabled !== void 0 && typeof enabled !== "boolean")
    return null;
  const sourceCount = externalSources.length;
  if (!Number.isSafeInteger(sourceCount) || sourceCount < 0 || sourceCount > 1e4) return null;
  let syntax = { style: "curly" };
  if (externalSyntax !== void 0) {
    if (!record2(externalSyntax)) return null;
    const style = externalSyntax.style, keys = Object.keys(externalSyntax);
    if ((style === "curly" || style === "double-underscore") && keys.length === 1)
      syntax = { style };
    else if (style === "custom" && keys.sort().join() === "alsoRecognizeCurly,prefix,style,suffix") {
      const prefix = externalSyntax.prefix, suffix = externalSyntax.suffix, alsoRecognizeCurly = externalSyntax.alsoRecognizeCurly;
      if (typeof prefix !== "string" || typeof suffix !== "string" || typeof alsoRecognizeCurly !== "boolean")
        return null;
      syntax = { style, prefix, suffix, alsoRecognizeCurly };
    } else return null;
    try {
      wildcardDelimiters(syntax);
    } catch (error) {
      if (error instanceof TypeError || error instanceof RangeError) return null;
      throw error;
    }
  }
  const sources = /* @__PURE__ */ new Map();
  let cells = 0, units = 0, items = 0;
  function text2(value2) {
    return typeof value2 === "string" && value2.length <= 1e6 && ++cells <= 25e4 && (units += value2.length) <= 8e6;
  }
  for (let sourceIndex = 0; sourceIndex < sourceCount; sourceIndex++) {
    const source = externalSources[sourceIndex];
    if (!record2(source)) return null;
    const form = source.form, identifier = source.identifier, status = source.status;
    if (form !== "simple" && form !== "csv" || typeof identifier !== "string" || identifier.length > 1024 || status !== "ready" && status !== "unavailable")
      return null;
    const expected = status === "unavailable" ? ["status", "form", "identifier"] : ["status", "form", "identifier", "sourceId", form === "simple" ? "lines" : "rows"];
    if (Object.keys(source).sort().join() !== expected.sort().join()) return null;
    let key;
    try {
      key = JSON.stringify([form, wildcardCatalogKey(identifier)]);
    } catch (error) {
      if (error instanceof TypeError) return null;
      throw error;
    }
    if (sources.has(key)) return null;
    if (status === "unavailable") {
      sources.set(key, Object.freeze({ status: "unavailable" }));
      continue;
    }
    const sourceId = source.sourceId, values = form === "simple" ? source.lines : source.rows;
    if (typeof sourceId !== "string" || !sourceId || sourceId.length > 1024 || !isWildcardUnicode(sourceId) || !Array.isArray(values))
      return null;
    const valueCount = values.length;
    if (!Number.isSafeInteger(valueCount) || valueCount < 0 || (items += valueCount) > 25e4)
      return null;
    if (form === "simple") {
      const lines = [];
      for (let index = 0; index < valueCount; index++) {
        const line = values[index];
        if (!text2(line)) return null;
        lines.push(line);
      }
      sources.set(
        key,
        Object.freeze({
          status: "ready",
          sourceId,
          values: Object.freeze(lines)
        })
      );
    } else {
      const rows = [];
      for (let index = 0; index < valueCount; index++) {
        const row = values[index];
        if (!record2(row)) return null;
        const captured = /* @__PURE__ */ new Map();
        for (const column of Object.keys(row)) {
          const cell = row[column];
          if (column.length > 1024 || !text2(column) || !text2(cell)) return null;
          captured.set(unicodeLowerCase(strip(column)), cell);
        }
        rows.push(captured);
      }
      sources.set(
        key,
        Object.freeze({
          status: "ready",
          sourceId,
          values: Object.freeze(rows)
        })
      );
    }
  }
  return {
    revision,
    seed,
    entropySeed: entropySeed ?? null,
    enabled: enabled !== false,
    syntax: Object.freeze(syntax),
    sources
  };
}
function generationSourceKey(form, identifier) {
  return JSON.stringify([form, wildcardCatalogKey(identifier)]);
}

// frontend/sugarprompt/services/src/wildcards/generation-random.ts
var WildcardRandom = class {
  state = new Uint32Array(624);
  position = 624;
  /** Seed from absolute little-endian 32-bit words, including Python's single zero word. */
  constructor(seed) {
    let value = seed < 0n ? -seed : seed;
    const key = [];
    do {
      key.push(Number(value & 0xffffffffn));
      value >>= 32n;
    } while (value);
    this.state[0] = 19650218;
    for (let i2 = 1; i2 < 624; i2++)
      this.state[i2] = Math.imul(
        requireValue(this.state[i2 - 1], "generation-random") ^ requireValue(this.state[i2 - 1], "generation-random") >>> 30,
        1812433253
      ) + i2 >>> 0;
    let i = 1, j = 0;
    for (let k = Math.max(624, key.length); k; k--) {
      this.state[i] = (requireValue(this.state[i], "generation-random") ^ Math.imul(
        requireValue(this.state[i - 1], "generation-random") ^ requireValue(this.state[i - 1], "generation-random") >>> 30,
        1664525
      )) + requireValue(key[j], "generation-random") + j >>> 0;
      if (++i >= 624) {
        this.state[0] = requireValue(this.state[623], "generation-random");
        i = 1;
      }
      if (++j >= key.length) j = 0;
    }
    for (let k = 623; k; k--) {
      this.state[i] = (requireValue(this.state[i], "generation-random") ^ Math.imul(
        requireValue(this.state[i - 1], "generation-random") ^ requireValue(this.state[i - 1], "generation-random") >>> 30,
        1566083941
      )) - i >>> 0;
      if (++i >= 624) {
        this.state[0] = requireValue(this.state[623], "generation-random");
        i = 1;
      }
    }
    this.state[0] = 2147483648;
  }
  /** Return one tempered 32-bit word, regenerating the fixed-size state in place. */
  word() {
    if (this.position >= 624) {
      for (let i = 0; i < 624; i++) {
        const y2 = requireValue(this.state[i], "generation-random") & 2147483648 | requireValue(this.state[(i + 1) % 624], "generation-random") & 2147483647;
        this.state[i] = (requireValue(this.state[(i + 397) % 624], "generation-random") ^ y2 >>> 1 ^ (y2 & 1 ? 2567483615 : 0)) >>> 0;
      }
      this.position = 0;
    }
    let y = requireValue(this.state[this.position++], "generation-random");
    y ^= y >>> 11;
    y ^= y << 7 & 2636928640;
    y ^= y << 15 & 4022730752;
    y ^= y >>> 18;
    return y >>> 0;
  }
  /** Preserve Python's rejection-sampling bit length, including the draw consumed for one-item sources. */
  index(count) {
    if (!Number.isSafeInteger(count) || count < 1 || count > 25e4)
      throw new RangeError("Invalid wildcard candidate count");
    const bits = Math.floor(Math.log2(count)) + 1;
    for (let draw = 0; draw < 1024; draw++) {
      const value = this.word() >>> 32 - bits;
      if (value < count) return value;
    }
    throw new Error("wildcard-random-rejection-invariant");
  }
};

// frontend/sugarprompt/services/src/wildcards/generation-selection.ts
var WildcardSelectionContext = class {
  streams = /* @__PURE__ */ new Map();
  bases = /* @__PURE__ */ new Map();
  offsets = /* @__PURE__ */ new Map();
  /** Capture one injected entropy integer for null-seed selection; its seed is never reported as an authored seed. */
  entropySeed;
  /** Capture the host entropy integer for one pass, without making it an effective authored seed. */
  constructor(entropySeed) {
    this.entropySeed = entropySeed;
  }
  /** Match retained source/seed grouping and stable tagged offsets; finite context saturation is explicit. */
  select(seed, form, identifier, sourceId, count, tag) {
    if (!this.streams.has(seed)) {
      if (this.streams.size >= 64) return null;
      this.streams.set(
        seed,
        new WildcardRandom(BigInt(seed ?? requireValue(this.entropySeed, "generation-selection")))
      );
    }
    const rng = requireValue(this.streams.get(seed), "generation-selection"), key = JSON.stringify([seed, form, identifier]);
    let base = this.bases.get(key);
    if (base === void 0) {
      if (this.bases.size >= 16384) return null;
      base = rng.index(count);
      this.bases.set(key, base);
    }
    if (tag === null) return base;
    const tagKey = JSON.stringify([key, tag]);
    let offset = this.offsets.get(tagKey);
    if (offset === void 0) {
      if (this.offsets.size >= 16384) return null;
      offset = seed === null ? rng.index(count) : Number(wildcardDigest(`${seed}:${sourceId}:${tag}`) % BigInt(count));
      this.offsets.set(tagKey, offset);
    }
    return (base + offset) % count;
  }
  /** Retire every random stream and coordinated choice at the generation pass boundary. */
  dispose() {
    this.streams.clear();
    this.bases.clear();
    this.offsets.clear();
  }
};

// frontend/sugarprompt/services/src/wildcards/generation.ts
var GenerationPass = class {
  disposed = false;
  requests = 0;
  context;
  input;
  /** One immutable revision defines the entire pass; changing candidates requires preparing a new pass. */
  constructor(input) {
    this.input = input;
    this.context = new WildcardSelectionContext(input.entropySeed);
  }
  /** Resolve full prompts with the same seed/source coordination across calls, preserving exact source separately. */
  resolve(source, seed) {
    if (this.disposed) return Object.freeze({ status: "rejected", reason: "disposed" });
    if (typeof source !== "string" || source.length > 1e6)
      return Object.freeze({ status: "rejected", reason: "invalid-source" });
    if (seed !== void 0 && seed !== null && !validGenerationSeed(seed))
      return Object.freeze({ status: "rejected", reason: "invalid-seed" });
    if (++this.requests > 1024)
      return Object.freeze({ status: "rejected", reason: "request-limit" });
    const effectiveSeed = seed ?? this.input.seed;
    let current = source;
    const details = [], issues = [];
    const finish = (status) => Object.freeze({
      status,
      revision: this.input.revision,
      sourceText: source,
      resolvedText: current,
      replacements: Object.freeze(
        details.map((item) => Object.freeze([item.outerText, item.value]))
      ),
      replacementDetails: Object.freeze(details),
      issues: Object.freeze(issues)
    });
    if (!this.input.enabled) return finish("disabled");
    for (let iteration = 0; iteration < 25; iteration++) {
      const spans = parsePrompt(current, this.input.syntax).wildcardSpans;
      const groups = /* @__PURE__ */ new Map(), failed = /* @__PURE__ */ new Set();
      const replacements = [];
      const issue = (span, reason, inputIteration = iteration) => {
        issues.push(
          Object.freeze({
            reason,
            iteration: inputIteration,
            range: span.outerRange,
            form: span.form,
            identifier: span.identifier,
            tag: span.tag,
            csvColumn: span.csvColumn
          })
        );
      };
      let coveredTo = 0;
      for (const span of spans) {
        if (span.outerRange.from < coveredTo) continue;
        const outerText = current.slice(span.outerRange.from, span.outerRange.to);
        if (failed.has(outerText)) continue;
        let group = groups.get(outerText);
        if (!group) {
          const chosen = this.choose(span, outerText, effectiveSeed, iteration);
          if (typeof chosen === "string") {
            failed.add(outerText);
            issue(span, chosen);
            continue;
          }
          group = { span, ranges: [], detail: chosen };
          groups.set(outerText, group);
        }
        group.ranges.push(span.outerRange);
        replacements.push({ ...span.outerRange, value: group.detail.value });
        coveredTo = span.outerRange.to;
      }
      const iterationDetails = [...groups.values()].map(
        (group) => Object.freeze({ ...group.detail, ranges: Object.freeze(group.ranges) })
      );
      if (!replacements.length) break;
      let cursor = 0, next = "";
      for (const replacement of replacements) {
        if (replacement.from < cursor) throw new Error("wildcard-replacement-overlap-invariant");
        next += current.slice(cursor, replacement.from) + replacement.value;
        cursor = replacement.to;
        if (next.length > 8e6) break;
      }
      next += current.slice(cursor);
      if (next.length > 8e6) {
        for (const { span } of groups.values()) issue(span, "output-limit");
        break;
      }
      details.push(...iterationDetails);
      if (next === current) {
        for (const { span } of groups.values()) issue(span, "iteration-limit");
        break;
      }
      current = next;
      if (iteration === 24)
        for (const span of parsePrompt(current, this.input.syntax).wildcardSpans)
          issue(span, "iteration-limit", 25);
    }
    return finish(issues.length ? "partial" : "resolved");
  }
  /** Resolve one authored token using the sole prepared candidates and selection context. */
  choose(span, outerText, effectiveSeed, iteration) {
    const candidate = this.input.sources.get(generationSourceKey(span.form, span.identifier));
    if (!candidate) {
      return "source-missing";
    }
    if (candidate.status === "unavailable") {
      return "source-unavailable";
    }
    const count = candidate.values.length;
    if (!count) {
      return "empty-source";
    }
    if (effectiveSeed !== null && span.tag !== null && !isWildcardUnicode(span.tag)) {
      return "invalid-tag";
    }
    const index = this.context.select(
      effectiveSeed,
      span.form,
      span.identifier,
      candidate.sourceId,
      count,
      span.tag
    );
    if (index === null) {
      return "context-limit";
    }
    const selected = requireValue(candidate.values[index], "generation");
    let value;
    if (span.form === "simple") {
      if (typeof selected !== "string") throw new Error("wildcard-source-form-invariant");
      value = selected;
    } else {
      if (typeof selected === "string") throw new Error("wildcard-source-form-invariant");
      const cell = selected.get(
        unicodeLowerCase(strip(requireValue(span.csvColumn, "generation")))
      );
      if (cell === void 0) {
        return "column-missing";
      }
      value = cell;
    }
    return {
      outerText,
      value,
      form: span.form,
      identifier: span.identifier,
      sourceId: candidate.sourceId,
      selectedIndex: index,
      lineNumber: index + (span.form === "simple" ? 1 : 2),
      itemCount: count,
      tag: span.tag,
      csvColumn: span.csvColumn,
      seed: effectiveSeed,
      iteration
    };
  }
  /** Retire candidate references and coordination state without retaining user-content results. */
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.input.sources.clear();
    this.context.dispose();
  }
};
function prepareWildcardGeneration(value) {
  const input = captureGenerationInput(value);
  return input ? Object.freeze({ status: "ready", pass: new GenerationPass(input) }) : Object.freeze({ status: "rejected", reason: "invalid-input" });
}

// frontend/sugarprompt/services/src/completion/catalog.ts
var empty = Object.freeze([]);

// frontend/sugarprompt/services/src/completion/scene-catalog.ts
var empty2 = Object.freeze([]);

// frontend/sugarprompt/services/src/features/registry.ts
function definition(id, label, aliases, syntax = null, dependencies = [], legacyBase = false) {
  return Object.freeze({
    id,
    label,
    aliases: Object.freeze(aliases),
    syntax,
    dependencies: Object.freeze(dependencies),
    conflicts: Object.freeze([]),
    legacyBase
  });
}
var featureDefinitions = Object.freeze([
  definition("emphasis", "Emphasis weights", ["emphasis"], "emphasis"),
  definition("danbooru_url_import", "Danbooru URL import", ["danbooru_url_import"], null, [], true),
  definition("wildcard_syntax", "Wildcard syntax", ["wildcard", "wildcard_syntax"], "wildcard"),
  definition(
    "wildcard_autocomplete",
    "Wildcard autocomplete",
    ["wildcard", "wildcard_autocomplete"],
    null,
    ["wildcard_syntax"]
  ),
  definition("autocomplete_ghost_text", "Autocomplete ghost text", [], null, [], true),
  definition("lora_syntax", "LoRA syntax", ["lora", "lora_syntax"], "lora"),
  definition("lora_autocomplete", "LoRA autocomplete", ["lora", "lora_autocomplete"], null, [
    "lora_syntax"
  ]),
  definition("lora_picker", "LoRA picker", ["lora", "lora_picker"], null, ["lora_syntax"]),
  definition("lora_trigger_words", "LoRA trigger words", ["lora", "lora_trigger_words"]),
  definition("segment_reorder", "Segment reorder", ["segment_reorder"], null, [], true),
  definition("spellcheck", "Spellcheck", ["spellcheck"], null, [], true),
  definition(
    "duplicate_segment_diagnostics",
    "Duplicate segment warnings",
    ["duplicate_segment_diagnostics"],
    null,
    [],
    true
  )
]);

// frontend/sugarprompt/services/src/lora/metadata-validation.ts
var loraItemKeys = Object.freeze([
  "model",
  "title",
  "subtitle",
  "triggerWords",
  "thumbnail",
  "modelPageUrl"
]);

// frontend/sugarprompt/services/src/features/preferences.ts
var defaultPreferences = Object.freeze({
  allowed: Object.freeze(
    Object.fromEntries(featureDefinitions.map(({ id }) => [id, true]))
  ),
  wheelMode: "hover_dwell",
  resolveOnGeneration: true
});

// frontend/sugarprompt/services/src/regions/context.ts
var unresolvedConditioning = Object.freeze({
  key: null,
  mode: "unresolved"
});

export {
  prepareWildcardCatalog,
  prepareWildcardGeneration
};
