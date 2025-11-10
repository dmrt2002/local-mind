#!/bin/bash

# Download TinyLlama Model for LocalMind
# This script downloads the TinyLlama-1.1B Q4_K_M model (670MB)

MODEL_DIR="src-tauri/models"
MODEL_NAME="TinyLlama-1.1B-Chat-v1.0.Q4_K_M.gguf"
MODEL_URL="https://huggingface.co/TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF/resolve/main/${MODEL_NAME}"

echo "🚀 LocalMind Model Download Script"
echo "===================================="
echo ""
echo "Model: TinyLlama-1.1B-Chat-v1.0.Q4_K_M"
echo "Size: ~670MB"
echo "Destination: $MODEL_DIR/$MODEL_NAME"
echo ""

# Check if model already exists
if [ -f "$MODEL_DIR/$MODEL_NAME" ]; then
    echo "⚠️  Model file already exists!"
    read -p "Overwrite? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Skipping download."
        exit 0
    fi
    rm "$MODEL_DIR/$MODEL_NAME"
fi

# Create directory if needed
mkdir -p "$MODEL_DIR"

# Check for download tools
if command -v curl &> /dev/null; then
    DOWNLOAD_CMD="curl"
    DOWNLOAD_ARGS="-L --progress-bar -o"
elif command -v wget &> /dev/null; then
    DOWNLOAD_CMD="wget"
    DOWNLOAD_ARGS="-O"
else
    echo "❌ Error: Need curl or wget to download"
    echo "   Please install one of these tools first"
    exit 1
fi

echo "📥 Starting download..."
echo "   This may take 5-10 minutes depending on your connection"
echo ""

cd "$MODEL_DIR"

if [ "$DOWNLOAD_CMD" = "curl" ]; then
    curl -L --progress-bar -o "$MODEL_NAME" "$MODEL_URL"
    DOWNLOAD_STATUS=$?
else
    wget --progress=bar "$MODEL_URL" -O "$MODEL_NAME"
    DOWNLOAD_STATUS=$?
fi

if [ $DOWNLOAD_STATUS -eq 0 ]; then
    echo ""
    echo "✅ Download complete!"
    echo ""
    echo "📊 File information:"
    ls -lh "$MODEL_NAME"
    echo ""
    echo "📝 Next steps:"
    echo "   1. Update src-tauri/tauri.conf.json:"
    echo "      Change \"resources\": [] to \"resources\": [\"models/**/*\"]"
    echo ""
    echo "   2. Model is ready for Phase 2 (LLM integration)"
    echo ""
else
    echo ""
    echo "❌ Download failed!"
    echo "   Please check your internet connection and try again"
    exit 1
fi
