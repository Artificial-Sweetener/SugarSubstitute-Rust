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
/** Prove the full-window file editor keeps edits per file, saves, applies and asks before losing work. */
import { jest } from '@jest/globals';
import { TextAssetEditorModal } from '../../../frontend/comfyui/ui/text_assets/presentation/TextAssetEditorModal.js';
import type {
  TextAsset,
  TextAssetSource,
} from '../../../frontend/comfyui/ui/text_assets/domain/TextAsset.js';

afterEach(() => document.body.replaceChildren());

const settle = async (): Promise<void> => {
  for (let turn = 0; turn < 10; turn++)
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
};

/** A source over in-memory files, and an editor that is a textarea the test types into. */
function fixture(lists = false) {
  const files = new Map<string, string>([
    ['a.txt', 'alpha'],
    ['b.txt', 'beta'],
  ]);
  const enabled = new Map<string, boolean>([
    ['a.txt', true],
    ['b.txt', false],
  ]);
  const applied = jest.fn(async () => undefined);
  const writes: string[] = [];
  const source: TextAssetSource = {
    title: 'Wildcards',
    listTitle: 'Wildcard files',
    emptyText: 'No user wildcard files yet.',
    creations: [
      {
        label: 'New TXT',
        refusal: (name) => (files.has(`${name}.txt`) ? 'Taken.' : null),
        create: async (name) => {
          files.set(`${name}.txt`, '');
          return `${name}.txt`;
        },
      },
    ],
    assets: async () =>
      [...files.keys()].sort().map(
        (id): TextAsset => ({
          id,
          group: 'TXT Wildcards',
          label: id.replace('.txt', ''),
          subtitle: `${String(files.get(id)?.length)} letters`,
          enabled: lists ? (enabled.get(id) ?? true) : null,
          document: 'wildcard-txt',
        }),
      ),
    read: async (id) => files.get(id) ?? null,
    write: async (id, text) => {
      writes.push(id);
      files.set(id, text);
    },
    renameText: (asset) => asset.label,
    renameRefusal: () => null,
    rename: async (asset, name) => {
      files.set(`${name}.txt`, files.get(asset.id) ?? '');
      files.delete(asset.id);
      return `${name}.txt`;
    },
    delete: async (asset) => void files.delete(asset.id),
    ...(lists
      ? { setEnabled: async (asset: TextAsset, on: boolean) => void enabled.set(asset.id, on) }
      : {}),
    applied,
  };
  const prompt = jest.fn(async (): Promise<string | null> => null);
  const confirm = jest.fn(async () => true);
  const mounted: string[] = [];
  const modal = new TextAssetEditorModal({
    document,
    source,
    mountEditor: async (parent, file) => {
      mounted.push(file.label);
      const editor = document.createElement('textarea');
      editor.dataset.proofEditor = file.label;
      editor.value = file.source;
      editor.addEventListener('input', () => file.changed(editor.value));
      parent.append(editor);
      return { dispose: () => editor.remove() };
    },
    prompt,
    confirm,
  });
  const closed = modal.open();
  const editor = (): HTMLTextAreaElement => {
    const element = document.querySelector<HTMLTextAreaElement>('[data-proof-editor]');
    if (!element) throw new Error('No editor is mounted.');
    return element;
  };
  const type = (text: string): void => {
    editor().value = text;
    editor().dispatchEvent(new Event('input'));
  };
  const row = (id: string): HTMLElement => {
    const element = document.querySelector<HTMLElement>(`[data-asset="${id}"]`);
    if (!element) throw new Error(`No row for ${id}.`);
    return element;
  };
  const button = (label: string): HTMLButtonElement => {
    const found = [
      ...document.querySelectorAll<HTMLButtonElement>('.sc-text-assets-dialog button'),
    ].find((candidate) => candidate.textContent?.trim() === label || candidate.title === label);
    if (!found) throw new Error(`No ${label} button.`);
    return found;
  };
  return {
    files,
    writes,
    applied,
    prompt,
    confirm,
    mounted,
    closed,
    modal,
    editor,
    type,
    row,
    button,
  };
}

