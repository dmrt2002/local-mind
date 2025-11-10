# LocalMind Scripts

This folder contains utility scripts for setting up and testing LocalMind.

## Available Scripts

### Setup Scripts

#### `download_model.sh`
Downloads the TinyLlama model for local AI Q&A (Phase 2 feature).

```bash
./scripts/download_model.sh
```

#### `download_embedding_model.sh`
Downloads the embedding model for semantic search.

```bash
./scripts/download_embedding_model.sh
```

#### `download_embedding_model_onnx.sh`
Downloads the ONNX version of the embedding model (optimized).

```bash
./scripts/download_embedding_model_onnx.sh
```

### Testing Scripts

#### `test_search.sh`
Tests the search functionality with various queries.

```bash
./scripts/test_search.sh
```

#### `quick-test.sh`
Quick smoke test for the application.

```bash
./scripts/quick-test.sh
```

## Usage

All scripts should be run from the project root directory:

```bash
cd /path/to/local-mind
./scripts/script-name.sh
```

## Script Permissions

If you encounter permission errors, make scripts executable:

```bash
chmod +x scripts/*.sh
```
