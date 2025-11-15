#!/usr/bin/env python3
"""
OCR Benchmark Suite for LocalMind Screenshot Processing
Tests different OCR engines: Tesseract, Apple Vision, Surya, PaddleOCR
"""

import time
import json
import sys
from pathlib import Path
from typing import Dict, List, Tuple, Optional
import platform

# Results storage
class BenchmarkResult:
    def __init__(self, engine: str, image_path: str):
        self.engine = engine
        self.image_path = image_path
        self.text = ""
        self.processing_time = 0.0
        self.word_count = 0
        self.char_count = 0
        self.confidence = 0.0
        self.error = None

    def to_dict(self):
        return {
            "engine": self.engine,
            "image_path": self.image_path,
            "text": self.text,
            "processing_time": self.processing_time,
            "word_count": self.word_count,
            "char_count": self.char_count,
            "confidence": self.confidence,
            "error": self.error
        }

def benchmark_tesseract(image_path: str) -> BenchmarkResult:
    """Test current Tesseract implementation"""
    result = BenchmarkResult("Tesseract", image_path)
    try:
        import pytesseract
        from PIL import Image

        start = time.time()
        img = Image.open(image_path)

        # Current LocalMind config: PSM 11, OEM 1, DPI 300
        custom_config = r'--oem 1 --psm 11 -c tessedit_char_blacklist=|©®™ -c preserve_interword_spaces=1'
        text = pytesseract.image_to_string(img, config=custom_config)

        result.processing_time = time.time() - start
        result.text = text
        result.word_count = len(text.split())
        result.char_count = len(text)

        # Get confidence data
        data = pytesseract.image_to_data(img, config=custom_config, output_type=pytesseract.Output.DICT)
        confidences = [int(conf) for conf in data['conf'] if conf != '-1']
        result.confidence = sum(confidences) / len(confidences) if confidences else 0

    except Exception as e:
        result.error = str(e)

    return result

def benchmark_tesseract_improved(image_path: str) -> BenchmarkResult:
    """Test improved Tesseract with PSM 3 (automatic)"""
    result = BenchmarkResult("Tesseract-Improved", image_path)
    try:
        import pytesseract
        from PIL import Image

        start = time.time()
        img = Image.open(image_path)

        # Improved config: PSM 3 (automatic), OEM 1, DPI 300
        custom_config = r'--oem 1 --psm 3 -c tessedit_char_blacklist=|©®™ -c preserve_interword_spaces=1'
        text = pytesseract.image_to_string(img, config=custom_config)

        result.processing_time = time.time() - start
        result.text = text
        result.word_count = len(text.split())
        result.char_count = len(text)

        data = pytesseract.image_to_data(img, config=custom_config, output_type=pytesseract.Output.DICT)
        confidences = [int(conf) for conf in data['conf'] if conf != '-1']
        result.confidence = sum(confidences) / len(confidences) if confidences else 0

    except Exception as e:
        result.error = str(e)

    return result

def benchmark_apple_vision(image_path: str) -> BenchmarkResult:
    """Test Apple Vision Framework (macOS only)"""
    result = BenchmarkResult("Apple-Vision", image_path)

    if platform.system() != "Darwin":
        result.error = "Apple Vision only available on macOS"
        return result

    try:
        from ocrmac import ocrmac

        start = time.time()
        # Try accurate mode first
        ocr_instance = ocrmac.OCR(image_path, recognition_level='accurate')
        annotations = ocr_instance.recognize()
        result.processing_time = time.time() - start

        # Extract text and confidence
        texts = []
        confidences = []
        for text, conf, bbox in annotations:
            texts.append(text)
            if conf > 0:  # LiveText returns 1.0 always
                confidences.append(conf)

        result.text = " ".join(texts)
        result.word_count = len(result.text.split())
        result.char_count = len(result.text)
        result.confidence = sum(confidences) / len(confidences) if confidences else 0

    except ImportError:
        result.error = "ocrmac not installed (pip install ocrmac)"
    except Exception as e:
        result.error = str(e)

    return result

def benchmark_apple_livetext(image_path: str) -> BenchmarkResult:
    """Test Apple LiveText Framework (macOS Sonoma+)"""
    result = BenchmarkResult("Apple-LiveText", image_path)

    if platform.system() != "Darwin":
        result.error = "Apple LiveText only available on macOS"
        return result

    try:
        from ocrmac import ocrmac

        start = time.time()
        ocr_instance = ocrmac.OCR(image_path, framework='livetext')
        annotations = ocr_instance.recognize()
        result.processing_time = time.time() - start

        texts = [text for text, conf, bbox in annotations]
        result.text = " ".join(texts)
        result.word_count = len(result.text.split())
        result.char_count = len(result.text)
        result.confidence = 1.0  # LiveText always returns 1.0

    except ImportError:
        result.error = "ocrmac not installed (pip install ocrmac)"
    except Exception as e:
        result.error = str(e)

    return result

