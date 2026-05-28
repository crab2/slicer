**Language / 语言:** [English](README.en.md) | [中文](README.md)

# SLICER

SLICER is a local-first desktop tool for document page slicing and retrieval. It converts PDF, PPT, PPTX, DOC, and DOCX files into page-level images, uses a multimodal model to generate structured page JSON metadata, and builds a local BM25 index for traceable search results.

The project is built with Tauri, React, TypeScript, and Rust. Windows is the first-priority platform. Documents, page images, the database, JSONL metadata, and indexes are stored by default in the local workspace selected by the user. SLICER does not perform cloud sync by default.

## Use Cases

- Local knowledge base organization: convert courseware, reports, proposals, policies, papers, and other materials into page-level knowledge assets.
- Document page retrieval: search page titles, summaries, visible text, topics, keywords, and source filenames.
- Multimodal page understanding: analyze page images into structured JSON through a user-configured vision or multimodal model.
- Enterprise document archiving: organize training materials, sales decks, proposal documents, and workflow documentation.
- Local automation integration: query search results, page records, document records, or trigger index rebuilds through the localhost HTTP API.

## Features

- Select a local workspace and initialize its directory structure automatically.
- Import PDF, DOC, DOCX, PPT, and PPTX files.
- Render PDF files into per-page PNG images.
- Convert Office documents to PDF through local LibreOffice headless mode, then render them into PNG images.
- Name page images by content hash to reduce duplicate image conflicts.
- Store document, page, job, analysis, index, and settings state in SQLite.
- Export page-level metadata to `metadata/pages.jsonl`.
- Configure model provider, Base URL, custom endpoint, model name, and API key.
- Analyze page images and generate `page_analysis_v1` JSON.
- Support single-page analysis, batch analysis of new pages, document reanalysis, and failed-page retry.
- Build and rebuild a local BM25 index.
- Search pages in the desktop GUI, with image preview and page JSON inspection.
- Optionally enable a localhost HTTP API.

## Tech Stack

- Frontend: React 19, TypeScript, Vite
- Desktop shell: Tauri 2
- Backend: Rust
- Database: SQLite
- Search: Tantivy BM25
- HTTP API: Axum
- PDF rendering: Pdfium
- Office conversion: LibreOffice headless
- Secret storage: system credential storage; API keys are not written to ordinary config files

## Prerequisites

### Required

1. Node.js: `20.19+` or `22.12+` is recommended.
2. Rust stable and Cargo.
3. Tauri 2 system dependencies.
4. On Windows, Microsoft WebView2 Runtime and C++ Build Tools are recommended.

### Optional

1. LibreOffice: required for importing DOC, DOCX, PPT, and PPTX files. It is not required if you only import PDF files.
2. Multimodal model API: required for page analysis. SLICER supports SiliconFlow, MiMo, OpenAI, and Anthropic.

## Install From Source

Run this command in the project root:

```bash
npm install
```

If Rust dependencies have not been downloaded yet, Cargo will fetch them automatically during the first Tauri run or build.

## Development

Start the desktop app in development mode:

```bash
npm run tauri dev
```

Start only the frontend Vite dev server:

```bash
npm run dev
```

In most cases, use `npm run tauri dev`, because it starts both the frontend and the Tauri desktop window.

## Build

Build the frontend:

```bash
npm run build
```

Package the desktop app:

```bash
npm run tauri build
```

Tauri writes build artifacts under the corresponding directory in `src-tauri/target/`.

## Basic Workflow

### 1. Select a Workspace

On first launch, select a local directory as the workspace from the Workbench or Settings page. SLICER creates the required runtime files in that directory:

```text
workspace/
  originals/
  pages/
  analysis/
  metadata/
    pages.jsonl
  indexes/
    bm25/
  jobs/
  logs/
  tmp/
  app.db
```

Notes:

- `originals/` stores copies of imported source documents.
- `pages/` stores rendered per-page PNG images.
- `metadata/pages.jsonl` stores page-level JSONL exports.
- `indexes/bm25/` stores the local search index.
- `app.db` is the local SQLite ledger.
- `logs/` stores diagnostic logs.

### 2. Import Documents

Open the Workbench, click "Select Files", and choose one or more documents:

- Supported extensions: `.pdf`, `.doc`, `.docx`, `.ppt`, `.pptx`
- PDF files are rendered directly into page images.
- Office documents are converted to PDF through LibreOffice, then rendered into page images.

If an Office document is imported before LibreOffice is configured, the task fails with a recoverable error. Configure the path, then import again or retry.

### 3. Configure LibreOffice

Open Settings and enter a LibreOffice installation directory or the `soffice` executable path.

Common Windows path:

```text
C:/Program Files/LibreOffice/program
```

You can also enter:

```text
C:/Program Files/LibreOffice/program/soffice.exe
```

### 4. Configure a Model

Open Settings and fill in the model configuration:

- Provider
- Base URL
- Custom Endpoint
- Model Name
- API Key

The API key is saved through system credential storage and is not written to ordinary config files. Before cloud model analysis is enabled, the app shows a privacy notice explaining that page images will be sent to the configured model service.

Model analysis requires a supported remote vision provider and privacy notice confirmation.

### 5. Analyze Pages

After import completes, click "Analyze New Pages" in the Workbench model analysis section.

After analysis, each page receives JSON that conforms to `page_analysis_v1`. The analysis result is written to SQLite and exported to:

