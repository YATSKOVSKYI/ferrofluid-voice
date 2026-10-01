// Run against Vite with playwright-cli run-code --filename=tests/manual-recording-ui.js.
async (page) => {
  await page.addInitScript(() => {
    const state = { recording: false, starts: 0, stops: 0, orphan: false, hides: 0, listeners: {} };
    const callbacks = [];
    window.recordingTest = state;
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: callback => callbacks.push(callback) - 1,
      invoke: async (cmd, args) => {
        if (cmd === 'plugin:event|listen') { state.listeners[args.event] = callbacks[args.handler]; return args.handler; }
        if (cmd === 'plugin:window|hide') state.hides++;
        if (cmd === 'get_model_status') return { exists: true, engineExists: true };
        if (cmd === 'get_hotkey_settings') return { alwaysOn: false, autoSubmit: false };
        if (cmd === 'get_recording_state') return state.recording;
        if (cmd === 'start_recording') {
          state.starts++;
          await new Promise(resolve => setTimeout(resolve, 300));
          state.recording = true;
          if (state.orphan) throw new Error('Delayed recording acknowledgement');
        }
        if (cmd === 'stop_recording') { state.stops++; state.recording = false; }
        if (cmd === 'transcribe_audio') return { text: '', language: 'ru', durationSeconds: 1 };
        return 0;
      },
    };
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  });
  await page.goto('http://127.0.0.1:1420');
  const start = page.getByRole('button', { name: 'Start recording', exact: true });
  await start.waitFor();
  await page.waitForFunction(() => window.__TAURI_INTERNALS__);
  await start.dblclick();
  await page.getByRole('button', { name: 'Stop recording', exact: true }).waitFor();
  if (await page.evaluate(() => window.recordingTest.starts) !== 1) throw new Error('Double click started multiple recordings');
  await page.getByRole('button', { name: 'Stop recording', exact: true }).click();
  await start.waitFor();
  await page.evaluate(() => { window.recordingTest.orphan = true; });
  await start.click();
  await page.waitForFunction(() => window.recordingTest.starts === 2 && window.recordingTest.recording);
  await page.waitForFunction(() => !document.querySelector('.widget-record').disabled);
  await start.click();
  await page.waitForFunction(() => window.recordingTest.stops === 2 && !window.recordingTest.recording);
  await start.waitFor();
  await page.evaluate(() => {
    const state = window.recordingTest;
    state.orphan = false;
    state.recording = true;
    void state.listeners['hotkey-start-recording']({ payload: { alreadyStarted: true } });
  });
  await page.getByRole('button', { name: 'Stop recording', exact: true }).waitFor();
  await page.evaluate(() => {
    const state = window.recordingTest;
    state.recording = false;
    void state.listeners['hotkey-stop-recording']({ payload: { alreadyStopped: true } });
  });
  await start.waitFor();
  await start.click();
  await page.getByRole('button', { name: 'Stop recording', exact: true }).waitFor();
  await page.waitForTimeout(1200); // Cross the old hotkey's delayed-hide deadline.
  if (await page.evaluate(() => window.recordingTest.hides) !== 0) throw new Error('Previous hotkey session hid new manual recording');
  await page.getByRole('button', { name: 'Stop recording', exact: true }).click();
  console.log('PASS: double-click guard, orphan recovery, and manual recording remains visible after hotkey');
}
