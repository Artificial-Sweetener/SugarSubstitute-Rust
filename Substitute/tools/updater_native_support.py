# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Observe native exclusion and collect startup failures on owned off-screen windows."""
import ctypes
import json
import os
import time
from ctypes import wintypes as W
from pathlib import Path

from tools.splash_parity.offscreen_processes import ProcessFamily, declare, require


class _Overlapped(ctypes.Structure):
    """Native byte-range lock request; retain storage until its file handle closes."""
    _fields_ = [('internal', ctypes.c_size_t), ('high', ctypes.c_size_t),
                ('offset', W.DWORD), ('offset_high', W.DWORD), ('event', W.HANDLE)]


class SelectionWriter:
    """Hold the existing selection writer's actual OS lock without changing installation bytes."""
    def __init__(self, root):
        kernel = ctypes.WinDLL('kernel32', use_last_error=True)
        create = declare(kernel, 'CreateFileW', [W.LPCWSTR, W.DWORD, W.DWORD,
            ctypes.c_void_p, W.DWORD, W.DWORD, W.HANDLE], W.HANDLE)
        self._close = declare(kernel, 'CloseHandle', [W.HANDLE], W.BOOL)
        lock = declare(kernel, 'LockFileEx', [W.HANDLE, W.DWORD, W.DWORD, W.DWORD,
            W.DWORD, ctypes.POINTER(_Overlapped)], W.BOOL)
        self._handle = create(str(root / '.selection-state/.selection-lock'),
            0xC0000000, 3, None, 3, 0x200000, None)
        if self._handle == ctypes.c_void_p(-1).value:
            self._handle = None
            raise ctypes.WinError(ctypes.get_last_error())
        self._overlap = _Overlapped()
        try:
            require(lock(self._handle, 3, 0, 0xffffffff, 0xffffffff,
                         ctypes.byref(self._overlap)))
        except BaseException:
            self.close()
            raise

    def close(self):
        """Release only this observer's retained writer handle; closing twice is harmless."""
        if self._handle is not None:
            require(self._close(self._handle))
            self._handle = None


def require_installer_finish(report):
    """Require durable preparation without granting health or observing application creation.

    Native family retirement, shell ownership and run exclusion are checked separately by the
    observer. Existing ComfyUI listeners are unrelated to this installer-only observation.
    """
    receipt = report.get('receipt', {})
    current = report.get('selection')
    before = report.get('selection_before') or {}
    if (receipt.get('passed') is not True
            or receipt.get('application_creation_observed') is not False
            or report.get('application_root_pid') is not None or report.get('splash_hwnds')):
        raise RuntimeError('Installer Finish did not complete without application creation')
    if (not isinstance(current, dict) or 'accepted' not in current
            or current['accepted'] != before.get('accepted')
            or not isinstance(current.get('candidate'), dict)
            or current['candidate'].get('phase') != 'prepared'):
        raise RuntimeError('Installer Finish did not preserve accepted state and prepared choice')
    generation = current['candidate'].get('generation')
    if (not isinstance(generation, dict)
            or any(not isinstance(generation.get(key), str) or not generation[key]
                   for key in ('id', 'package'))):
        raise RuntimeError('Installer Finish did not identify its prepared generation')


def shortcut_launch(shortcut, installation):
    """Validate Shell-resolved launch parameters and retain its actual working directory."""
    target = Path(shortcut.TargetPath)
    directory = Path(shortcut.WorkingDirectory)
    if (not target.is_absolute() or not directory.is_absolute()
            or target.resolve() != (installation / 'sugarsubstitute-launcher.exe').resolve()
            or shortcut.Arguments or directory.resolve() != installation.resolve()):
        raise RuntimeError('Shortcut target, arguments or working directory differ from the installed entry')
    return target, directory


def duplicate_trial_ready(current, splash_hwnds):
    """Duplicate a launch only after the first trial is recorded and its splash is visible."""
    return (bool(splash_hwnds) and isinstance(current, dict)
            and isinstance(current.get('candidate'), dict)
            and current['candidate'].get('phase') == 'trial')


def observe_invocation(process, destination, timeout):
    """Publish retained process capabilities next to their creator, independent of window polling."""
    family = ProcessFamily()
    report = {'creator_pid': os.getpid()}
    published = None
    frames = destination.with_suffix('.frames')
    frames.mkdir()
    sequence = 0
    fast = Path(process.args[0]).name.lower() == 'sugarsubstitute-launcher.exe'
    def publish():
        nonlocal sequence
        report['handles'] = family.handles.copy()
        report['parents'] = family.parents.copy()
        report['process_creation_filetimes'] = family.creations.copy()
        report['exit_codes'] = family.exit_codes()
        report['alive'] = family.alive()
        sequence += 1
        publish_receipt(frames / f'{sequence:08}.json', report)
    try:
        report['entry_pid'] = family.retain_root(int(process._handle))
        until = time.monotonic() + timeout
        while time.monotonic() < until:
            family.discover()
            family.observe_images(report)
            alive = family.alive()
            changed = (tuple(family.handles), tuple(alive))
            if changed != published:
                publish()
                published = changed
            if not alive:
                report['completed'] = True
                break
            if any(Path(path).name.lower() == 'sugarsubstitute.exe'
                    for path in report.get('process_images', {}).values()):
                fast = False
            time.sleep(.001 if fast else .01)
    except Exception as cause:
        report['error'] = str(cause)
    finally:
        try:
            report['pre_fixture_cleanup_alive'] = family.alive()
            publish()
            publish_receipt(destination, report)
        finally:
            family.close()


def publish_receipt(path, report):
    """Publish each observation once; readers never contend with replacement on Windows."""
    if path.exists():
        raise RuntimeError('Process receipt is immutable')
    staged = path.with_suffix('.tmp')
    staged.write_text(json.dumps(report), encoding='utf-8')
    staged.rename(path)


