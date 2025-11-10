# Icons Directory

This directory should contain application icons for LocalMind.

## Required Files

- `icon.png` - 512x512 PNG icon
- `32x32.png` - 32x32 PNG icon
- `128x128.png` - 128x128 PNG icon
- `128x128@2x.png` - 256x256 PNG icon (for Retina displays)
- `icon.icns` - macOS icon format
- `icon.ico` - Windows icon format

## Quick Setup (Temporary)

For development, you can:
1. Use placeholder icons
2. Remove icon configuration from tauri.conf.json temporarily
3. Create simple colored squares as placeholders

## Generating Icons

You can use online tools or ImageMagick to generate icons from a source image:

```bash
# Example: Generate from a source icon (if you have one)
convert source-icon.png -resize 32x32 icons/32x32.png
convert source-icon.png -resize 128x128 icons/128x128.png
convert source-icon.png -resize 256x256 icons/128x128@2x.png
```

For now, the build will fail without icons. See ICONS_SETUP.md for detailed instructions.
