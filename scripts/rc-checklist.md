# Cadence v1.2 macOS RC Checklist

Run this checklist against one Apple Silicon release candidate produced by `scripts/release.sh`. Record the artifact URL, commit SHA, macOS version, and tester initials with the release.

## Unsigned Artifact

- [ ] Download the DMG from the draft GitHub release; do not use the local build output.
- [ ] Confirm the downloaded DMG has `com.apple.quarantine`.
- [ ] Open the DMG and drag `Cadence.app` to `/Applications`.
- [ ] Confirm `/Applications/Cadence.app` remains quarantined.
- [ ] Run `xattr -dr com.apple.quarantine /Applications/Cadence.app`.
- [ ] Confirm quarantine is absent from the app and its nested `Contents/MacOS/cadence-mcp`.
- [ ] Launch Cadence normally from `/Applications`.

## Main Window And Variables

- [ ] The existing library loads without a blank screen or CSP error.
- [ ] Copy a prompt with no variables and confirm pasted bytes are exact.
- [ ] Copy a prompt containing repeated variables; fill them and confirm every occurrence uses the same value.
- [ ] Cancel variable filling and confirm the clipboard and usage count do not change.
- [ ] Run a Playbook with single and choice steps; confirm a successful copy advances exactly once.
- [ ] Use floating search variable filling; confirm blur does not hide it while filling and a clipboard error remains inline.

## Model Context Protocol

- [ ] In Settings, confirm the resolved path is `/Applications/Cadence.app/Contents/MacOS/cadence-mcp`.
- [ ] Register Claude Code with `claude mcp add cadence --scope user -- /Applications/Cadence.app/Contents/MacOS/cadence-mcp`.
- [ ] Run `/mcp` and confirm the Cadence server and seven read-only tools are listed.
- [ ] Invoke `/mcp__cadence__<slug>` for a prompt with variables and confirm one interpolated user message is inserted.
- [ ] Resolve `@cadence:cadence://prompt/<id>` and confirm the rendered prompt matches Cadence.
- [ ] Call `search_prompts` and confirm ordered results from the current library.
- [ ] Call MCP `record_copy` while a Cadence window is visible and confirm its usage display refreshes within 1 second.
- [ ] Register the sidecar in Codex and run a tool-list/search smoke test.
- [ ] Register the sidecar in Gemini and run a tool-list/search smoke test.
- [ ] Record whether each client exposes MCP prompts and resources; these capabilities are client-dependent.
- [ ] Re-register one client with `CADENCE_MCP_ALLOW_WRITES=1`; confirm `create_prompt` and `update_prompt_content` appear, then remove the write gate.

## Local API And Crash Recovery

- [ ] Enable the local API and confirm `api.json` exists with mode `0600`.
- [ ] Authenticate to `/api/v1/prompts` using the published port and key.
- [ ] Disable the API and confirm discovery is removed and the port refuses connections.
- [ ] Enable the API, force-quit Cadence, confirm stale `api.json` remains, then relaunch and confirm startup replaces it with a new port/key.

## Wry Fatal Exit

- [ ] On a disposable test account/database, set `PRAGMA user_version` above Cadence's supported version and launch the app.
- [ ] Confirm the native Wry startup-fatal dialog appears with the newer-schema error.
- [ ] Dismiss the dialog and confirm the Cadence process exits with code 1 rather than leaving either WebView running.
- [ ] Restore or delete the disposable database before normal testing.

## Window, Tray, And CSP

- [ ] `Cmd+Shift+P` opens floating search from another app; Enter copies and closes it.
- [ ] The tray contains Search, Open Cadence, and Quit Cadence, and each command works.
- [ ] Confirm the main and search documents use the pinned CSP and produce no CSP violations while exercising prompts, Settings, Playbooks, and search.

## Result

- [ ] Every automated gate in `scripts/verify-release.sh --unsigned` passed for this artifact.
- [ ] Every applicable manual item above passed on the same RC.
- [ ] Attach failures, logs, and reproduction steps to the release issue. Do not ship with unchecked or failed required items.
