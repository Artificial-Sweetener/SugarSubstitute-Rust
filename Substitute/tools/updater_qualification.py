# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Qualify signed setup Finish/Open and installed startup on a fresh inactive Windows desktop.

Use an isolated installation and an output directory on the capture drive. The
real application owns health acceptance; this observer never edits its records.
Finish-only qualification observes prepared state and natural installer retirement without Open.
Faults target retained process handles created by this run. Shortcut cases resolve
the actual Windows Shell link, then explicitly launch on the private desktop;
they do not simulate an Explorer gesture. Test-forced cleanup never counts as
application shutdown. The currently configured backend must use port 8188.
Window timings are first observations, not presentation timestamps. Native process
creation times are retained separately so observer delays cannot become launch delays.
"""
import argparse
import ctypes
import json
import os
import socket
from pathlib import Path
import subprocess
import sys
import time
import traceback
from ctypes import wintypes as W

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
from tools.splash_parity.offscreen_native import PrivateDesktop
from tools.splash_parity.offscreen_processes import declare, require
from tools.updater_native_support import (
    SelectionWriter, capture_startup_failure, duplicate_trial_ready, require_duplicate_trial,
    observe_invocation, duplicate_application_exits, require_invocation, latest_family_receipt,
    require_installer_finish, require_post_exit, require_prepared_while_running,
    run_available, shortcut_launch,
)

parser = argparse.ArgumentParser()
parser.add_argument('--inside', action='store_true')
parser.add_argument('--case', default='open')
parser.add_argument('--entry', choices=['setup', 'launcher', 'shortcut'], default='setup')
parser.add_argument('--crash', action='store_true')
parser.add_argument('--setup', type=Path)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--workspace', type=Path, required=True)
parser.add_argument('--installation', type=Path, required=True)
parser.add_argument('--comfy', type=Path, help='Create a fresh installation attached to this ComfyUI directory')
parser.add_argument('--crash-before-ready', action='store_true')
parser.add_argument('--finish-only', action='store_true', help='Complete setup without requesting Open')
parser.add_argument('--hold-selection-writer-ms', type=int, default=0,
    help='Hold the existing selection writer briefly after the installed entry starts')
parser.add_argument('--duplicate-before-ready', action='store_true',
    help='Launch the installed entry again while its first trial splash is visible')
parser.add_argument('--invocation', choices=['primary', 'duplicate'], default='primary',
    help=argparse.SUPPRESS)
parser.add_argument('--download-started', type=Path,
    help='Wait for the local feed transfer signal before closing, and require independent post-exit work')
parser.add_argument('--wait-prepared', action='store_true',
    help='Keep the accepted application open until its update worker prepares a candidate')
args = parser.parse_args()
if Path(args.case).name != args.case or args.case in ('.', '..'):
    raise ValueError('Expected a case leaf')
BASE, WORK, ROOT = args.output, args.workspace, args.installation
if not all(path.is_absolute() for path in (BASE, WORK, ROOT)):
    raise ValueError('Qualification paths must be absolute')
if args.entry == 'setup' and (args.setup is None or not args.setup.is_absolute() or not args.setup.is_file()):
    raise ValueError('Setup requires an existing absolute executable path')
if args.crash_before_ready and args.crash:
    raise ValueError('An unaccepted trial fault excludes a later fault')
if args.finish_only and (args.entry != 'setup' or args.crash or args.crash_before_ready):
    raise ValueError('Finish without Open requires setup and excludes application faults')
if not 0 <= args.hold_selection_writer_ms <= 10000 or (
        args.hold_selection_writer_ms and args.entry == 'setup'):
    raise ValueError('Selection contention requires an installed entry and a hold of at most ten seconds')
if args.duplicate_before_ready and (args.entry == 'setup' or args.crash
        or args.crash_before_ready or args.hold_selection_writer_ms):
    raise ValueError('Trial duplication requires an installed entry and excludes other faults')
if args.invocation != 'primary' and (not args.inside or not args.duplicate_before_ready):
    raise ValueError('A duplicate invocation belongs only to its private-desktop helper')
if args.download_started is not None and (not args.download_started.is_absolute()
        or args.entry == 'setup' or args.crash or args.crash_before_ready
        or args.duplicate_before_ready or args.hold_selection_writer_ms):
    raise ValueError('Post-exit qualification requires an absolute feed signal and an ordinary installed launch')
if args.comfy is not None and (args.entry != 'setup' or not args.comfy.is_absolute()
        or ROOT.resolve() != (WORK / 'installed').resolve()):
    raise ValueError('Fresh setup requires ComfyUI and the qualification workspace installation')
if args.wait_prepared and (args.entry == 'setup' or args.crash or args.crash_before_ready
        or args.duplicate_before_ready or args.hold_selection_writer_ms or args.download_started):
    raise ValueError('Preparation observation requires an ordinary installed launch')
STATE = ROOT / '.selection-state'
CASE = BASE / args.case

def selection():
    """Observe a bounded snapshot without preventing the owner's atomic replacement."""
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    create = declare(kernel, 'CreateFileW', [W.LPCWSTR, W.DWORD, W.DWORD,
        ctypes.c_void_p, W.DWORD, W.DWORD, W.HANDLE], W.HANDLE)
    close = declare(kernel, 'CloseHandle', [W.HANDLE], W.BOOL)
    read = declare(kernel, 'ReadFile', [W.HANDLE, ctypes.c_void_p, W.DWORD,
        ctypes.POINTER(W.DWORD), ctypes.c_void_p], W.BOOL)
    handle = create(str(STATE / '.substitute-selection.json'), 0x80000000, 7, None, 3, 0x200000, None)
    if handle == ctypes.c_void_p(-1).value:
        error = ctypes.get_last_error()
        if args.comfy is not None and error in (2, 3):
            return None
        if error in (32, 33):
            return None
        raise ctypes.WinError(error)
    try:
        buffer = ctypes.create_string_buffer(65537)
        length = W.DWORD()
        require(read(handle, buffer, len(buffer), ctypes.byref(length), None))
        if length.value == len(buffer):
            raise RuntimeError('Selection exceeded observer bounds')
        return json.loads(buffer.raw[:length.value])
    finally:
        require(close(handle))

