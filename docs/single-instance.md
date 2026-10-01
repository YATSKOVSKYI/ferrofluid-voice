# Windows application lifecycle

Windows acquires a session-scoped kernel mutex before constructing Tauri or
creating WebViews, hooks, databases or worker state. The name is based on the
stable application identifier, independent of the executable path or version.
The mutex handle lives for the full application lifetime; Windows releases it
when the process exits, including a crash. No PID file or stale lock file is used.

The first process registers the existing Tauri single-instance activation
receiver. Later launches never start Tauri: they wait up to 10 seconds for that
receiver and send the requested arguments with a two-second SendMessageTimeout.
They also recognize an older installed build's receiver. A timeout exits with
code 2 and appends a diagnostic to `%APPDATA%/Ferrofluid Voice/startup.log`; it
does not spawn a replacement or terminate an owner that may be processing audio.

The receiver queues window operations instead of performing them inside the
Windows message callback. Library and settings lookup/creation run serially on
the UI thread, so concurrent activation requests reuse one window. Reopening
shows, restores and focuses the existing Library (or settings/widget). An
explicit cold launch without `--library` reveals the widget even in hold-hotkey
mode. Closing Library/settings retains the background widget and tray; Exit
from the tray or closing the widget exits the application.

The stable endpoint follows tauri-plugin-single-instance 2.x's Windows
WM_COPYDATA protocol. Recheck `instance.rs` if upgrading that plugin's IPC
implementation. Other desktop platforms continue using the Tauri plugin.

Run the native regression test with the app closed:

```powershell
./tests/test_single_instance.ps1
```

The test launches 20 processes together, verifies exactly one owner, hides and
minimizes its window and restores it by relaunching, suspends the test-owned
process to verify bounded duplicate lifetime, and force-terminates that owner
to verify automatic kernel-lock recovery. It stops only its test-owned primary
process. It also checks default widget launch and complete exit when closing
the widget with Library open. WebView2 renderer/GPU processes and active transcription workers are
normal child processes and are not additional application instances.

This protects process ownership and activation; it does not guarantee freedom
from unrelated hangs. A frozen primary remains preserved for diagnosis rather
than risking unsaved meeting edits or active transcription through automatic
termination.

Validated on Windows on 1 October 2026 using the native release executable:
20 simultaneous launches left one owner; relaunch restored hidden/minimized
windows; a suspended owner caused duplicate exit in about 2 seconds; forced
termination released ownership; default launch and full widget exit passed.
TypeScript/Vite and the native release build passed.
