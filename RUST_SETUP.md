# Rust Setup Guide for Cursor Terminal

## Quick Fix (Temporary - Current Session Only)

Run this command in your Cursor terminal to add Rust to PATH for the current session:

```powershell
. .\setup-rust-path.ps1
```

Or manually:
```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
```

## Permanent Solution

To make Rust available in all Cursor terminal sessions, you have two options:

### Option 1: Create PowerShell Profile (Recommended)

1. Open PowerShell as Administrator (right-click PowerShell → Run as Administrator)
2. Run these commands:

```powershell
# Create the profile directory if it doesn't exist
$profileDir = Split-Path $PROFILE
if (-not (Test-Path $profileDir)) {
    New-Item -ItemType Directory -Path $profileDir -Force
}

# Add Rust to PATH in profile
$cargoBin = "$env:USERPROFILE\.cargo\bin"
$profileContent = @"
# Add Rust/Cargo to PATH
if (`$env:PATH -notlike `"*$cargoBin*`") {
    `$env:PATH = `"$cargoBin;`$env:PATH`"
}
"@
Add-Content -Path $PROFILE -Value $profileContent
```

3. Restart Cursor

### Option 2: Manual PATH Update

1. Press `Win + R`, type `sysdm.cpl`, press Enter
2. Go to "Advanced" tab → "Environment Variables"
3. Under "User variables", find "Path" and click "Edit"
4. Click "New" and add: `C:\Users\Godsmiracle\.cargo\bin`
5. Click OK on all dialogs
6. **Restart Cursor completely** (close all windows and reopen)

## Verify Installation

After setting up, verify everything works:

```powershell
rustup --version
cargo --version
rustc --version
rustup target list --installed
```

You should see `wasm32-unknown-unknown` in the installed targets list.

## Troubleshooting

- **If commands still don't work after restart**: Make sure you completely closed and reopened Cursor (not just the terminal)
- **If PATH is set but commands fail**: Run `. .\setup-rust-path.ps1` in your current terminal session
- **To check if Rust is installed**: Verify `C:\Users\Godsmiracle\.cargo\bin\rustup.exe` exists



