import {
  strip,
  unicodeLowerCase
} from "./chunk-BJZB62WU.js";

// frontend/comfyui/ui/wildcards/domain/WildcardSource.ts
function wildcardFile(path, stamp) {
  const parts = path.replaceAll("\\", "/").split("/");
  const name = parts.at(-1) ?? "";
  const dot = name.lastIndexOf(".");
  const suffix = dot > 0 ? unicodeLowerCase(name.slice(dot)) : "";
  if (path.length > 1024 || path.includes("\0") || suffix !== ".txt" && suffix !== ".csv" || parts.some((part) => part === "" || part === "." || part === ".." || part.includes(":")))
    return null;
  const normalized = parts.join("/");
  return {
    path: normalized,
    identifier: wildcardIdentifier(normalized.slice(0, normalized.length - suffix.length)),
    kind: suffix === ".txt" ? "text" : "csv",
    stamp
  };
}
function wildcardIdentifier(name) {
  return unicodeLowerCase(
    name.replaceAll("\\", "/").split("/").filter((part) => part !== "" && part !== ".").join("/")
  );
}
function textCandidates(source) {
  return source.split(/\r\n|\r|\n/u).map((line) => strip(line)).filter((line) => line !== "");
}
function csvRecords(source, limit = Number.POSITIVE_INFINITY) {
  const records = [];
  let record = [];
  let field = "";
  let state = "record";
  const endField = () => {
    record.push(field);
    field = "";
  };
  const endRecord = () => {
    records.push(record);
    record = [];
    state = "record";
  };
  for (let index = 0; index < source.length; index++) {
    const character = source.charAt(index);
    const lineBreak = character === "\n" || character === "\r";
    if (state === "quoted") {
      if (character === '"') state = "closing";
      else field += character;
      continue;
    }
    if (lineBreak) {
      if (state !== "record") endField();
      endRecord();
      if (records.length >= limit) return records;
      if (character === "\r" && source.charAt(index + 1) === "\n") index++;
      continue;
    }
    if (character === ",") {
      endField();
      state = "field";
    } else if (character === '"' && (state === "record" || state === "field")) state = "quoted";
    else if (character === '"' && state === "closing") {
      field += '"';
      state = "quoted";
    } else {
      field += character;
      state = "plain";
    }
  }
  if (state !== "record") {
    endField();
    endRecord();
  }
  return records;
}
function csvWildcard(source) {
  const [headers, ...records] = csvRecords(source);
  if (!headers?.length) return { columns: [], rows: [] };
  const names = headers.map((header) => unicodeLowerCase(strip(header)));
  const rows = records.map((record) => {
    const row = /* @__PURE__ */ Object.create(null);
    for (let index = 0; index < Math.min(names.length, record.length); index++)
      row[names[index] ?? ""] = strip(record[index] ?? "");
    return row;
  });
  return { columns: csvColumns(source), rows };
}
function csvColumns(source) {
  const seen = /* @__PURE__ */ new Set();
  return (csvRecords(source, 1)[0] ?? []).map((header) => strip(header)).filter((header) => {
    const name = unicodeLowerCase(header);
    if (!name || seen.has(name)) return false;
    seen.add(name);
    return true;
  });
}

export {
  wildcardFile,
  wildcardIdentifier,
  textCandidates,
  csvRecords,
  csvWildcard,
  csvColumns
};