test('the first file opens; edits are kept per file and saved one file at a time', async () => {
  const f = fixture();
  await settle();
  expect(document.querySelector('.sc-text-assets__group')?.textContent).toBe('TXT Wildcards (2)');
  expect(f.editor().value).toBe('alpha');
  expect(f.button('Save changes').disabled).toBe(true);
  expect(f.button('Apply').disabled).toBe(true);
  f.type('alpha edited');
  expect(f.button('Save changes').disabled).toBe(false);
  expect(f.button('Apply').disabled).toBe(false);
  f.row('b.txt').click();
  await settle();
  expect(f.editor().value).toBe('beta');
  f.row('a.txt').click();
  await settle();
  // The edit survived opening another file, and the file was read only once.
  expect(f.editor().value).toBe('alpha edited');
  f.button('Save changes').click();
  await settle();
  expect(f.files.get('a.txt')).toBe('alpha edited');
  expect(f.button('Save changes').disabled).toBe(true);
  expect(f.row('a.txt').textContent).toContain('12 letters');
});

test('Discard changes puts a file back; Apply writes every changed file and closes', async () => {
  const f = fixture();
  await settle();
  f.type('changed a');
  f.button('Discard changes').click();
  await settle();
  expect(f.editor().value).toBe('alpha');
  f.type('changed a');
  f.row('b.txt').click();
  await settle();
  f.type('changed b');
  f.button('Apply').click();
  await f.closed;
  expect(f.writes.sort()).toEqual(['a.txt', 'b.txt']);
  expect(f.applied).toHaveBeenCalledTimes(1);
  expect(f.confirm).not.toHaveBeenCalled();
});

test('closing with unsaved edits asks, and stays open when the person keeps them', async () => {
  const f = fixture();
  await settle();
  f.type('unsaved');
  f.confirm.mockResolvedValueOnce(false);
  f.button('Discard').click();
  await settle();
  expect(f.confirm).toHaveBeenCalledWith(
    'Discard unsaved changes?',
    'One file has changes that are not saved. Closing discards them.',
    'Discard',
  );
  expect(document.querySelector('.sc-text-assets-dialog')?.isConnected).toBe(true);
  expect(f.editor().value).toBe('unsaved');
  f.button('Discard').click();
  await f.closed;
  expect(f.files.get('a.txt')).toBe('alpha');
  expect(f.applied).not.toHaveBeenCalled();
});

test('a new file is created from its button and opened', async () => {
  const f = fixture();
  await settle();
  f.prompt.mockResolvedValueOnce('colors');
  f.button('New TXT').click();
  await settle();
  expect(f.prompt).toHaveBeenCalledWith(
    expect.objectContaining({ title: 'New TXT', label: 'Name', confirmLabel: 'Create' }),
  );
  expect(f.files.has('colors.txt')).toBe(true);
  expect(f.row('colors.txt').getAttribute('aria-selected')).toBe('true');
});

test('a file’s menu deletes it after asking, and lists can be turned on or off', async () => {
  const f = fixture(true);
  await settle();
  f.row('b.txt').dispatchEvent(
    new MouseEvent('contextmenu', { bubbles: true, clientX: 5, clientY: 5 }),
  );
  await settle();
  const items = (): string[] =>
    [...document.querySelectorAll('[role=menuitem]')].map((item) => item.textContent?.trim() ?? '');
  expect(items()).toEqual(['Enable', 'Rename', 'Delete']);
  [...document.querySelectorAll<HTMLElement>('[role=menuitem]')][0]?.click();
  await settle();
  expect(f.row('b.txt').classList.contains('is-disabled')).toBe(false);
  f.row('b.txt').dispatchEvent(
    new MouseEvent('contextmenu', { bubbles: true, clientX: 5, clientY: 5 }),
  );
  await settle();
  [...document.querySelectorAll<HTMLElement>('[role=menuitem]')].at(-1)?.click();
  await settle();
  expect(f.confirm).toHaveBeenCalledWith('Delete', "Delete 'b'?", 'Delete');
  expect(f.files.has('b.txt')).toBe(false);
  expect(f.editor().value).toBe('alpha');
});

test('reopening the editor reuses its dialog and starts from the files, with no drafts', async () => {
  const f = fixture();
  await settle();
  f.type('changed and saved');
  f.button('Save changes').click();
  await settle();
  f.button('Discard').click();
  await f.closed;
  const reopened = f.modal.open();
  await settle();
  expect(document.querySelectorAll('.sc-text-assets-dialog')).toHaveLength(1);
  expect(f.editor().value).toBe('changed and saved');
  expect(f.button('Apply').disabled).toBe(true);
  f.button('Discard').click();
  await reopened;
});
