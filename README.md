# Researcher, Scientific Scraper (with AI Validation)
This program, based on the give prompt, scrapes SearXNG instance for scientific papers, extracts DOIs and Abstracts from articles and evaluates its relevancy via Ai modules.

## Features
- Searches SearXNG (local or remote instance) for research papers on a chosen topic
- Extracts and validates DOIs
- Uses Ollama (local LLM) to check if results are relevant
- Saves validated results to a text file
- Avoids duplicate finds

## Requirements
- Rust & Cargo
- Ollama installed **and running**
- SearXNG instance (local or remote)

## Compile
```
cargo build --release
```

## Main Options

| Option | Description | Default |
|--------|--------------|----------|
| `--subject` | Search topic | `"machine learning"` |
| `--instance` | SearXNG instance URL | `https://searxng.site/` |
| `--max-results` | Maximum number of results | `50` |
| `--output` | Output text file | `results.txt` |
| `--model` | Ollama model name | `llama3.2:latest` |
| `--no-ai` | Disable AI validation | `false` |
| `--time-range` | Time filter (`day`, `week`, `month`, `year`) | `""` |
| `--category` | SearXNG category | `science` |
| `--engines` | Comma-separated engines list | `arxiv,pubmed,google scholar+` |
| `--min-score` | Minimum AI relevance score | `0.6` |
| `--verbose` | Print extra debug info and AI reasoning | `false` |