def latest_family_receipt(path):
    """Observe the latest fully published immutable creator snapshot, if one exists."""
    frames = list(path.with_suffix('.frames').glob('????????.json'))
    return json.loads(max(frames).read_text()) if frames else None


def duplicate_application_exits(family):
    """Select actual executable identities from the creator observer's retained process objects."""
    return {str(pid): family.get('exit_codes', {}).get(str(pid))
            for pid, path in family.get('process_images', {}).items()
            if Path(path).name.lower() == 'sugarsubstitute.exe'}


def require_invocation(family, entry):
    """Completion requires the creator's original process family to have retired naturally."""
    if (not family or not family.get('completed') or family.get('error')
            or family.get('pre_fixture_cleanup_alive') != [] or family.get('entry_pid') != entry):
        raise RuntimeError('Entry creator did not observe complete native process retirement')


def require_duplicate_trial(report):
    """A second invocation must retire successfully without changing the first trial's result."""
    before = report.get('selection_at_duplicate')
    if not duplicate_trial_ready(before, report.get('splash_hwnds')):
        raise RuntimeError('Duplicate invocation did not overlap a visible unaccepted trial')
    if (report.get('duplicate_entry_exit') or {}).get('exit_code') != 0:
        raise RuntimeError('Duplicate invocation did not retire successfully')
    family = report.get('duplicate_family') or {}
    if (not family.get('completed') or family.get('error')
            or family.get('pre_fixture_cleanup_alive') != []
            or not family.get('entry_pid') or family['entry_pid'] != report.get('duplicate_entry_pid')
            or duplicate_application_exits(family) != report.get('duplicate_application_exit_codes')):
        raise RuntimeError('Duplicate creator did not retain complete native exit evidence')
    if not report.get('duplicate_application_exit_codes') or any(
            code != 0 for code in report['duplicate_application_exit_codes'].values()):
        raise RuntimeError('Duplicate application did not finish forwarding before original closure')
    if len(report.get('main_hwnds', [])) != 1 or len(report.get('splash_hwnds', [])) != 1:
        raise RuntimeError('Duplicate invocation did not retain one splash and one main window')
    trial = before['candidate'].get('attempt', {})
    generation = trial.get('generation')
    if (not isinstance(generation, dict) or not generation.get('id') or not generation.get('package')
            or not report.get('accepted_with_main')
            or (report.get('selection') or {}).get('accepted') != generation):
        raise RuntimeError('The original trial did not reach health acceptance')


def require_post_exit(report):
    """Require real remaining work after application custody retired, followed by preparation."""
    observed = report.get('post_exit') or {}
    if (not report.get('download_observed_before_close') or not observed.get('run_available')
            or not observed.get('backend_released') or observed.get('visible_application_windows') != 0
            or not observed.get('root_pid') or observed['root_pid'] not in observed.get('alive', [])):
        raise RuntimeError('Post-exit work did not release visible application and backend custody')
    after = report.get('selection') or {}
    before = report.get('selection_before_close') or {}
    if (not before.get('accepted') or after.get('accepted') != before['accepted']
            or (after.get('candidate') or {}).get('phase') != 'prepared'):
        raise RuntimeError('Post-exit work did not prepare an update while preserving accepted state')


def require_prepared_while_running(report):
    """A recovered download must prepare without replacing the application's accepted generation."""
    prepared = report.get('prepared_while_running')
    before = report.get('selection_before_close') or {}
    after = report.get('selection') or {}
    if (not report.get('accepted_with_main') or not isinstance(prepared, dict)
            or not prepared.get('id') or not prepared.get('package')
            or not before.get('accepted') or before['accepted'] != after.get('accepted')
            or (after.get('candidate') or {}).get('phase') != 'prepared'
            or after['candidate'].get('generation') != prepared):
        raise RuntimeError('Running application did not preserve its recovered preparation')


def capture_startup_failure(owner, window, report):
    """Reveal retry diagnostics across event-loop turns, then request normal closure.

    A retry surface always fails qualification. Retain the console before closing;
    an absent disclosure remains subject to the campaign's observation bound.
    Every native operation validates the private desktop and process ownership.
    """
    if not window['splash'] or not window['visible'] or not window['retry']:
        return False
    failures = report.setdefault('startup_failures', {})
    key = str(window['hwnd'])
    if key not in failures:
        failures[key] = {'console_requested': False, 'close_requested': False}
    failure = failures[key]
    if failure['close_requested']:
        return True
    if not window['console']:
        if not failure['console_requested']:
            owner.click_control(window['hwnd'], 1)
            failure['console_requested'] = True
        return False
    text = owner.console_tail(window['hwnd'])
    if text is None:
        return False
    failure['console_tail'] = text
    owner.close_window(window['hwnd'])
    failure['close_requested'] = True
    return True


def run_available(root, allow_missing=False):
    """Probe existing OS exclusion without creating, claiming or changing the lock."""
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    create = declare(kernel, 'CreateFileW', [W.LPCWSTR, W.DWORD, W.DWORD,
        ctypes.c_void_p, W.DWORD, W.DWORD, W.HANDLE], W.HANDLE)
    close = declare(kernel, 'CloseHandle', [W.HANDLE], W.BOOL)
    handle = create(str(root / '.selection-state/.installation-run-lock'),
        0xC0000000, 7, None, 3, 0x200000, None)
    if handle == ctypes.c_void_p(-1).value:
        error = ctypes.get_last_error()
        if allow_missing and error in (2, 3):
            return True
        if error == 32:
            return False
        raise ctypes.WinError(error)
    require(close(handle))
    return True