def inside():
    """Run the real entry from an ordinary parent on the observer's private desktop."""
    working_directory = CASE
    if args.entry == 'setup':
        command = [str(args.setup), '--workspace', str(WORK)]
        if args.comfy is not None:
            mode = '--qualify-lifecycle' if args.finish_only else '--qualify-launch'
            command += [mode, str(CASE / 'receipt.json'), '--existing-comfy', str(args.comfy)]
        else:
            mode = '--qualify-maintenance' if args.finish_only else '--qualify-maintenance-launch'
            command += [mode, str(CASE / 'receipt.json'), '--native-installation', str(ROOT)]
    else:
        command = [str(ROOT / 'sugarsubstitute-launcher.exe')]
        if args.entry == 'shortcut':
            import win32com.client
            link = ROOT / 'SugarSubstitute.lnk'
            if not link.is_file():
                raise RuntimeError('Installed shortcut absent')
            shortcut = win32com.client.Dispatch('WScript.Shell').CreateShortCut(str(link))
            target, working_directory = shortcut_launch(shortcut, ROOT)
            (CASE / 'shortcut.json').write_text(json.dumps({'target':shortcut.TargetPath,
                'arguments':shortcut.Arguments,'working_directory':shortcut.WorkingDirectory,
                'method':'Windows Shell resolves actual .lnk; explicit private-desktop process launch'}))
            command = [str(target)]
    prefix = 'duplicate-' if args.invocation == 'duplicate' else ''
    with (CASE / f'{prefix}stderr.log').open('wb') as err, (CASE / f'{prefix}stdout.log').open('wb') as out:
        with subprocess.Popen(command, cwd=working_directory, stdin=subprocess.DEVNULL, stdout=out,
            stderr=err, env=dict(os.environ, RUST_LOG='info'), close_fds=True,
            creationflags=subprocess.CREATE_NO_WINDOW) as process:
            (CASE / f'{prefix}entry.json').write_text(json.dumps({'pid':process.pid, 'command':command,
                'working_directory':str(working_directory)}))
            family_name = 'duplicate-family.json' if args.invocation == 'duplicate' else 'entry-family.json'
            observe_invocation(process, CASE / family_name, 360 if args.invocation == 'duplicate' else 600)
            code = process.wait(timeout=360)
            (CASE / f'{prefix}entry-exit.json').write_text(json.dumps({'exit_code':code}))
            if code:
                raise RuntimeError(f'Entry failed: {code}')

