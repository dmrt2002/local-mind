#!/bin/bash
# Create minimal placeholder icons for Tauri build

ICON_DIR="icons"
mkdir -p "$ICON_DIR"

# Create a minimal valid PNG (1x1 blue pixel, then resize)
# Using base64 encoded minimal PNG
cat > "$ICON_DIR/temp_minimal.png" << 'EOF'
iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==
EOF

# Decode base64 to actual PNG
base64 -d <<< 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==' > "$ICON_DIR/icon.png" 2>/dev/null || {
    # Fallback: create using printf (minimal valid PNG)
    printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\nIDATx\x9cc\x00\x01\x00\x00\x05\x00\x01\r\n-\xdb\x00\x00\x00\x00IEND\xaeB`\x82' > "$ICON_DIR/icon.png"
}

# Create all required sizes using sips (macOS) or just copy
if command -v sips &> /dev/null; then
    sips -z 32 32 "$ICON_DIR/icon.png" --out "$ICON_DIR/32x32.png" 2>/dev/null || cp "$ICON_DIR/icon.png" "$ICON_DIR/32x32.png"
    sips -z 128 128 "$ICON_DIR/icon.png" --out "$ICON_DIR/128x128.png" 2>/dev/null || cp "$ICON_DIR/icon.png" "$ICON_DIR/128x128.png"
    sips -z 256 256 "$ICON_DIR/icon.png" --out "$ICON_DIR/128x128@2x.png" 2>/dev/null || cp "$ICON_DIR/icon.png" "$ICON_DIR/128x128@2x.png"
else
    # Just copy the base icon for all sizes (Tauri will scale)
    cp "$ICON_DIR/icon.png" "$ICON_DIR/32x32.png"
    cp "$ICON_DIR/icon.png" "$ICON_DIR/128x128.png"
    cp "$ICON_DIR/icon.png" "$ICON_DIR/128x128@2x.png"
fi

rm -f "$ICON_DIR/temp_minimal.png"
echo "Placeholder icons created in $ICON_DIR/"
ls -lh "$ICON_DIR"/*.png 2>/dev/null || echo "Files created (check directory)"
