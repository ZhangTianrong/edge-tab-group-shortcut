# Edge Store Submission Notes

## Short Description

Windows-only hovered tab-group shortcuts for Edge. Requires a separate native companion install.

## Reviewer Notes

- This add-on is **Windows-only** in its current form.
- The extension requires a separately installed native companion for hover detection.
- The extension opens `help.html` on first install and when native messaging is unavailable so the dependency is disclosed in-product.
- The GitHub setup guide is:
  - `https://github.com/ZhangTianrong/edge-tab-group-shortcut/blob/master/docs/WINDOWS_SETUP.md`

## Icon Attribution

- Flaticon License: `https://www.flaticon.com/legal`
- Tabs icons created by Freepik - Flaticon: `https://www.flaticon.com/free-icons/tabs`
- Qwerty icons created by Freepik - Flaticon: `https://www.flaticon.com/free-icons/qwerty`

## Core Shortcuts

- `Alt+Shift+W`: close the hovered tab group
- `Alt+Shift+Q`: close all other tab groups

## Packaging

Build the submission zip with:

```powershell
.\scripts\package-edge-store.ps1
```

The generated zip excludes the development `key` from `manifest.json` and includes only the extension assets required by the store package.