```text
metadata/pages.jsonl
```

### 6. Build or Rebuild the Index

Open the Search page or the index status section in the Workbench, then click "Build Index" or "Rebuild Index".

The index is built from analyzed pages. Search text includes:

- Page title
- Summary
- Visible text
- Topics
- Keywords
- Source filename

Index rebuilds do not delete original images or page JSON. If a rebuild fails, the previous usable index is kept whenever possible.

### 7. Search Pages

Open the Search page, enter a keyword, and run the search. Results include:

- Page title or page number
- Summary
- Source document
- Page number
- Relevance score
- Page image preview
- Page JSON

## Localhost HTTP API

SLICER can enable a local HTTP API from Settings. The default listener is:

```text
127.0.0.1:17321
```

Available endpoints:

```text
GET  /health
GET  /search?q={query}&limit={n}
GET  /pages/{page_id}
GET  /documents/{document_id}
POST /indexes/rebuild
```

Examples:

```bash
curl "http://127.0.0.1:17321/health"
```

```bash
curl "http://127.0.0.1:17321/search?q=multimodal%20retrieval&limit=10"
```

`POST /indexes/rebuild` is a write/heavy endpoint and requires a local token. You can reset the token in the Localhost API section of Settings.

```bash
curl -X POST "http://127.0.0.1:17321/indexes/rebuild" \
  -H "Authorization: Bearer <your-local-token>"
```

Successful responses use:

```json
{
  "data": {}
}
```

Error responses use:

```json
{
  "error": {
    "code": "example_error",
    "message": "Error description",
    "stage": "api",
    "retryable": true,
    "details": null,
    "correlation_id": "..."
  }
}
```

## Page JSON Example

Page analysis results use the `page_analysis_v1` schema. Example:

```json
{
  "page_id": "page_123",
  "image_hash": "7f9a2c91b44d18e2...",
  "image_path": "pages/doc_123/7f9a2c91b44d18e2.png",
  "source": {
    "document_id": "doc_123",
    "original_filename": "AI Product Proposal.pptx",
    "page_number": 12,
    "total_pages": 30,
    "document_type": "pptx"
  },
  "analysis": {
    "title": "Multimodal Retrieval System Architecture",
    "summary": "This page shows the overall flow of document-to-image conversion, visual understanding, index building, and query result return.",
    "topics": ["multimodal", "document parsing", "retrieval"],
    "visible_text": ["Input Document", "Image Generation", "Multimodal Analysis", "BM25"],
    "keywords": ["PPT to image", "page-level index", "visual analysis"],
    "content_type": "architecture_diagram"
  },
  "retrieval": {
    "bm25_text": "Multimodal Retrieval System Architecture Input Document Image Generation Multimodal Analysis BM25 Page-level Index"
  },
  "model": {
    "provider": "custom_http",
    "model_name": "configured-by-user"
  },
  "schema_version": "page_analysis_v1"
}
```

## Common Commands

```bash
npm install
npm run tauri dev
npm run build
npm run tauri build
```

Rust backend tests:

```bash
cd src-tauri
cargo test
```

Rust backend compile check:

```bash
cd src-tauri
cargo check
```

## Privacy and Security

- SLICER is local-first by default. Documents, images, the database, and indexes are stored in the workspace selected by the user.
- The app does not perform cloud sync by default.
- API keys are stored through system credential storage and should not appear in logs, exported JSON, error messages, or search results.
- Page images are sent to a model service only when cloud or custom model analysis is enabled.
- The Localhost API binds to `127.0.0.1` by default and should not listen on a public network address by default.
- Write endpoints such as index rebuild require a local token.

## Project Structure

```text
.
  src/                  React + TypeScript frontend
  src-tauri/            Rust/Tauri backend
  src-tauri/src/api/    localhost HTTP API
  src-tauri/src/commands/
                        Tauri commands
  src-tauri/src/services/
                        application service layer
  src-tauri/src/repositories/
                        SQLite access layer
  src-tauri/src/providers/
                        PDF, LibreOffice, model, and search providers
  src-tauri/src/artifacts/
                        workspace files, JSONL, and index directory management
  src-tauri/migrations/ SQLite migrations
  public/               static assets
  docs/                 project documentation
```

## Troubleshooting

### Startup or Build Fails

Make sure Node.js, Rust, Cargo, and Tauri system dependencies are installed. On Windows, WebView2 Runtime and C++ Build Tools are also required.

### Office Document Import Fails

Make sure LibreOffice is installed and the path in Settings is correct. You can enter the LibreOffice `program` directory or the direct `soffice.exe` path.

### PDF Rendering Fails

The file may be corrupted, encrypted, or Pdfium may be unavailable. First verify that the PDF can be opened in a normal PDF reader.

### Model Analysis Is Unavailable

Check Provider, Base URL, Custom Endpoint, Model Name, and API Key in Settings. Cloud model use also requires accepting the privacy notice.

### Search Is Unavailable

Search depends on analyzed pages and the BM25 index. Analyze pages first, then build or rebuild the index from the Search page.

### Localhost API Is Unavailable

Make sure the API is enabled in Settings, the port is not occupied, and the bind address is `127.0.0.1`. The default port is `17321`.

## Current Status

This project is still in MVP development. This README reflects the capabilities currently present in the repository and the planned MVP workflow. If the UI, API, or workspace structure changes, update this document accordingly.
