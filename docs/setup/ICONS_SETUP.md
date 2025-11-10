# Icon Setup Guide

## Current Status

Icons are **not required** for development builds. The `tauri.conf.json` has been configured with empty icon array for now.

## For Production Builds

You'll need icons before building the production installer.

### Quick Solution: Create Simple Placeholder Icons

```bash
cd src-tauri/icons

# Using ImageMagick (if installed)
# Create a simple colored square as placeholder
convert -size 512x512 xc:#007bff icon.png
convert icon.png -resize 32x32 32x32.png
convert icon.png -resize 128x128 128x128.png
convert icon.png -resize 256x256 128x128@2x.png

# For macOS .icns (requires iconutil)
mkdir icon.iconset
sips -z 16 16     icon.png --out icon.iconset/icon_16x16.png
sips -z 32 32     icon.png --out icon.iconset/icon_16x16@2x.png
sips -z 32 32     icon.png --out icon.iconset/icon_32x32.png
sips -z 64 64     icon.png --out icon.iconset/icon_32x32@2x.png
sips -z 128 128   icon.png --out icon.iconset/icon_128x128.png
sips -z 256 256   icon.png --out icon.iconset/icon_128x128@2x.png
sips -z 256 256   icon.png --out icon.iconset/icon_256x256.png
sips -z 512 512   icon.png --out icon.iconset/icon_256x256@2x.png
sips -z 512 512   icon.png --out icon.iconset/icon_512x512.png
iconutil -c icns icon.iconset -o icon.icns

# For Windows .ico (use online converter or ImageMagick)
convert icon.png -define icon:auto-resize=256,128,64,48,32,16 icon.ico
```

### Using Online Tools

1. Design your icon (512x512 PNG)
2. Use online converters:
   - **macOS .icns**: https://cloudconvert.com/png-to-icns
   - **Windows .ico**: https://cloudconvert.com/png-to-ico
3. Save all files to `src-tauri/icons/`

### Update tauri.conf.json

Once icons are ready, update:

```json
"icon": [
  "icons/32x32.png",
  "icons/128x128.png",
  "icons/128x128@2x.png",
  "icons/icon.icns",
  "icons/icon.ico"
]
```

## For Now (Development)

The build will work **without icons** for development. Icons are only required for production builds (installers).

---

**Note**: For development testing, icons are optional. Focus on functionality first!
