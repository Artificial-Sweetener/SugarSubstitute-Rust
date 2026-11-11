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
/** One snapshot owns candidate enumeration, exact identities, capabilities and indexed lookup. */
import { requireValue } from '../required.js';
import { physicalLines } from '../prompt/physical-lines.js';
import { freezeStructure } from '../prompt/text.js';
import { parseWildcardCsv } from './csv.js';
import type {
  PromptDocumentKind,
  PreparedDocumentValue,
  PreparedDocumentValues,
  PreparedDocumentContainer,
  DocumentContainerOutcome,
} from './contracts.js';
import {
  prepareSourceOffsets,
  prepareIdentityMapping,
  prepareValueMapping,
  sourceRangeToDecoded,
  trimmedValueRange,
} from './value-mapping.js';
/** Prepare outside interaction paths; a matching previous snapshot returns every populated output unchanged. */
export function prepareDocumentValues(
  source: string,
  kind: PromptDocumentKind,
  previous?: PreparedDocumentValues,
): PreparedDocumentValues {
  if (previous?.source === source && previous.kind === kind) return previous;
  const sourceOffsets = prepareSourceOffsets(source);
  const values: PreparedDocumentValue[] = [];
  const containers: PreparedDocumentContainer[] = [];
  const csv = kind === 'wildcard-csv' ? parseWildcardCsv(source) : null;
  if (kind === 'wildcard-txt') {
    let lineIndex = 0;
    for (const line of physicalLines(source, 'include-eof')) {
      const mapping = prepareIdentityMapping(
        sourceOffsets,
        source.slice(line.from, line.contentTo),
        line.from,
      );
      let value: PreparedDocumentValue | null = null;
      const decodedRange = trimmedValueRange(mapping.text);
      if (decodedRange.from < decodedRange.to) {
        const text = mapping.text.slice(decodedRange.from, decodedRange.to);
        const from = line.from + decodedRange.from;
        value = {
          id: `txt-line:${lineIndex}`,
          mapping: prepareIdentityMapping(sourceOffsets, text, from),
          decodedRange,
          location: { kind: 'txt', line: lineIndex },
        };
        values.push(value);
      }
      containers.push({
        id: `txt-container:${lineIndex}`,
        storageRange: { from: line.from, to: line.contentTo },
        decodedRange: { from: 0, to: mapping.text.length },
        valueRange: decodedRange,
        mapping,
        value,
        editability: 'editable',
        location: { kind: 'txt', line: lineIndex, record: line },
      });
      lineIndex++;
    }
  } else if (csv) {
    csv.records.forEach((record, row) => {
      record.forEach((cell, column) => {
        const mapping = csv.valid
          ? prepareValueMapping(
              sourceOffsets,
              cell.value,
              cell.characterRanges,
              cell.range.from + Number(cell.quoted),
            )
          : null;
        let value: PreparedDocumentValue | null = null;
        const decodedRange = trimmedValueRange(cell.value);
        if (csv.valid && row > 0) {
          const characterRanges = cell.characterRanges.slice(decodedRange.from, decodedRange.to);
          const anchor =
            cell.characterRanges[decodedRange.from]?.from ??
            cell.characterRanges.at(-1)?.to ??
            cell.range.from + Number(cell.quoted);
          value = {
            id: `csv-cell:${row}:${column}`,
            mapping: prepareValueMapping(
              sourceOffsets,
              cell.value.slice(decodedRange.from, decodedRange.to),
              characterRanges,
              anchor,
            ),
            decodedRange,
            location: { kind: 'csv', row, column, cell },
          };
          values.push(value);
        }
        containers.push({
          id: `csv-container:${row}:${column}`,
          storageRange: cell.range,
          decodedRange: { from: 0, to: cell.value.length },
          valueRange: decodedRange,
          mapping,
          value,
          editability: !csv.valid ? 'malformed' : row === 0 ? 'header' : 'editable',
          location: { kind: 'csv', row, column, cell },
        });
      });
    });
  } else {
    containers.push({
      id: 'prompt-container',
      storageRange: { from: 0, to: source.length },
      decodedRange: { from: 0, to: source.length },
      valueRange: { from: 0, to: source.length },
      mapping: prepareIdentityMapping(sourceOffsets, source, 0),
      value: null,
      editability: 'editable',
      location: { kind: 'prompt' },
    });
  }
  const byId: Record<string, PreparedDocumentValue> = Object.create(null) as Record<
    string,
    PreparedDocumentValue
  >;
  for (const value of values) byId[value.id] = value;
  const containersById: Record<string, PreparedDocumentContainer> = Object.create(null) as Record<
    string,
    PreparedDocumentContainer
  >;
  const containerOutcomes: Record<string, DocumentContainerOutcome> = Object.create(null) as Record<
    string,
    DocumentContainerOutcome
  >;
  for (const container of containers) {
    containersById[container.id] = container;
    containerOutcomes[container.id] =
      container.editability === 'editable'
        ? { status: 'editable', container }
        : { status: 'protected', container, reason: container.editability };
  }
  return freezeStructure({
    source,
    kind,
    valid: csv?.valid ?? true,
    scenesEnabled: kind === 'prompt',
    usesStructuredPromptValues: kind === 'wildcard-csv',
    isolatesDuplicates: kind !== 'prompt',
    values,
    byId,
    csv,
    sourceOffsets,
    containers,
    containersById,
    containerOutcomes,
    containerState: !csv
      ? 'ready'
      : !csv.valid
        ? 'malformed'
        : source.length === 0
          ? 'no-data'
          : csv.records.length === 1
            ? 'header-only'
            : 'ready',
  });
}
/** Identity lookup requires current exact source/kind context; stale snapshots cannot authorize commands. */
export function documentValueById(
  prepared: PreparedDocumentValues,
  source: string,
  kind: PromptDocumentKind,
  id: string,
): PreparedDocumentValue | null {
  if (prepared.source !== source || prepared.kind !== kind || !prepared.valid) return null;
  return prepared.byId[id] ?? null;
}
/** Binary search ordered source extents; only legitimate scalar/CRLF/escape boundaries select values. */
export function documentValueAtSource(
  prepared: PreparedDocumentValues,
  source: string,
  kind: PromptDocumentKind,
  position: number,
): PreparedDocumentValue | null {
  if (
    prepared.source !== source ||
    prepared.kind !== kind ||
    !prepared.valid ||
    !Number.isSafeInteger(position)
  )
    return null;
  let low = 0,
    high = prepared.values.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if (requireValue(prepared.values[middle], 'values').mapping.sourceRange.to < position)
      low = middle + 1;
    else high = middle;
  }
  const value = prepared.values[low];
  return value && sourceRangeToDecoded(value.mapping, { from: position, to: position })
    ? value
    : null;
}
