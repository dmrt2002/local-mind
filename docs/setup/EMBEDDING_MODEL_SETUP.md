# Embedding Model Setup - Bundle with App

## Problem

The embedding model is trying to download at runtime and getting **403 Forbidden** errors from Google Cloud Storage. This is unreliable and slow.

## Solution: Bundle Model with App

Download the model **once during development**, bundle it with the app, and copy it to fastembed's cache on first run.

---

## Quick Setup

### Step 1: Download the Model

Run the download script:

```bash
./download_embedding_model.sh
```

Or manually:

```bash
# Create directory
mkdir -p src-tauri/models/embeddings
cd src-tauri/models/embeddings

# Download model files from HuggingFace
curl -L -o config.json "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/config.json"
curl -L -o pytorch_model.bin "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/pytorch_model.bin"
curl -L -o tokenizer_config.json "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer_config.json"
curl -L -o vocab.txt "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/vocab.txt"
```

**Total size**: ~90MB

### Step 2: Verify Files

```bash
ls -lh src-tauri/models/embeddings/
```

Should show:
- `config.json` (~2KB)
- `pytorch_model.bin` (~90MB) ⭐ **Main model file**
- `tokenizer_config.json` (~1KB)
- `vocab.txt` (~230KB)

### Step 3: Bundle with App

The model is automatically bundled via `tauri.conf.json`:

```json
"resources": ["models/**/*"]
```

### Step 4: How It Works

1. **During Development**: Model is copied from `src-tauri/models/embeddings/` to `~/.cache/fastembed/` on first use
2. **In Built App**: Model is copied from bundled resources to `~/.cache/fastembed/` on first use
3. **Subsequent Runs**: fastembed finds it in cache (no download needed!)

---

## Model Information

- **Model**: `sentence-transformers/all-MiniLM-L6-v2`
- **Size**: ~90MB
- **Dimensions**: 384
- **License**: Apache 2.0 (can be bundled)
- **Source**: HuggingFace

---

## Troubleshooting

### 403 Forbidden Error

If you still see 403 errors:
1. **Check if model is downloaded**: `ls -lh src-tauri/models/embeddings/`
2. **Check if cache exists**: `ls -la ~/.cache/fastembed/`
3. **Check logs**: Look for "Found bundled model" or "Found model in dev directory"

### Model Not Found

If bundled model isn't found:
- In dev: Check `src-tauri/models/embeddings/` exists
- In production: Check `tauri.conf.json` has `"resources": ["models/**/*"]`

### Cache Permission Issues

If cache directory can't be created:
```bash
mkdir -p ~/.cache/fastembed
chmod 755 ~/.cache/fastembed
```

---

## File Structure

```
src-tauri/
├── models/
│   └── embeddings/          # Model files (downloaded once)
│       ├── config.json
│       ├── pytorch_model.bin  (90MB)
│       ├── tokenizer_config.json
│       └── vocab.txt
└── tauri.conf.json          # Bundles models/**/*
```

After bundling, the model is included in:
- **macOS**: `.app/Contents/Resources/models/embeddings/`
- **Windows**: `resources/models/embeddings/`
- **Linux**: `resources/models/embeddings/`

---

## Benefits

✅ **No runtime downloads** - Model bundled with app  
✅ **No 403 errors** - No dependency on external storage  
✅ **Faster startup** - Model ready immediately  
✅ **Offline capable** - Works without internet  
✅ **Consistent** - Same model version for all users  

---

## Summary

**Before**: Model downloads at runtime → 403 errors → fails  
**After**: Model bundled with app → copied to cache → works instantly!

**Next Step**: Run `./download_embedding_model.sh` and rebuild the app! 🚀

