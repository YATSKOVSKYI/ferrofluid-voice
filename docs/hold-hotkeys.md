# Hold-to-record keyboard combinations on Windows

In Settings, press **Record**, hold the desired key or combination (for example
Ctrl+Q), then release all keys. Capture completes only after release, so Ctrl is
not prematurely saved before Q. Escape cancels capture. Single keyboard keys,
middle/side mouse buttons and combinations with those mouse buttons remain
supported. Up to four keys can be stored; old single-key settings remain valid.

Holding the complete configured combination shows the widget and starts audio.
Releasing any required key stops audio and sends the existing stop event to the
widget, which starts transcription. The combination must be fully released
before rearming. Key auto-repeat does not start additional recordings. Extra
held keys do not activate a different combination. Left/right modifier keys are
equivalent for binding, while their physical releases are tracked separately.

The previous Windows callbacks synchronously initialized/stopped the microphone,
read application settings and changed windows. These operations could exceed
Windows' LowLevelHooksTimeout and cause the hook to be silently removed. The new
callbacks only update a small in-memory state machine and enqueue actions.
A separate worker serializes audio start/stop, preserving a quick release even
when microphone initialization is slow. Capture and settings changes use the
same state machine; changing a binding or starting capture ends its active hold.
Hotkey releases do not stop a recording started manually. An active transcription
blocks new audio initialization. Hook thread IDs and the saved foreground window
use atomics instead of unsynchronized mutable globals.

Windows guidance: [LowLevelKeyboardProc](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc).
This new combination implementation is Windows-specific; the macOS event-tap
implementation retains its existing behavior.

Regression command (close the application first; Windows SDK is required):

```powershell
./tests/test_hotkeys.ps1
```

Eight tests passed on 1 October 2026: release order, auto-repeat/rearming,
capture after full release, modifier sides, setting changes, mouse combinations,
extra modifiers, cancellation and native Windows input. The native test injects
Ctrl+F24 with both release orders and a middle-button click through real Windows input;
an intentionally unread worker queue still receives exactly one Start and Stop.
The test executable gets its own Common Controls v6 manifest because it links
native Tauri UI code. Production TypeScript and native release builds passed.

## Native Windows shortcut registration and release recovery

An installed-app microphone test reproduced missing key-up delivery after the
widget took focus. Modifier + key shortcuts now use Windows `RegisterHotKey`
with `MOD_NOREPEAT`, and a 25 ms timer reads `GetAsyncKeyState` outside callbacks
to stop when either required key is released. Windows owns keyboard activation
and suppression; low-level hooks are still used for capture and mouse/arbitrary
multi-key fallback. Registration changes are serialized on the input thread.

The saved binding was also different from Ctrl+Q; the user confirmed Ctrl+Q,
which was persisted before restarting the installed build. The actual installed
release was verified with its microphone and widget: a two-second Ctrl+Q hold
created a finalized 376,364-byte WAV and automatically started
`whisper-cli-cuda.exe`. Releasing Ctrl first while Q remained held finalized a
second 188,204-byte WAV and again invoked the widget's transcription handler.
These verify activation, microphone stop and automatic Whisper invocation,
without claiming speech accuracy on the short test audio.

`tests/test_ctrlq_recording.ps1` repeats both release orders against a real
application, checks finalized RIFF lengths, and observes the child Whisper
process. It requires Ctrl+Q configured, auto-submit disabled, a downloaded
Whisper model, and the app closed. Run with `-Executable` to test the installed
copy. No debugging port is needed. Test audio and private logs remain outside
Git. Runtime shortcut diagnostics are written to `hotkey.log` in application data.
