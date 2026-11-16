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
/** Read wildcard files the way SugarSubstitute's Python application read them. */
import { strip, unicodeLowerCase } from '../../../../sugarprompt/core/src/index.js';

/** One TXT or CSV file of the user's wildcard folder. */
export interface WildcardFile {
  /** Path inside the wildcard folder, with forward slashes and its extension. */
  readonly path: string;
  /** What a prompt names it by: the lowercased path without its extension. */
  readonly identifier: string;
  readonly kind: 'text' | 'csv';
  /** Changes whenever the file's contents may have changed. */
  readonly stamp: string;
}

/**
 * Admit a path inside the wildcard folder as a wildcard file, or null for anything else.
 *
 * A path that leaves the folder, has an empty or dot segment, or is not `.txt` or `.csv` is no
 * wildcard. Lookup is by lowercased identifier, so a collection copied from a case-insensitive
 * file system keeps working.
 */
export function wildcardFile(path: string, stamp: string): WildcardFile | null {
  const parts = path.replaceAll('\\', '/').split('/');
  const name = parts.at(-1) ?? '';
  const dot = name.lastIndexOf('.');
  const suffix = dot > 0 ? unicodeLowerCase(name.slice(dot)) : '';
  if (
    path.length > 1024 ||
    path.includes('\0') ||
    (suffix !== '.txt' && suffix !== '.csv') ||
    parts.some((part) => part === '' || part === '.' || part === '..' || part.includes(':'))
  )
    return null;
  const normalized = parts.join('/');
  return {
    path: normalized,
    identifier: wildcardIdentifier(normalized.slice(0, normalized.length - suffix.length)),
    kind: suffix === '.txt' ? 'text' : 'csv',
    stamp,
  };
}

/** Normalize a name as a prompt writes it: collapsed path separators, no dot segments, lowercase. */
export function wildcardIdentifier(name: string): string {
  return unicodeLowerCase(
    name
      .replaceAll('\\', '/')
      .split('/')
      .filter((part) => part !== '' && part !== '.')
      .join('/'),
  );
}

/** A TXT wildcard's candidates: every line that is not blank, without surrounding whitespace. */
export function textCandidates(source: string): string[] {
  return source
    .split(/\r\n|\r|\n/u)
    .map((line) => strip(line))
    .filter((line) => line !== '');
}

/**
 * Split CSV text into records exactly as Python's `csv.reader` does with its default dialect.
 *
 * A quote opens a quoted field only at the start of a field; inside one a doubled quote is a
 * quote and a line break is data. A quote anywhere else is an ordinary character, and text after
 * a closing quote joins the field. A blank line is a record with no fields. Reading stops once
 * `limit` records are complete.
 */
export function csvRecords(source: string, limit = Number.POSITIVE_INFINITY): string[][] {
  const records: string[][] = [];
  let record: string[] = [];
  let field = '';
  let state: 'record' | 'field' | 'plain' | 'quoted' | 'closing' = 'record';
  const endField = (): void => {
    record.push(field);
    field = '';
  };
  const endRecord = (): void => {
    records.push(record);
    record = [];
    state = 'record';
  };
  for (let index = 0; index < source.length; index++) {
    const character = source.charAt(index);
    const lineBreak = character === '\n' || character === '\r';
    if (state === 'quoted') {
      if (character === '"') state = 'closing';
      else field += character;
      continue;
    }
    if (lineBreak) {
      if (state !== 'record') endField();
      endRecord();
      if (records.length >= limit) return records;
      if (character === '\r' && source.charAt(index + 1) === '\n') index++;
      continue;
    }
    if (character === ',') {
      endField();
      state = 'field';
    } else if (character === '"' && (state === 'record' || state === 'field')) state = 'quoted';
    else if (character === '"' && state === 'closing') {
      field += '"';
      state = 'quoted';
    } else {
      field += character;
      state = 'plain';
    }
  }
  if (state !== 'record') {
    endField();
    endRecord();
  }
  return records;
}

/** A CSV wildcard's column names as shown, and one candidate per record after the header. */
export interface CsvWildcard {
  /** Header names without surrounding whitespace; a repeated name counts once. */
  readonly columns: readonly string[];
  /** Each record's values by lowercased header name, without surrounding whitespace. */
  readonly rows: readonly Readonly<Record<string, string>>[];
}

/**
 * Read a CSV wildcard: the first record names the columns and every later record is a candidate.
 *
 * Values pair with headers in order and extra values or headers are dropped, so a blank line is a
 * candidate with no values. That candidate still counts when one is picked, as it did in Python.
 */
export function csvWildcard(source: string): CsvWildcard {
  const [headers, ...records] = csvRecords(source);
  if (!headers?.length) return { columns: [], rows: [] };
  const names = headers.map((header) => unicodeLowerCase(strip(header)));
  const rows = records.map((record) => {
    // A null prototype keeps a column named like an Object member an ordinary value.
    const row: Record<string, string> = Object.create(null) as Record<string, string>;
    for (let index = 0; index < Math.min(names.length, record.length); index++)
      row[names[index] ?? ''] = strip(record[index] ?? '');
    return row;
  });
  return { columns: csvColumns(source), rows };
}

/** Read only a CSV wildcard's column names, without decoding its candidates. */
export function csvColumns(source: string): string[] {
  const seen = new Set<string>();
  return (csvRecords(source, 1)[0] ?? [])
    .map((header) => strip(header))
    .filter((header) => {
      const name = unicodeLowerCase(header);
      if (!name || seen.has(name)) return false;
      seen.add(name);
      return true;
    });
}
