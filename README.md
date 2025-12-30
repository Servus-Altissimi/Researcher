# Researcher, Scientific Scraper (with AI Validation)
This program, based on the given prompt, scrapes SearXNG instance for scientific papers, extracts DOIs and Abstracts from articles and evaluates its relevancy via Ai modules.

## Features
- An easy to use UI
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

If no options are given, it will start a browser UI on [port 6601](http://localhost:6601). Which you can open in your browser of choice.
<img width="928" height="886" alt="image" src="https://github.com/user-attachments/assets/e21158da-d9c2-43d1-af22-16d6504a1edd" />

# draft, too lazy to finish this rn
## The Windows shaped elephant in the room
Setting up SearXNG is notoriously difficult, especially on Windows since there's no official instructions tailored towards. Hence I recommend using a more UNIX-adjacent system, for example FreeBSD. If you really need to use Windows, here are the instructions to set this program up:

1. [Download and install Docker](https://www.docker.com/), if you have a Windows package-manager installed, use that
2. Open up a Powershell as Admin and run 

