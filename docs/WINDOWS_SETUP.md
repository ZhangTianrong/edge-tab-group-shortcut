# Windows Setup

This extension is currently supported on **Microsoft Edge for Windows only**.

The extension package by itself is not enough. Hover detection depends on a native companion:

- `native-host.exe`
- `hover-detector.exe`
- optional: `EdgeTabGroupAHK.ahk` or `EdgeTabGroupAHK.exe` for middle-click ergonomics

## Standard Windows Install

The intended user flow is:

1. Download the Windows companion zip from the project release.
2. Extract it anywhere on your machine.
3. Run `install.ps1`.
4. Reload the extension in Edge.

From the extracted package directory:

```powershell
.\install.ps1
```

The release package already contains:

- `native-host.exe`
- `hover-detector.exe`
- `install.ps1`
- `uninstall.ps1`
- optional AHK helper files

The installer copies the native binaries into `%LOCALAPPDATA%\TabGroupShortcut\` and registers the Edge native messaging host under HKCU.

## Development From Source

If you are running the extension unpacked from this repository, build the release binaries first:

```powershell
cargo build --release --manifest-path native-host/Cargo.toml
cargo build --release --manifest-path hover-detector/Cargo.toml
```

Then run:

```powershell
.\install.ps1 -Channel development
```

## Development vs Published Builds

- `-Channel development` uses the fixed unpacked extension ID from `manifest.json`.
- After the first Edge Add-ons submission, set `installer/settings.json -> publishedExtensionId` or pass an explicit ID:

  ```powershell
  .\install.ps1 -Channel published -ExtensionId "<published-edge-extension-id>"
  ```

## Optional Middle-Click Helper

The extension officially supports keyboard shortcuts:

- `Alt+Shift+W`: close the hovered tab group
- `Alt+Shift+Q`: close all other tab groups

If you want middle-click to trigger group closing, run the AutoHotkey helper from `ahk-script/`. It is optional and not required for the extension itself.

## Detector Configuration

The hover detector behavior can be tuned with environment variables.

### Color configuration

- `TABGROUP_HOVER_EXTRA_COLORS`
  - Extra tab-group colors to treat as valid group colors.
  - Format: comma, semicolon, or whitespace-separated hex colors.
  - Example: `#5E87BC,#DB6ABA`

- `TABGROUP_HOVER_BG_COLORS`
  - Explicit background colors to treat as tab-strip background between groups.
  - Format: comma, semicolon, or whitespace-separated hex colors.
  - Example: `#000000,#333333`

### Width thresholds

- `TABGROUP_HOVER_MIN_GROUP_WIDTH`
  - Minimum width in pixels for a colored segment to count as a tab group.
  - Default: `24`

- `TABGROUP_HOVER_MIN_BG_GAP_WIDTH`
  - Minimum width in pixels for a continuous background-colored gap to split neighboring groups.
  - Default: `8`

### Diagnostics

- `TABGROUP_HOVER_DETECTOR_VERBOSE`
  - Any non-empty value enables verbose logs and diagnostic screenshots.

- `TABGROUP_NATIVE_HOST_DEBUG`
  - Any non-empty value enables native-host logging in `%LOCALAPPDATA%\TabGroupShortcut\logs\native-host.log`.

### AutoHotkey helper override

- `TABGROUP_HOVER_DETECTOR_EXE`
  - Optional override used by `EdgeTabGroupAHK.ahk` when you want it to launch a specific detector executable path.

## Suggested Screenshot Labels

For a labeled explainer image, the most useful callouts are:

- the colored tab-group header itself, to explain `TABGROUP_HOVER_EXTRA_COLORS`
- the neutral tab-strip background between groups, to explain `TABGROUP_HOVER_BG_COLORS`
- the width of a valid colored segment, to explain `TABGROUP_HOVER_MIN_GROUP_WIDTH`
- the width of the gap between groups, to explain `TABGROUP_HOVER_MIN_BG_GAP_WIDTH`

## Diagnostics

- `TABGROUP_HOVER_DETECTOR_VERBOSE=1` enables detailed detector logs and screenshots.
- Verbose diagnostics are written under `%LOCALAPPDATA%\TabGroupShortcut\logs\` and `%LOCALAPPDATA%\TabGroupShortcut\diagnostics\`.
- `TABGROUP_NATIVE_HOST_DEBUG=1` enables native-host file logging in `%LOCALAPPDATA%\TabGroupShortcut\logs\native-host.log`.

## Uninstall

To remove the native companion and its Edge registration:

```powershell
.\uninstall.ps1
```
