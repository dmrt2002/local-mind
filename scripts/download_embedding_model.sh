#!/bin/bash

# Download Embedding Model for LocalMind
# This script downloads the all-MiniLM-L6-v2 embedding model (~90MB)
# The model will be bundled with the app so it doesn't need to download at runtime

MODEL_DIR="src-tauri/models/embeddings"
REPO_URL="https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main"

echo "🚀 LocalMind Embedding Model Download Script"
echo "=============================================="
echo ""
echo "Model: all-MiniLM-L6-v2 (sentence-transformers)"
echo "Size: ~90MB total"
echo "Destination: $MODEL_DIR/"
echo ""

# Create directory
mkdir -p "$MODEL_DIR"
cd "$MODEL_DIR" || exit 1

# Check if already downloaded
if [ -f "pytorch_model.bin" ]; then
    echo "⚠️  Model already exists at $MODEL_DIR/pytorch_model.bin"
    read -p "Overwrite? (y/N): " -n 1 -r
    echo
    if [[ ! $REPO =~ ^[Yy]$ ]]; then
        echo "Skipping download."
        exit 0
    fi
fi

echo "📥 Downloading model files from HuggingFace..."
echo "   This may take 5-10 minutes depending on your connection..."
echo ""

# Required files for the model
FILES=(
    "config.json"
    "pytorch_model.bin"
    "tokenizer_config.json"
    "vocab.txt"
)

DOWNLOAD_CMD=""
if command -v curl &> /dev/null; then
    DOWNLOAD_CMD="curl"
elif command -v wget &> /dev/null; then
    DOWNLOAD_CMD="wget"
else
    echo "❌ Error: Need curl or wget to download"
    exit 1
fi

for file in "${FILES[@]}"; do
    echo "   Downloading $file..."
    
    if [ "$DOWNLOAD_CMD" = "curl" ]; then
        curl -L --progress-bar -f -o "$file" "${REPO_URL}/${file}"
    else
        wget --progress=bar "${REPO_URL}/${file}" -O "$file"
    fi
    
    if [ $? -eq 0 ]; then
        FILE_SIZE=$(ls -lh "$file" | awk '{print $5}')
        echo "   ✅ $file downloaded ($FILE_SIZE)"
    else
        echo "   ❌ Failed to download $file"
        echo "   Retrying..."
        sleep 2
        # Retry once
        if [ "$DOWNLOAD_CMD" = "curl" ]; then
            curl -L --progress-bar -f -o "$file" "${REPO_URL}/${file}"
        else
            wget "${REPO_URL}/${file}" -O "$file"
        fi
        
        if [ $? -ne 0 ]; then
            echo "   ❌ Failed after retry. Check your internet connection."
            exit 1
        fi
    fi
done

echo ""
echo "✅ Model download complete!"
echo ""
echo "📊 Files downloaded:"
ls -lh
echo ""
echo "📝 Next steps:"
echo "   1. Model is in: $MODEL_DIR/"
echo "   2. It will be bundled with the app (via tauri.conf.json)"
echo "   3. On first run, it will be copied to ~/.cache/fastembed/"
echo "   4. No runtime downloads needed!"
echo ""
echo "✨ You can now build the app and the model will be included!"
echo ""

