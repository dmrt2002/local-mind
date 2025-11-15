# LLM Model Setup Guide

## When Will You Need This?

- **Phase 1 (MVP)**: ❌ **NOT NEEDED** - Only keyword and semantic search are required
- **Phase 2 (AI Q&A & Categorization)**: ✅ **REQUIRED** - LLM inference for answering questions and intelligent categorization

You can complete Phase 1 testing without this model. The model is needed for:
- AI Q&A feature (Ask AI)
- Intelligent snippet categorization
- Smart suggestions

---

## Quick Setup Steps

### Step 1: Download Qwen2.5-1.5B Model

**Option A: Direct Download (Recommended)**

The model is available on Hugging Face:

```bash
# Navigate to models directory
cd src-tauri/models/llm

# Create directory if it doesn't exist
mkdir -p src-tauri/models/llm

# Download using curl (macOS/Linux)
curl -L -o qwen2.5-1.5b-instruct-q3_k_m.gguf \
  "https://huggingface.co/bartowski/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/Qwen2.5-1.5B-Instruct-Q3_K_M.gguf"
```

**Option B: Using Hugging Face CLI** (if you have it installed)

```bash
# Install huggingface-cli if needed
pip install huggingface_hub

# Download the model
huggingface-cli download bartowski/Qwen2.5-1.5B-Instruct-GGUF \
  Qwen2.5-1.5B-Instruct-Q3_K_M.gguf \
  --local-dir src-tauri/models/llm \
  --local-dir-use-symlinks False
```

**Option C: Manual Download**

1. Visit: https://huggingface.co/bartowski/Qwen2.5-1.5B-Instruct-GGUF/tree/main
2. Find `Qwen2.5-1.5B-Instruct-Q3_K_M.gguf` (about 940MB)
3. Click "Download" button
4. Save to: `src-tauri/models/llm/qwen2.5-1.5b-instruct-q3_k_m.gguf`

---

### Step 2: Verify Download

```bash
# Check file exists and size
ls -lh src-tauri/models/llm/qwen2.5-1.5b-instruct-q3_k_m.gguf

# Expected output:
# -rw-r--r--  1 user  staff   940M  ...  qwen2.5-1.5b-instruct-q3_k_m.gguf
```

**Expected file size**: ~940MB (approximately 985,000,000 bytes)

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

**Note**: The model file is ~940MB, so committing it to git is not recommended unless using Git LFS.

---

## Alternative: Different Quantization Levels

If you want a different size/quality tradeoff, you could use:

### Qwen2.5-1.5B Q2_K (Smaller, lower quality)
- **Size**: ~600MB
- **URL**: https://huggingface.co/bartowski/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/Qwen2.5-1.5B-Instruct-Q2_K.gguf
- **Quality**: Lower than Q3_K_M, but faster

### Qwen2.5-1.5B Q4_K_M (Higher quality, larger)
- **Size**: ~1.0GB
- **URL**: https://huggingface.co/bartowski/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf
- **Quality**: Better than Q3_K_M, but larger

**Recommendation**: Start with **Q3_K_M** (balanced quality/size)

---

## Download Script

Save this as `download_model.sh` in the project root:

```bash
#!/bin/bash

# Download Qwen2.5-1.5B Model
MODEL_DIR="src-tauri/models/llm"
MODEL_NAME="qwen2.5-1.5b-instruct-q3_k_m.gguf"
MODEL_URL="https://huggingface.co/bartowski/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/Qwen2.5-1.5B-Instruct-Q3_K_M.gguf"

echo "Downloading Qwen2.5-1.5B model..."
echo "This may take a while (940MB download)..."

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

### Qwen2.5-1.5B-Instruct-Q3_K_M

- **Model Size**: 1.5B parameters
- **File Size**: ~940MB (quantized)
- **Quantization**: Q3_K_M (3-bit, medium quality)
- **Context Window**: 2048 tokens
- **Format**: GGUF (compatible with llama.cpp)
- **License**: Apache 2.0
- **Specialization**: Instruction following, categorization tasks

### Performance Expectations

On a typical CPU (4-core):
- **First token**: 2-5 seconds
- **Generation speed**: 1-2 tokens/second
- **Memory usage**: ~1.5-2GB RAM when loaded
- **Categorization**: 2-5 seconds per snippet

---

## When to Download

### Timeline Recommendation

1. **Now (Phase 1)**: ❌ Skip - Not needed yet
2. **After Phase 1 testing**: ✅ Download and test Phase 2 integration
3. **Before Phase 2 development**: ✅ Have model ready

### Current Status

- ✅ Phase 1 MVP can run without model (keyword + semantic search work)
- ⏳ Phase 2+ requires model (AI Q&A feature, intelligent categorization)
- 📦 Model download takes 5-10 minutes depending on internet speed
- 🤖 Model is used for: snippet categorization, AI Q&A, smart suggestions

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
shasum -a 256 src-tauri/models/llm/qwen2.5-1.5b-instruct-q3_k_m.gguf
```

### Insufficient Space

- Ensure at least 1GB free space (for download + extraction if needed)
- Check: `df -h .` (shows available space)

---

## Next Steps After Download

Once the model is downloaded:

1. ✅ Place it in `src-tauri/models/llm/`
2. ✅ Update `tauri.conf.json` to include it in resources (if not already configured)
3. ✅ Test model loading
4. ✅ Verify inference works
5. ✅ Test categorization and RAG pipeline

---

## Integration Code Reference

The model is loaded in:
- `src-tauri/src/inference/llm_manager.rs` - Model loading and caching
- `src-tauri/src/inference/llama.rs` - Model inference
- `src-tauri/src/inference/categorization.rs` - Categorization prompts
- `src-tauri/src/commands.rs` - `ask_ai` command handler

---

## Summary

**For Phase 1**: No model needed ✅  
**For Phase 2+**: Download model when ready ⏳  
**Download command**: See Step 1 above  
**File location**: `src-tauri/models/llm/qwen2.5-1.5b-instruct-q3_k_m.gguf`

Happy coding! 🚀
