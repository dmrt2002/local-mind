#!/usr/bin/env python3
"""
Apple Vision Framework OCR for LocalMind
Uses native macOS Vision API via ocrmac wrapper
"""

import sys
import json
import time
from pathlib import Path


def extract_text_vision(image_path: str, recognition_level: str = 'accurate') -> dict:
    """
    Extract text from image using Apple Vision Framework

    Args:
        image_path: Path to image file
        recognition_level: 'fast' or 'accurate' (default: 'accurate')

    Returns:
        Dictionary with:
        - full_text: Combined text from all regions
        - annotations: List of {text, confidence, bbox}
        - processing_time: Time taken in seconds
        - word_count: Number of words extracted
        - char_count: Number of characters extracted
    """
    try:
        from ocrmac import ocrmac
    except ImportError:
        return {
            "error": "ocrmac not installed. Run: pip install ocrmac",
            "full_text": "",
            "annotations": [],
            "processing_time": 0,
            "word_count": 0,
            "char_count": 0
        }

    try:
        start_time = time.time()

        # Run OCR
        ocr_instance = ocrmac.OCR(
            image_path,
            recognition_level=recognition_level,
            language_preference=['en-US']  # Can be configured
        )
        raw_annotations = ocr_instance.recognize()

        processing_time = time.time() - start_time

        # Parse annotations
        annotations = []
        texts = []

        for text, confidence, bbox in raw_annotations:
            # bbox format: (x, y, width, height)
            annotations.append({
                "text": text,
                "confidence": float(confidence),
                "bbox": {
                    "x": float(bbox[0]) if len(bbox) > 0 else 0,
                    "y": float(bbox[1]) if len(bbox) > 1 else 0,
                    "width": float(bbox[2]) if len(bbox) > 2 else 0,
                    "height": float(bbox[3]) if len(bbox) > 3 else 0
                }
            })
            texts.append(text)

        full_text = " ".join(texts)

        return {
            "full_text": full_text,
            "annotations": annotations,
            "processing_time": processing_time,
            "word_count": len(full_text.split()),
            "char_count": len(full_text),
            "error": None
        }

    except Exception as e:
        return {
            "error": str(e),
            "full_text": "",
            "annotations": [],
            "processing_time": 0,
            "word_count": 0,
            "char_count": 0
        }


def main():
    if len(sys.argv) < 2:
        print(json.dumps({"error": "Usage: python ocr_apple.py <image_path> [recognition_level]"}))
        sys.exit(1)

    image_path = sys.argv[1]
    recognition_level = sys.argv[2] if len(sys.argv) > 2 else 'accurate'

    # Validate image exists
    if not Path(image_path).exists():
        print(json.dumps({"error": f"Image not found: {image_path}"}))
        sys.exit(1)

    # Extract text
    result = extract_text_vision(image_path, recognition_level)

    # Output JSON
    print(json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    main()