def main():
    """Retain every descendant through health, normal closure or the requested fault."""
    CASE.mkdir()
    owner = PrivateDesktop()
    started = time.monotonic()
    scope = 'installer_finish_without_open' if args.finish_only else 'lifecycle_and_cleanup'
    report = {'passed':False, 'scope':scope, 'desktop':owner.name, 'splash_hwnds':[], 'entry':args.entry,
        'selection_before':selection(), 'forced_fault':args.crash or args.crash_before_ready,
        'selection_writer_hold_ms':args.hold_selection_writer_ms}
    closing = None
    observed_app = False
    writer = None
    writer_started = None
    duplicate_started = False
    try:
        if args.download_started is not None and args.download_started.exists():
            raise RuntimeError('Post-exit qualification requires a fresh feed transfer signal')
        if not run_available(ROOT, allow_missing=True):
            raise RuntimeError('Installation was running before this campaign')
        if not args.finish_only:
            with socket.socket() as probe:
                probe.settimeout(1)
                if probe.connect_ex(('127.0.0.1',8188)) == 0:
                    raise RuntimeError('ComfyUI port already has a listener')
        if args.hold_selection_writer_ms:
            writer = SelectionWriter(ROOT)
        helper = owner.launch(Path(sys.executable), REPO, [str(Path(__file__)), *sys.argv[1:], '--inside'])
        while time.monotonic() - started < 600:
            elapsed = time.monotonic() - started
            owner.discover()
            family_path = CASE / 'entry-family.json'
            family = latest_family_receipt(family_path)
            if family is not None:
                entry = json.loads((CASE / 'entry.json').read_text())['pid']
                owner.import_family(family, helper, entry)
            owner.observe_images(report)
            report['process_creation_filetimes'] = owner.creations.copy()
            applications = [pid for pid, path in report['process_images'].items()
                if Path(path).name.lower() == 'sugarsubstitute.exe']
            if writer is not None:
                if applications:
                    raise RuntimeError('Application started before selection writer released')
                if writer_started is None and (CASE / 'entry.json').is_file():
                    writer_started = elapsed
                    report['selection_writer_held_from_seconds'] = elapsed
                if writer_started is not None and elapsed - writer_started >= args.hold_selection_writer_ms / 1000:
                    writer.close()
                    writer = None
                    report['selection_writer_released_seconds'] = elapsed
            if applications:
                report.setdefault('application_root_pid', min(applications, key=owner.creations.__getitem__))
                report.setdefault('application_first_observed_seconds', time.monotonic() - started)
            windows = owner.windows()
            elapsed = time.monotonic() - started
            report['elapsed_seconds'] = elapsed
            current = selection()
            if current is None:
                report['selection_publication_pending_observations'] = report.get('selection_publication_pending_observations',0) + 1
                report['alive'] = owner.alive()
                (CASE / 'progress.json').write_text(json.dumps({**report,'windows':windows},indent=2))
                time.sleep(0.1)
                continue
            report['selection'] = current
            if (args.duplicate_before_ready and not duplicate_started
                    and duplicate_trial_ready(current, report['splash_hwnds'])):
                report['selection_at_duplicate'] = current
                report['duplicate_started_seconds'] = elapsed
                report['duplicate_helper_pid'] = owner.launch(Path(sys.executable), REPO,
                    [str(Path(__file__)), *sys.argv[1:], '--inside', '--invocation', 'duplicate'])
                duplicate_started = True
            if args.crash_before_ready and closing is None and applications:
                candidate = current.get('candidate')
                backend = any(pid != helper and Path(path).name.lower() == 'python.exe'
                    and pid in owner.alive() for pid,path in report['process_images'].items())
                if candidate and candidate['phase'] == 'trial' and backend and report['splash_hwnds']:
                    report['selection_before_fault'] = current
                    report['faulted_pid'] = report['application_root_pid']
                    owner.stop(report['faulted_pid'])
                    closing = elapsed
                    report['fault_seconds'] = elapsed
            for window in windows:
                if not window['visible']:
                    continue
                if window['cls'] == '#32770':
                    report.setdefault('dialogs', []).append(owner.dialog_text(window['hwnd']))
                    owner.close_window(window['hwnd'])
                    if not args.duplicate_before_ready:
                        closing = closing or elapsed
                elif window['splash']:
                    observed_app = True
                    if window['hwnd'] not in report['splash_hwnds']:
                        report['splash_hwnds'].append(window['hwnd'])
                    report.setdefault('splash_seconds', elapsed)
                    if capture_startup_failure(owner, window, report):
                        closing = closing if closing is not None else elapsed
                elif window['title'] == 'SugarSubstitute':
                    observed_app = True
                    if window['hwnd'] not in report.setdefault('main_hwnds', []):
                        report['main_hwnds'].append(window['hwnd'])
                    report.setdefault('main_seconds', elapsed)
                    accepted = current['accepted']
                    root_image = Path(report['process_images'][report['application_root_pid']])
                    healthy = (accepted is not None and (current['candidate'] is None or args.wait_prepared)
                        and accepted['id'] == root_image.parent.name)
                    if healthy and closing is None:
                        if args.wait_prepared:
                            candidate = current.get('candidate') or {}
                            if candidate.get('phase') != 'prepared':
                                continue
                            report['prepared_while_running'] = candidate['generation']
                        if args.download_started is not None:
                            if not args.download_started.is_file():
                                continue
                            report['download_observed_before_close'] = True
                        if args.duplicate_before_ready and not duplicate_started:
                            owner.close_window(window['hwnd'])
                            raise RuntimeError('First trial became healthy before duplicate admission')
                        if args.duplicate_before_ready and not (CASE / 'duplicate-entry-exit.json').is_file():
                            continue
                        if args.duplicate_before_ready:
                            family_path = CASE / 'duplicate-family.json'
                            if not family_path.is_file():
                                continue
                            report['duplicate_family'] = json.loads(family_path.read_text())
                            report['duplicate_entry_pid'] = json.loads((CASE / 'duplicate-entry.json').read_text())['pid']
                            report['duplicate_application_exit_codes'] = duplicate_application_exits(report['duplicate_family'])
                        if run_available(ROOT):
                            raise RuntimeError('Visible application did not own installation exclusion')
                        report['accepted_with_main'] = True
                        report['selection_before_close'] = current
                        report['run_held_with_main'] = True
                        if args.crash:
                            root_pid = report['application_root_pid']
                            owner.stop(root_pid)
                            report['faulted_pid'] = root_pid
                        else:
                            owner.close_window(window['hwnd'])
                        closing = elapsed
                        report['close_seconds'] = elapsed
            report['alive'] = owner.alive()
            if (args.download_started is not None and closing is not None
                    and report.get('application_root_pid') in report['alive']
                    and not any(window['visible'] and (window['splash']
                        or window['title'] == 'SugarSubstitute') for window in windows)
                    and run_available(ROOT)):
                with socket.socket() as probe:
                    probe.settimeout(1)
                    if probe.connect_ex(('127.0.0.1', 8188)) != 0:
                        report.setdefault('post_exit', {'seconds': elapsed,
                            'root_pid': report['application_root_pid'],
                            'alive': list(report['alive']), 'run_available': True,
                            'backend_released': True, 'visible_application_windows': 0})
            (CASE / 'progress.json').write_text(json.dumps({**report,'windows':windows},indent=2))
            if not report['alive']:
                break
            # Post-exit work has its own 120-second allowance plus 15-second native retirement.
            # Observe that entire interval before the fixture declares incomplete settlement.
            settlement_budget = 150 if args.download_started is not None else 120
            if closing is not None and elapsed - closing > settlement_budget:
                raise RuntimeError('Application family did not retire within the observer budget')
            time.sleep(0.1)
        owner.discover()
        report['alive'] = owner.alive()
        report['exit_codes'] = owner.exit_codes()
        report['selection'] = selection()
        report['run_available_after_exit'] = run_available(ROOT, allow_missing=args.finish_only)
        report['backend_port_released'] = None
        if not args.finish_only:
            with socket.socket() as probe:
                probe.settimeout(1)
                report['backend_port_released'] = probe.connect_ex(('127.0.0.1',8188)) != 0
        report['entry_exit'] = json.loads((CASE / 'entry-exit.json').read_text())
        entry_pid = json.loads((CASE / 'entry.json').read_text())['pid']
        report['entry_family'] = json.loads((CASE / 'entry-family.json').read_text())
        require_invocation(report['entry_family'], entry_pid)
        if args.duplicate_before_ready:
            duplicate_exit = CASE / 'duplicate-entry-exit.json'
            report['duplicate_entry_exit'] = json.loads(duplicate_exit.read_text()) if duplicate_exit.is_file() else None
            require_duplicate_trial(report)
        if args.download_started is not None:
            require_post_exit(report)
        if args.wait_prepared:
            require_prepared_while_running(report)
        root_pid = report.get('application_root_pid')
        if root_pid in owner.creations and entry_pid in owner.creations:
            report['application_creation_after_entry_seconds'] = (
                owner.creations[root_pid] - owner.creations[entry_pid]) / 10_000_000
        report['shell_entry_files'] = {name:(ROOT/name).is_file() for name in
            ['sugarsubstitute-launcher.exe','SugarSubstitute.lnk']}
        report['application_exit_codes'] = duplicate_application_exits(report['entry_family'])
        if args.entry == 'setup':
            report['receipt'] = json.loads((CASE / 'receipt.json').read_text())
            if not args.finish_only and not report['receipt'].get('application_creation_observed'):
                raise RuntimeError('Installer never observed actual application creation')
            if args.finish_only:
                require_installer_finish(report)
                import win32com.client
                shortcut = win32com.client.Dispatch('WScript.Shell').CreateShortCut(str(ROOT / 'SugarSubstitute.lnk'))
                target, directory = shortcut_launch(shortcut, ROOT)
                report['shortcut'] = {'target': str(target), 'working_directory': str(directory)}
        expected_health = report.get('accepted_with_main')
        if args.crash_before_ready:
            expected_health = (report.get('faulted_pid') is not None
                and report['selection']['accepted'] == (report['selection_before'] or {}).get('accepted')
                and report['selection']['candidate'] is None)
        if (report.get('dialogs') or report.get('startup_failures') or report['alive']
                or (not args.finish_only and (not observed_app or not expected_health or len(report['splash_hwnds']) != 1))
                or not report['run_available_after_exit'] or report['entry_exit']['exit_code']
                or (not args.finish_only and not report['backend_port_released'])
                or (args.hold_selection_writer_ms and 'selection_writer_released_seconds' not in report)
                or not all(report['shell_entry_files'].values())
                or (not report['forced_fault'] and any(report['application_exit_codes'].values()))):
            raise RuntimeError('Application health or natural settlement proof incomplete')
        report['passed'] = True
    except BaseException as error:
        report['error'] = str(error)
        report['traceback'] = traceback.format_exc()
        raise
    finally:
        try:
            if writer is not None:
                writer.close()
        finally:
            try:
                report['process_creation_filetimes'] = owner.creations.copy()
                report['pre_fixture_cleanup_alive'] = owner.alive()
                report['pre_fixture_cleanup_exit_codes'] = owner.exit_codes()
                (CASE / 'result.json').write_text(json.dumps(report,indent=2))
            finally:
                owner.close()
    print(json.dumps(report))

if __name__ == '__main__':
    if args.inside:
        try:
            inside()
        except BaseException:
            (CASE / 'helper-error.log').write_text(traceback.format_exc())
            raise
    else:
        main()