def benchmark_surya(image_path: str) -> BenchmarkResult:
    """Test Surya OCR (transformer-based)"""
    result = BenchmarkResult("Surya", image_path)
    try:
        from PIL import Image
        from surya.ocr import run_ocr
        from surya.model.detection.model import load_model as load_det_model, load_processor as load_det_processor
        from surya.model.recognition.model import load_model as load_rec_model
        from surya.model.recognition.processor import load_processor as load_rec_processor

        # Load models (cached after first run)
        start = time.time()

        image = Image.open(image_path)
        langs = ["en"]  # English for now

        det_processor, det_model = load_det_processor(), load_det_model()
        rec_model, rec_processor = load_rec_model(), load_rec_processor()

        predictions = run_ocr([image], [langs], det_model, det_processor, rec_model, rec_processor)

        result.processing_time = time.time() - start

        # Extract text from predictions
        texts = []
        confidences = []
        for pred in predictions[0].text_lines:
            texts.append(pred.text)
            if hasattr(pred, 'confidence'):
                confidences.append(pred.confidence)

        result.text = " ".join(texts)
        result.word_count = len(result.text.split())
        result.char_count = len(result.text)
        result.confidence = sum(confidences) / len(confidences) if confidences else 0

    except ImportError:
        result.error = "surya-ocr not installed (pip install surya-ocr)"
    except Exception as e:
        result.error = str(e)

    return result

def benchmark_paddleocr(image_path: str) -> BenchmarkResult:
    """Test PaddleOCR"""
    result = BenchmarkResult("PaddleOCR", image_path)
    try:
        from paddleocr import PaddleOCR

        # Initialize with English language
        ocr = PaddleOCR(use_angle_cls=True, lang='en', show_log=False)

        start = time.time()
        ocr_result = ocr.ocr(image_path, cls=True)
        result.processing_time = time.time() - start

        # Extract text and confidence
        texts = []
        confidences = []
        for line in ocr_result[0]:
            text = line[1][0]
            conf = line[1][1]
            texts.append(text)
            confidences.append(conf)

        result.text = " ".join(texts)
        result.word_count = len(result.text.split())
        result.char_count = len(result.text)
        result.confidence = sum(confidences) / len(confidences) if confidences else 0

    except ImportError:
        result.error = "paddleocr not installed (pip install paddleocr)"
    except Exception as e:
        result.error = str(e)

    return result

def run_benchmark(image_paths: List[str]) -> Dict[str, List[BenchmarkResult]]:
    """Run all benchmarks on provided images"""

    engines = [
        ("Tesseract (Current)", benchmark_tesseract),
        ("Tesseract (Improved)", benchmark_tesseract_improved),
        ("Apple Vision", benchmark_apple_vision),
        ("Apple LiveText", benchmark_apple_livetext),
        ("Surya OCR", benchmark_surya),
        ("PaddleOCR", benchmark_paddleocr),
    ]

    results = {name: [] for name, _ in engines}

    for img_path in image_paths:
        print(f"\n{'='*60}")
        print(f"Processing: {img_path}")
        print(f"{'='*60}")

        for engine_name, engine_func in engines:
            print(f"\nTesting {engine_name}...", end=" ")
            result = engine_func(img_path)
            results[engine_name].append(result)

            if result.error:
                print(f"❌ Error: {result.error}")
            else:
                print(f"✓ {result.processing_time:.2f}s | {result.word_count} words | {result.confidence:.1f}% conf")

    return results

def print_summary(results: Dict[str, List[BenchmarkResult]]):
    """Print benchmark summary"""
    print("\n" + "="*80)
    print("BENCHMARK SUMMARY")
    print("="*80)

    for engine_name, engine_results in results.items():
        print(f"\n{engine_name}:")

        # Filter out errors
        valid_results = [r for r in engine_results if not r.error]
        if not valid_results:
            print("  ❌ No valid results (check errors above)")
            continue

        # Calculate averages
        avg_time = sum(r.processing_time for r in valid_results) / len(valid_results)
        avg_words = sum(r.word_count for r in valid_results) / len(valid_results)
        avg_chars = sum(r.char_count for r in valid_results) / len(valid_results)
        avg_conf = sum(r.confidence for r in valid_results) / len(valid_results)

        print(f"  Avg Processing Time: {avg_time:.2f}s")
        print(f"  Avg Words Extracted: {avg_words:.1f}")
        print(f"  Avg Chars Extracted: {avg_chars:.1f}")
        print(f"  Avg Confidence: {avg_conf:.1f}%")

def save_results(results: Dict[str, List[BenchmarkResult]], output_file: str):
    """Save detailed results to JSON"""
    json_data = {
        engine: [r.to_dict() for r in results_list]
        for engine, results_list in results.items()
    }

    with open(output_file, 'w') as f:
        json.dump(json_data, f, indent=2)

    print(f"\n✓ Detailed results saved to: {output_file}")

def main():
    print("LocalMind OCR Benchmark Suite")
    print("="*80)

    if len(sys.argv) < 2:
        print("Usage: python ocr_benchmark.py <image_path1> [image_path2] ...")
        print("\nExample:")
        print("  python ocr_benchmark.py test_screenshot1.png test_screenshot2.png")
        sys.exit(1)

    image_paths = sys.argv[1:]

    # Validate images exist
    for img_path in image_paths:
        if not Path(img_path).exists():
            print(f"❌ Error: Image not found: {img_path}")
            sys.exit(1)

    print(f"\nTesting {len(image_paths)} image(s):")
    for img_path in image_paths:
        print(f"  - {img_path}")

    # Run benchmarks
    results = run_benchmark(image_paths)

    # Print summary
    print_summary(results)

    # Save detailed results
    output_file = "ocr_benchmark_results.json"
    save_results(results, output_file)

    print("\n" + "="*80)
    print("✓ Benchmark complete!")
    print("="*80)

if __name__ == "__main__":
    main()
