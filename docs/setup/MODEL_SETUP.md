# TinyLlama Model Setup Guide

## When Will You Need This?

- **Phase 1 (MVP)**: ❌ **NOT NEEDED** - Only keyword and semantic search are required
- **Phase 2 (AI Q&A)**: ✅ **REQUIRED** - LLM inference for answering questions

You can complete Phase 1 testing without this model. The model is only needed when implementing the "Ask AI" feature.

---

## Quick Setup Steps

### Step 1: Download TinyLlama Model

**Option A: Direct Download (Recommended)**

The model is available on Hugging Face:

```bash
# Navigate to models directory
cd src-tauri/models

# Download using curl (macOS/Linux)
curl -L -o TinyLlama-1.1B-Chat-v1.0.Q4_K_M.gguf \
  "https://huggingface.co/TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF/resolve/main/TinyLlama-1.1B-Chat-v1.0.Q4_K_M.gguf"
```

**Option B: Using Hugging Face CLI** (if you have it installed)

```bash
# Install huggingface-cli if needed
pip install huggingface_hub

# Download the model
huggingface-cli download TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF \
  TinyLlama-1.1B-Chat-v1.0.Q4_K_M.gguf \
  --local-dir src-tauri/models \
  --local-dir-use-symlinks False
```

**Option C: Manual Download**

1. Visit: https://huggingface.co/TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF/tree/main
2. Find `TinyLlama-1.1B-Chat-v1.0.Q4_K_M.gguf` (about 670MB)
3. Click "Download" button
4. Save to: `src-tauri/models/TinyLlama-1.1B-Chat-v1.0.Q4_K_M.gguf`

---

### Step 2: Verify Download

```bash
# Check file exists and size
ls -lh src-tauri/models/TinyLlama-1.1B-Chat-v1.0.Q4_K_M.gguf

# Expected output:
# -rw-r--r--  1 user  staff   670M  ...  TinyLlama-1.1B-Chat-v1.0.Q4_K_M.gguf
```

**Expected file size**: ~670MB (approximately 700,000,000 bytes)

---

### Step 3: Update Configuration (After Download)

Once the model file is in place, update `src-tauri/tauri.conf.json`:

```json
"resources": ["models/**/*"]
```

This tells Tauri to bundle the model with your application installer.

---

### Step 4: Verify in .gitignore

Make sure `models/*.gguf` is in `.gitignore` (unless you want to commit it):

```gitignore
# Models (large files)
models/*.gguf
models/*.ggml
```

**Note**: The model file is ~670MB, so committing it to git is not recommended unless using Git LFS.

---

## Alternative: Smaller Model for Testing

If you want a smaller model for initial testing, you could use:

### TinyLlama Q2_K (Smaller, lower quality)
- **Size**: ~420MB
- **URL**: https://huggingface.co/TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF/resolve/main/TinyLlama-1.1B-Chat-v1.0.Q2_K.gguf
- **Quality**: Lower than Q4_K_M, but faster

### TinyLlama Q8_0 (Higher quality, larger)
- **Size**: ~1.2GB
- **URL**: https://huggingface.co/TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF/resolve/main/TinyLlama-1.1B-Chat-v1.0.Q8_0.gguf
- **Quality**: Better than Q4_K_M, but larger

**Recommendation**: Start with **Q4_K_M** (balanced quality/size)

---

## Download Script

Save this as `download_model.sh` in the project root:

```bash
#!/bin/bash

# Download TinyLlama Model
MODEL_DIR="src-tauri/models"
MODEL_NAME="TinyLlama-1.1B-Chat-v1.0.Q4_K_M.gguf"
MODEL_URL="https://huggingface.co/TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF/resolve/main/${MODEL_NAME}"

echo "Downloading TinyLlama model..."
echo "This may take a while (670MB download)..."

mkdir -p "$MODEL_DIR"

cd "$MODEL_DIR"

if command -v curl &> /dev/null; then
    curl -L -o "$MODEL_NAME" "$MODEL_URL"
elif command -v wget &> /dev/null; then
    wget -O "$MODEL_NAME" "$MODEL_URL"
else
    echo "Error: Need curl or wget to download"
    exit 1
fi

echo "Download complete!"
echo "File location: $MODEL_DIR/$MODEL_NAME"
ls -lh "$MODEL_NAME"
```

Make it executable:
```bash
chmod +x download_model.sh
./download_model.sh
```

---

## Model Information

### TinyLlama-1.1B-Chat-v1.0.Q4_K_M

- **Model Size**: 1.1B parameters
- **File Size**: ~670MB (quantized)
- **Quantization**: Q4_K_M (4-bit, medium quality)
- **Context Window**: 2048 tokens
- **Format**: GGUF (compatible with llama.cpp)
- **License**: Apache 2.0

### Performance Expectations

On a typical CPU (4-core):
- **First token**: 2-5 seconds
- **Generation speed**: 1-2 tokens/second
- **Memory usage**: ~1.5-2GB RAM when loaded

---

## When to Download

### Timeline Recommendation

1. **Now (Phase 1)**: ❌ Skip - Not needed yet
2. **After Phase 1 testing**: ✅ Download and test Phase 2 integration
3. **Before Phase 2 development**: ✅ Have model ready

### Current Status

- ✅ Phase 1 MVP can run without model (keyword + semantic search work)
- ⏳ Phase 2 requires model (AI Q&A feature)
- 📦 Model download takes 5-10 minutes depending on internet speed

---

## Troubleshooting

### Download Fails / Slow

- Try using a download manager
- Use Hugging Face CLI with resume capability
- Download during off-peak hours

### File Verification

Check the file hash (if available):
```bash
# Compare with Hugging Face page checksum
shasum -a 256 src-tauri/models/TinyLlama-1.1B-Chat-v1.0.Q4_K_M.gguf
```

### Insufficient Space

- Ensure at least 1GB free space (for download + extraction if needed)
- Check: `df -h .` (shows available space)

---

## Next Steps After Download

Once the model is downloaded:

1. ✅ Place it in `src-tauri/models/`
2. ✅ Update `tauri.conf.json` to include it in resources
3. ✅ Test model loading (Phase 2 implementation)
4. ✅ Verify inference works
5. ✅ Test RAG pipeline

---

## Integration Code Reference

When implementing Phase 2, the model will be loaded in:
- `src-tauri/src/inference/llama.rs` - Model loading and inference
- `src-tauri/src/inference/loader.rs` - Progressive loading with progress
- `src-tauri/src/commands.rs` - `ask_ai` command handler

The placeholder code is already in place, waiting for the actual model file!

---

## Summary

**For Phase 1**: No model needed ✅  
**For Phase 2**: Download model when ready ⏳  
**Download command**: See Step 1 above  
**File location**: `src-tauri/models/TinyLlama-1.1B-Chat-v1.0.Q4_K_M.gguf`

Happy coding! 🚀
