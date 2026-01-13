<div align="center">

  # Researcher

  **Easy 2 Use Scientific Research Scraper (with AI Validation)**

  [![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
  [![Rust](https://img.shields.io/badge/rust-1.89+-orange.svg)](https://www.rust-lang.org/)
</div>

# Researcher, Scientific Scraper (with AI Validation)
This program, based on the given prompt, scrapes SearXNG instance for scientific papers, extracts DOI's, links and abstracts from scientific articles and evaluates its relevancy via AI modules.

## Features
- Clean Web UI
- Searches SearXNG (local or remote instance) for research papers on a chosen topic
- Extracts and validates DOIs
- Uses Ollama to check if results are relevant
- Saves validated results to a text file
- Avoids duplicate finds

## Requirements
- Rust & Cargo
- Ollama instance (local or remote)
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
| `--category` | SearXNG category | `science` |
| `--engines` | Comma-separated engines list | `arxiv,pubmed,google scholar+` |
| `--min-score` | Minimum AI relevance score | `0.6` |
| `--verbose` | Print extra debug info and AI reasoning | `false` |

If no options are given, it will start a browser UI on [port 6601](http://localhost:6601). Which you can open in your browser of choice.
<img width="928" height="886" alt="image" src="https://github.com/user-attachments/assets/e21158da-d9c2-43d1-af22-16d6504a1edd" />

## The Windows shaped elephant in the room
I wouldn't bother trying to run this program natively in Windows. It's better to just use the Linux subsystem to run this. I won't package this program in a Docker, consult this [article](https://gist.github.com/jerrywaller/9927c7af2599553fd7b48af185a89dba).

