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
/**
 * Text files the person edits in the full-window editor: wildcard files and autocomplete lists.
 * Each kind supplies its files and their actions; the editor is the same for both.
 */

/** How a file's text is edited: as a wildcard of prompt values, or as a list of tags. */
export type TextAssetDocument = 'wildcard-txt' | 'wildcard-csv' | 'tag-list';

/** One file as the editor lists it. */
export interface TextAsset {
  readonly id: string;
  /** The heading it is listed under, such as "TXT Wildcards". */
  readonly group: string;
  /** Its name, such as `people/hair`. */
  readonly label: string;
  /** One line about it, such as "12 wildcards". */
  readonly subtitle: string;
  /** Whether it is in use, for kinds whose files can be turned off; null otherwise. */
  readonly enabled: boolean | null;
  readonly document: TextAssetDocument;
}

/** One way to make a new file, such as "New TXT". */
export interface TextAssetCreation {
  readonly label: string;
  /** Why a new file cannot take this name, or null when it can. */
  refusal(name: string): string | null;
  /** Make the file and return its id. */
  create(name: string): Promise<string>;
}

/** One kind of text file: its folder, its words and its actions. */
export interface TextAssetSource {
  /** The editor's title, such as "Wildcards". */
  readonly title: string;
  /** The file list's heading, such as "Wildcard files". */
  readonly listTitle: string;
  /** What the list says while there are no files. */
  readonly emptyText: string;
  readonly creations: readonly TextAssetCreation[];
  /** The files as they are now, read again. */
  assets(): Promise<readonly TextAsset[]>;
  /** A file's text; null when it is gone. */
  read(id: string): Promise<string | null>;
  write(id: string, text: string): Promise<void>;
  /** What the rename field starts with. */
  renameText(asset: TextAsset): string;
  /** Why the file cannot be renamed to this, or null when it can. */
  renameRefusal(asset: TextAsset, name: string): string | null;
  /** Rename the file and return its new id. */
  rename(asset: TextAsset, name: string): Promise<string>;
  delete(asset: TextAsset): Promise<void>;
  /** Turn a file on or off, for kinds whose files have `enabled`. */
  setEnabled?(asset: TextAsset, enabled: boolean): Promise<void>;
  /** Tell what follows these files that they changed, once the editor is applied. */
  applied(): Promise<void>;
}
