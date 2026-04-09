# Phase 1 Manual Boot Tests

Use this after building an ISO with `./build-iso.sh`.

## A. VM Boot Validation

1. Launch VM:
   - `./test-qemu.sh`
2. Observe live boot to login session.
3. Confirm autologin user reaches sway.
4. Confirm `mentalOS` launches automatically.

Expected:
- No boot loop.
- No sway crash on session start.
- `mentalOS` window appears without manual command.

## B. Core Runtime Checks

1. Open terminal inside sway.
2. Run:
   - `nmcli device status`
   - `ping -c 3 archlinux.org`
   - `firejail --version`
   - `mentalOS --help` (or run binary directly)

Expected:
- Network stack up with expected interfaces.
- DNS/connectivity works.
- Firejail available.

## C. UI + Safety Checks

1. Send a simple prompt in mentalOS.
2. Trigger command approval flow with an unapproved command.
3. Hit emergency stop (`Ctrl+Shift+Q`).
4. Test shortcuts:
   - `Ctrl+Shift+M`, `Ctrl+Shift+J`, `Ctrl+Shift+S`

Expected:
- Approval dialog appears for unknown command.
- Emergency stop returns app to idle state.
- Dialog shortcuts open expected views.

## D. Persistence Spot Check

1. Send two chats, one TODO item.
2. Reopen memory browser and tasks browser.
3. Export sync JSON from sync selection dialog.

Expected:
- Session and task entries are visible.
- Export file generated in `.memory/sync-exports`.

## E. Exit Criteria

- Boot + autologin + sway + mentalOS works.
- Network and safety controls behave correctly.
- No critical panic/crash during 15+ minute exploratory run.
