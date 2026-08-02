# Cadence v1.1 macOS RC Checklist

Run this checklist against the Apple Silicon release candidate built by
`scripts/release.sh`. Record the artifact URL, commit SHA, macOS version, and
tester initials with the release.

## Signed Artifact

- [ ] Download the DMG from the draft GitHub release. Do not test the local build output.
- [ ] Run `scripts/verify-release.sh "/path/to/Cadence.app" "/path/to/Cadence_1.1.0_aarch64.dmg"` against the downloaded artifacts.
- [ ] Apply the quarantine attribute and complete the mount, copy to `/Applications`, and launch steps printed by the verification script.
- [ ] Confirm the first launch succeeds without Control-click, Open Anyway, or other Privacy & Security intervention.

## Main Window

- [ ] Open Cadence from `/Applications`; the main window loads the existing library without a blank screen or CSP error.
- [ ] Select and copy a prompt from the main window, paste into TextEdit, and confirm the primary variant content is exact.
- [ ] Create, edit, reorder, and run a two-step Playbook, including one choice step.
- [ ] Delete a prompt referenced by the Playbook; confirm usage is shown before deletion and the runner displays a skippable missing-prompt notice.
- [ ] Create a manual collection and add then remove a prompt from the detail panel.

## Global Shortcut And Search Window

- [ ] With Cadence unfocused, press `Cmd+Shift+P`; the search window appears and its query is selected.
- [ ] Type a query, use Up and Down to select a result, press Enter, and confirm the primary variant pastes exactly into TextEdit.
- [ ] Press `Cmd+Shift+P` again while the search window is visible; the global shortcut toggles it closed.
- [ ] Reopen the search window, click another application, and confirm the search window hides on blur.
- [ ] Use the tray Search command while the search window is already visible; it remains visible and focused.

## Tray And Exit

- [ ] Confirm the tray menu contains Search, Open Cadence, and Quit Cadence.
- [ ] Hide or cover the main window, then choose Open Cadence; the main window becomes visible and focused.
- [ ] Enable the local API, choose Quit Cadence, and confirm `api.json` is removed and the recorded port refuses connections.

## Local API Setting

- [ ] Open Settings and confirm Enable local API is off on a fresh or upgraded install.
- [ ] Enable it; confirm Settings shows no error and `api.json` exists with mode `0600`.
- [ ] Use the file's port and key to call `/health` with Host `127.0.0.1:<port>` and a Bearer token; confirm HTTP 200.
- [ ] Disable it; confirm `api.json` is removed and the port refuses connections before the toggle reports off.

## Native CSP

Pinned policy:

`default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src ipc: http://ipc.localhost`

- [ ] In Safari Web Inspector for the main WebView, inspect the initial document response and confirm its Content-Security-Policy is the exact pinned policy.
- [ ] Exercise prompt list, detail, import, Settings, and Playbook builder flows while monitoring the macOS Console for Cadence WebKit CSP violations; confirm none occur.
- [ ] In Safari Web Inspector for the search WebView, inspect `search.html` and confirm the same exact policy.
- [ ] Exercise search, preview, keyboard selection, copy, show, and blur-hide while monitoring the macOS Console; confirm no CSP violations occur.

## Result

- [ ] Every item above passes on the same signed and notarized RC.
- [ ] Attach failures, Console excerpts, and reproduction steps to the release issue. Do not ship with unchecked or failed items.
