#!/usr/bin/env python3
"""
Create a synthetic test screenshot with various text types
to simulate real screenshot content scenarios
"""

from PIL import Image, ImageDraw, ImageFont
import sys

def create_test_screenshot(output_path: str):
    """Create a test image with various text types"""

    # Create image similar to screenshot resolution
    width, height = 1200, 800
    img = Image.new('RGB', (width, height), color='white')
    draw = ImageDraw.Draw(img)

    # Try to use system fonts
    try:
        title_font = ImageFont.truetype('/System/Library/Fonts/Helvetica.ttc', 24)
        normal_font = ImageFont.truetype('/System/Library/Fonts/Helvetica.ttc', 16)
        code_font = ImageFont.truetype('/System/Library/Fonts/Courier.dfont', 14)
    except:
        # Fallback to default
        title_font = ImageFont.load_default()
        normal_font = ImageFont.load_default()
        code_font = ImageFont.load_default()

    y_position = 50

    # 1. Browser-like UI elements (should be partially filtered)
    draw.rectangle([(0, 0), (width, 40)], fill='#f0f0f0')
    draw.text((20, 10), "File Edit View History Window Help", fill='black', font=normal_font)
    y_position = 60

    # 2. Title text
    draw.text((50, y_position), "LocalMind - Screenshot Testing", fill='#333', font=title_font)
    y_position += 50

    # 3. Main content paragraph (should be kept)
    content = [
        "LocalMind is a productivity application that captures and organizes",
        "screenshots, terminal commands, and clipboard snippets. It uses",
        "optical character recognition (OCR) to extract text from screenshots",
        "and provides intelligent categorization using machine learning models."
    ]

    for line in content:
        draw.text((50, y_position), line, fill='#000', font=normal_font)
        y_position += 25

    y_position += 20

    # 4. Code block (should be kept with structure)
    draw.rectangle([(50, y_position), (width-50, y_position+120)], fill='#282c34')
    y_position += 10

    code_lines = [
        "fn extract_text(image_path: &str) -> Result<String> {",
        "    let img = image::open(image_path)?;",
        "    let text = tesseract::ocr(&img)?;",
        "    Ok(clean_text(&text))",
        "}"
    ]

    for line in code_lines:
        draw.text((60, y_position), line, fill='#abb2bf', font=code_font)
        y_position += 20

    y_position += 30

    # 5. Terminal output (should be kept)
    draw.rectangle([(50, y_position), (width-50, y_position+80)], fill='#1e1e1e')
    y_position += 10

    terminal_lines = [
        "$ cargo build --release",
        "   Compiling local-mind v0.1.0",
        "    Finished release [optimized] target(s) in 2m 34s"
    ]

    for line in terminal_lines:
        draw.text((60, y_position), line, fill='#00ff00', font=code_font)
        y_position += 20

    y_position += 30

    # 6. Meeting-like UI noise (partially filtered)
    draw.text((width-200, 100), "👤 Meeting: abc-def-ghi", fill='#666', font=normal_font)
    draw.text((width-200, 125), "🔊 Participants: 5", fill='#666', font=normal_font)

    # 7. Footer with timestamp (may be filtered)
    draw.text((50, height-40), "Screenshot captured: 2025-01-13 11:28 PM", fill='#999', font=normal_font)

    # Save
    img.save(output_path)
    print(f"✓ Test screenshot created: {output_path}")
    print(f"  Size: {width}x{height}")
    print(f"  Content types: UI elements, paragraphs, code, terminal, noise")

if __name__ == "__main__":
    output = "test_screenshot.png"
    if len(sys.argv) > 1:
        output = sys.argv[1]

    create_test_screenshot(output)
