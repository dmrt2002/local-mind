#!/bin/bash

# Download ONNX format model files for fastembed
# fastembed requires ONNX, not PyTorch!

set -e

MODEL_DIR="src-tauri/models/embeddings"
MODEL_NAME="all-MiniLM-L6-v2"
BASE_URL="https://huggingface.co/sentence-transformers/${MODEL_NAME}/resolve/main"

echo "Downloading ONNX model files for fastembed..."
echo "This will download the ONNX format (required by fastembed)"

mkdir -p "${MODEL_DIR}"

# Download ONNX model file (required by fastembed)
echo "Downloading model.onnx..."
curl -L -f -o "${MODEL_DIR}/model.onnx" "${BASE_URL}/model.onnx" || {
    echo "Error: model.onnx not found. Checking for ONNX files..."
    # Sometimes the ONNX file might be in a different location
    curl -L -f -o "${MODEL_DIR}/model.onnx" "${BASE_URL}/onnx/model.onnx" || {
        echo "ONNX model not available from HuggingFace."
        echo "Fastembed needs ONNX format. You may need to convert PyTorch to ONNX."
        exit 1
    }
}

# Still need tokenizer files
echo "Downloading tokenizer files..."
curl -L -f -o "${MODEL_DIR}/tokenizer.json" "${BASE_URL}/tokenizer.json" || echo "Warning: tokenizer.json not found"
curl -L -f -o "${MODEL_DIR}/tokenizer_config.json" "${BASE_URL}/tokenizer_config.json"
curl -L -f -o "${MODEL_DIR}/vocab.txt" "${BASE_URL}/vocab.txt"
curl -L -f -o "${MODEL_DIR}/config.json" "${BASE_URL}/config.json"

echo ""
echo "✅ Download complete!"
echo "Files downloaded to: ${MODEL_DIR}"
echo ""
ls -lh "${MODEL_DIR}/" | grep -E "\.(onnx|json|txt)$"

