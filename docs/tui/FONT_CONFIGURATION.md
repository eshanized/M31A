# M31A TUI Typography & Font Configuration Guide

M31A's Terminal User Interface provides a high-fidelity visual experience featuring icon glyphs powered by the [Lucide](https://lucide.dev) iconography system, complemented by robust Unicode and ASCII fallbacks.

This document details font installation, terminal emulator configuration, fallback mechanisms, and diagnostic verification across Linux, macOS, and Windows.

---

## 1. Icon Architecture & Modes

M31A supports three distinct icon rendering modes:

| Mode | Target Glyphs | Font Requirements | Ideal Environment |
|---|---|---|---|
| **`lucide`** | Dedicated Lucide glyphs (PUA `U+E038`..`U+E417`) | Bundled `lucide.ttf` or Nerd Font | Modern terminals (Kitty, Alacritty, WezTerm, iTerm2, Windows Terminal) |
| **`unicode`** | Standard UTF-8 geometric & mathematical symbols | Standard UTF-8 monospace font | Clean systems, remote SSH sessions, default terminal fonts |
| **`ascii`** | Pure 7-bit ASCII characters (`[v]`, `[x]`, `*`, `>`) | Any terminal font | Restricted/legacy terminals (`TERM=dumb`, serial consoles, CI environments) |

### Automatic Detection & Safe Fallback

On initial launch, M31A detects your terminal's display capabilities and installed font inventory:

```
+--------------------------------------------------------------------------+
| Requested Mode: Lucide                                                   |
+--------------------------------------------------------------------------+
                         |
           Is Lucide font installed OR
         M31A_FORCE_ICONS=1 configured?
           /                       \
        [YES]                      [NO]
          |                          |
    Render Lucide             Does terminal support
     PUA Glyphs                     UTF-8?
                                /             \
                             [YES]           [NO]
                               |               |
                         Render Unicode   Render ASCII
                         Symbol Fallback    Fallback
```

This ensures that **clean machines without custom fonts will never render broken tofu glyphs (`\u{FFFD}` or missing character rectangles)**.

---

## 2. Installing the Bundled Lucide Font

The official `lucide.ttf` TrueType font (v0.460.0, SHA-256 `55e868f053500d4e4298fac5515f2d4f63a10aa7b263b022ff96c126cd9e95dc`) is bundled alongside M31A release archives and embedded in the source repository at `assets/fonts/lucide.ttf`.

### Linux (GNOME / KDE / Wayland / X11)

```bash
# 1. Create user fonts directory
mkdir -p ~/.local/share/fonts

# 2. Copy the bundled font
cp /path/to/m31a/assets/fonts/lucide.ttf ~/.local/share/fonts/

# 3. Rebuild font cache
fc-cache -f -v

# 4. Verify installation
fc-list | grep -i lucide
```

### macOS

```bash
# Copy into user Fonts library
cp /path/to/m31a/assets/fonts/lucide.ttf ~/Library/Fonts/
```
Alternatively, double-click `lucide.ttf` in Finder and select **Install Font** in Font Book.

### Windows

```powershell
# Copy font to user font store
Copy-Item "assets\fonts\lucide.ttf" "$env:LOCALAPPDATA\Microsoft\Windows\Fonts\"

# Register in Windows Registry for current user
New-ItemProperty -Path "HKCU:\Software\Microsoft\Windows NT\CurrentVersion\Fonts" `
  -Name "Lucide (TrueType)" -Value "lucide.ttf" -PropertyType String -Force
```
Alternatively, right-click `lucide.ttf` in Windows Explorer and select **Install for all users** or **Install**.

---

## 3. Terminal Emulator Configuration

Modern terminal emulators allow font fallbacks so you can retain your favorite monospace coding font while mapping Lucide icons seamlessly.

### Kitty

Add the following to `~/.config/kitty/kitty.conf`:

```conf
# Primary coding font
font_family JetBrains Mono

# Map Lucide icon codepoints to the Lucide font
symbol_map U+E000-U+E500 Lucide
```

### WezTerm

Add to `~/.wezterm.lua`:

```lua
local wezterm = require 'wezterm'
local config = wezterm.config_builder()

config.font = wezterm.font_with_fallback({
  'JetBrains Mono',
  'Lucide',
  'Symbols Nerd Font',
})

return config
```

### Alacritty

Add to `~/.config/alacritty/alacritty.toml`:

```toml
[font.normal]
family = "JetBrains Mono"

# Alacritty uses system font fallback; ensure lucide.ttf is installed in font path
```

### Windows Terminal

In `settings.json`, configure your profile:

```json
{
  "profiles": {
    "defaults": {
      "font": {
        "face": "Cascadia Code",
        "axes": {
          "wght": 400
        }
      }
    }
  }
}
```

---

## 4. Configuring Icon Modes in M31A

You can explicitly control the icon mode through configuration or environment variables.

### Via `config.toml`

Edit `~/.config/m31a/config.toml` (or `$M31A_HOME/config.toml`):

```toml
[tui]
# Options: "auto" (default), "lucide", "unicode", "ascii"
icons = "auto"
```

### Via Environment Variables

- **`M31A_ICONS=lucide`**: Forces Lucide font icon mode.
- **`M31A_ICONS=unicode`**: Forces Unicode symbol fallback mode.
- **`M31A_ICONS=ascii`**: Forces pure ASCII fallback mode.
- **`M31A_FORCE_ICONS=1`**: Bypasses system font detection and assumes Lucide glyphs are supported by your terminal configuration.

---

## 5. Verification & Diagnostics (`m31a doctor`)

M31A includes automated font diagnostics built directly into the CLI doctor utility:

```bash
m31a doctor
```

The report inspects:
1. **Installed Font Discovery**: Checks `~/.local/share/fonts`, `~/.fonts`, `/usr/share/fonts`, `~/Library/Fonts`, and `%LOCALAPPDATA%\Microsoft\Windows\Fonts`.
2. **TrueType Header & SHA-256 Validation**: Validates the 676,416-byte TrueType structure and verifies the cryptographic hash against official Lucide releases.
3. **TrueType `cmap` Format 4 Table Parsing**: Inspects the subtable glyph index directly to ensure all required M31A icons are present.
4. **Terminal Glyph Capability**: Inspects `$TERM`, `$LC_ALL`, and terminal capabilities.
5. **Effective Icon Mode**: Displays the active mode chosen by the runtime.

Example Output:
```
14. Typography & Font Verification
    Installed Font: /home/user/.local/share/fonts/lucide.ttf
    File Size: 660.56 KB (Hash: 55e868f053500d4e4298fac5515f2d4f63a10aa7b263b022ff96c126cd9e95dc)
    Integrity: Valid TrueType (Format 4 Subtable)
    Coverage: 26/26 required icons verified (100.0%)
    Effective Mode: Lucide
    Status: Fully configured and verified
```

For programmatic checks, run:
```bash
m31a doctor --json | jq .font_check
```
