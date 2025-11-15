#!/usr/bin/env python3
"""Quick OCR comparison test - Apple Vision vs Tesseract"""

import time
import sys

def test_apple_vision(image_path):
    """Test Apple Vision Framework"""
    print("\n" + "="*60)
    print("APPLE VISION FRAMEWORK (Accurate Mode)")
    print("="*60)

    try:
        from ocrmac import ocrmac

        start = time.time()
        ocr_instance = ocrmac.OCR(image_path, recognition_level='accurate')
        annotations = ocr_instance.recognize()
        elapsed = time.time() - start

        texts = [text for text, conf, bbox in annotations]
        full_text = " ".join(texts)

        print(f"\n⏱️  Processing time: {elapsed:.3f}s")
        print(f"📊 Words extracted: {len(full_text.split())}")
        print(f"📊 Characters extracted: {len(full_text)}")
        print(f"\n📄 Extracted text:\n")
        print(full_text[:500] + ("..." if len(full_text) > 500 else ""))

        return full_text

    except Exception as e:
        print(f"❌ Error: {e}")
        return None

def test_apple_livetext(image_path):
    """Test Apple LiveText Framework"""
    print("\n" + "="*60)
    print("APPLE LIVETEXT (macOS Sonoma+)")
    print("="*60)

    try:
        from ocrmac import ocrmac

        start = time.time()
        ocr_instance = ocrmac.OCR(image_path, framework='livetext')
        annotations = ocr_instance.recognize()
        elapsed = time.time() - start

        texts = [text for text, conf, bbox in annotations]
        full_text = " ".join(texts)

        print(f"\n⏱️  Processing time: {elapsed:.3f}s")
        print(f"📊 Words extracted: {len(full_text.split())}")
        print(f"📊 Characters extracted: {len(full_text)}")
        print(f"\n📄 Extracted text:\n")
        print(full_text[:500] + ("..." if len(full_text) > 500 else ""))

        return full_text

    except Exception as e:
        print(f"❌ Error: {e}")
        return None

def test_tesseract_current(image_path):
    """Test Tesseract with current LocalMind settings (PSM 11)"""
    print("\n" + "="*60)
    print("TESSERACT (Current: PSM 11 - Sparse Text)")
    print("="*60)

    try:
        import pytesseract
        from PIL import Image

        img = Image.open(image_path)

        # Current LocalMind config
        config = r'--oem 1 --psm 11 -c tessedit_char_blacklist=|©®™ -c preserve_interword_spaces=1'

        start = time.time()
        text = pytesseract.image_to_string(img, config=config)
        elapsed = time.time() - start

        # Get confidence
        data = pytesseract.image_to_data(img, config=config, output_type=pytesseract.Output.DICT)
        confidences = [int(conf) for conf in data['conf'] if conf != '-1']
        avg_conf = sum(confidences) / len(confidences) if confidences else 0

        print(f"\n⏱️  Processing time: {elapsed:.3f}s")
        print(f"📊 Words extracted: {len(text.split())}")
        print(f"📊 Characters extracted: {len(text)}")
        print(f"📊 Avg confidence: {avg_conf:.1f}%")
        print(f"\n📄 Extracted text:\n")
        print(text[:500] + ("..." if len(text) > 500 else ""))

        return text

    except Exception as e:
        print(f"❌ Error: {e}")
        return None

def test_tesseract_improved(image_path):
    """Test Tesseract with improved settings (PSM 3)"""
    print("\n" + "="*60)
    print("TESSERACT (Improved: PSM 3 - Automatic)")
    print("="*60)

    try:
        import pytesseract
        from PIL import Image

        img = Image.open(image_path)

        # Improved config - PSM 3 for automatic segmentation
        config = r'--oem 1 --psm 3 -c tessedit_char_blacklist=|©®™ -c preserve_interword_spaces=1'

        start = time.time()
        text = pytesseract.image_to_string(img, config=config)
        elapsed = time.time() - start

        # Get confidence
        data = pytesseract.image_to_data(img, config=config, output_type=pytesseract.Output.DICT)
        confidences = [int(conf) for conf in data['conf'] if conf != '-1']
        avg_conf = sum(confidences) / len(confidences) if confidences else 0

        print(f"\n⏱️  Processing time: {elapsed:.3f}s")
        print(f"📊 Words extracted: {len(text.split())}")
        print(f"📊 Characters extracted: {len(text)}")
        print(f"📊 Avg confidence: {avg_conf:.1f}%")
        print(f"\n📄 Extracted text:\n")
        print(text[:500] + ("..." if len(text) > 500 else ""))

        return text

    except Exception as e:
        print(f"❌ Error: {e}")
        return None

def main():
    if len(sys.argv) < 2:
        print("Usage: python quick_ocr_test.py <image_path>")
        sys.exit(1)

    image_path = sys.argv[1]

    print("╔" + "="*70 + "╗")
    print("║" + " "*15 + "LocalMind OCR Quick Comparison Test" + " "*20 + "║")
    print("╚" + "="*70 + "╝")
    print(f"\nTesting image: {image_path}\n")

    # Run tests
    results = {}
    results['apple_vision'] = test_apple_vision(image_path)
    results['apple_livetext'] = test_apple_livetext(image_path)
    results['tesseract_current'] = test_tesseract_current(image_path)
    results['tesseract_improved'] = test_tesseract_improved(image_path)

    # Summary
    print("\n" + "="*70)
    print("SUMMARY")
    print("="*70)

    for name, text in results.items():
        if text:
            words = len(text.split())
            chars = len(text)
            print(f"\n{name:25s}: {words:4d} words, {chars:5d} chars")

    # Recommendation
    print("\n" + "="*70)
    print("RECOMMENDATION")
    print("="*70)

    # Count words extracted
    word_counts = {name: len(text.split()) if text else 0 for name, text in results.items()}

    best_engine = max(word_counts, key=word_counts.get)
    print(f"\n🏆 Most text extracted: {best_engine} ({word_counts[best_engine]} words)")

    if results.get('apple_vision') or results.get('apple_livetext'):
        apple_words = max(
            word_counts.get('apple_vision', 0),
            word_counts.get('apple_livetext', 0)
        )
        tess_improved = word_counts.get('tesseract_improved', 0)

        if apple_words > tess_improved * 1.1:  # 10% better
            print("\n✓ Recommendation: Use Apple Vision Framework")
            print("  Reasons:")
            print("  - Extracts significantly more text")
            print("  - Native macOS integration (no external dependencies)")
            print("  - Fast processing speed")
            print("  - Better for screenshot/UI text")
        elif tess_improved > apple_words * 1.1:
            print("\n✓ Recommendation: Use Tesseract (with PSM 3)")
            print("  Reasons:")
            print("  - Extracts more text than Apple Vision")
            print("  - Already integrated in codebase")
            print("  - Cross-platform support")
        else:
            print("\n✓ Recommendation: Both engines perform similarly")
            print("  - Apple Vision: Better for UI/screenshot text, native integration")
            print("  - Tesseract PSM 3: Better cross-platform, more configurable")

    print("\n" + "="*70)
    print("✓ Test complete!")
    print("="*70 + "\n")

if __name__ == "__main__":
    main()
