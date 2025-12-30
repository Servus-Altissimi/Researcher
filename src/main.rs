// ┬─┐┌─┐┌─┐┌─┐┌─┐┬─┐┌─┐┬ ┬┌─┐┬─┐
// ├┬┘├┤ └─┐├┤ ├─┤├┬┘│  ├─┤├┤ ├┬┘
// ┴└─└─┘└─┘└─┘┴ ┴┴└─└─┘┴ ┴└─┘┴└─

// Requires Ollama & SearXNG
// validates relevance with AI, and saves results to a text file.

// Copyright 2025 Servus Altissimi (Pseudonym)

// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
// The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

mod web;

use anyhow::{Result, anyhow};
use clap::Parser;
use ollama_rs::Ollama;
use ollama_rs::generation::completion::request::GenerationRequest;
use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

// CL arguments for config
#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "SearXNG Scientific DOI Scraper with AI Validation", long_about = None)]
pub struct Args {
    #[arg(short, long, default_value = "machine learning")]
    pub subject: String,

    #[arg(short, long, default_value = "https://searxng.site/")]
    pub instance: String,

    #[arg(short, long, default_value = "50")]
    pub max_results: usize,

    #[arg(short, long, default_value = "results.txt")]
    pub output: String,

    #[arg(long, default_value = "llama3.2:latest")]
    pub model: String,

    #[arg(long, default_value_t = false)]
    pub no_ai: bool,

    #[arg(short, long, default_value = "")]
    pub time_range: String,

    #[arg(short, long, default_value = "science")]
    pub category: String,

    #[arg(short, long, default_value = "arxiv,pubmed,google scholar,crossref,openairepublications,openairedatasets,semantic scholar")]
    pub engines: String,

    #[arg(long, default_value = "0.6")]
    pub min_score: f32,

    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,

    #[arg(long, default_value = "6601")]
    pub web_poort: u16,

    #[arg(long, default_value = "http://localhost:11434")]
    pub ollama_url: String,
}

#[derive(Debug, Deserialize)]
struct SearxngResponse {
    results: Vec<SearchResult>,
}

// Represents one search result from SearXNG
#[derive(Debug, Deserialize, Clone)]
struct SearchResult {
    title: String,
    url: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    #[allow(dead_code)]
    engine: String,
}

#[derive(Debug, Deserialize)]
struct CrossRefResponse {
    message: CrossRefMessage,
}

#[derive(Debug, Deserialize)]
struct CrossRefMessage {
    #[serde(rename = "DOI")]
    #[allow(dead_code)]
    doi: String,
    title: Vec<String>,
    #[serde(default)]
    #[serde(rename = "abstract")]
    abstract_text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DataCiteResponse {
    data: DataCiteData,
}

#[derive(Debug, Deserialize)]
struct DataCiteData {
    attributes: DataCiteAttributes,
}

#[derive(Debug, Deserialize)]
struct DataCiteAttributes {
    #[serde(default)]
    titles: Vec<DataCiteTitle>,
    #[serde(default)]
    descriptions: Vec<DataCiteDescription>,
}

#[derive(Debug, Deserialize)]
struct DataCiteTitle {
    title: String,
}

#[derive(Debug, Deserialize)]
struct DataCiteDescription {
    description: String,
}

#[derive(Debug)]
pub struct ScientificPaper {
    title: String,
    url: String,
    doi: Option<String>,
    abstract_text: String,
    relevance_score: f32,
}

pub struct DOIScraper {
    client: Client,
    ollama: Option<Ollama>,
    processed_dois: HashSet<String>,
    args: Args,
    doi_regex: Regex,
    use_ai: bool,
    logger: Option<Arc<Mutex<Vec<String>>>>,
}

impl DOIScraper {
    pub async fn new(args: Args) -> Result<Self> {
        Self::new_with_logger(args, None).await
    }

    fn safe_truncate(s: &str, max_len: usize) -> &str {
        if s.len() <= max_len {
            return s;
        }
        
        // Find the last valid char boundary at or before max_len
        let mut end = max_len;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }

    pub async fn new_with_logger(args: Args, logger: Option<Arc<Mutex<Vec<String>>>>) -> Result<Self> {
        let user_agents = [
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
            "Mozilla/5.0 (X11; Ubuntu; Linux x86_64) AppleWebKit/537.36",
            "Mozilla/5.0 (Linux; Android 14; Pixel 7) AppleWebKit/537.36",
            "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) AppleWebKit/537.36",
            "Mozilla/5.0 (Windows NT 6.1; Win64; x64) AppleWebKit/537.36",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 11_6) AppleWebKit/537.36",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
            "Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36",
            "Mozilla/5.0 (iPad; CPU OS 16_6 like Mac OS X) AppleWebKit/537.36",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 12_5_1) AppleWebKit/537.36",
            "Mozilla/5.0 (X11; Fedora; Linux x86_64) AppleWebKit/537.36",
            "Mozilla/5.0 (Linux; Android 12; OnePlus 9) AppleWebKit/537.36",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_14_6) AppleWebKit/537.36",
            "Mozilla/5.0 (Linux; Android 11; Nokia X20) AppleWebKit/537.36",
            "Mozilla/5.0 (Windows NT 6.3; Win64; x64) AppleWebKit/537.36",
            "Mozilla/5.0 (X11; CrOS x86_64 15604.45.0) AppleWebKit/537.36",
            "Mozilla/5.0 (Windows NT 10.0) AppleWebKit/537.36",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_13_6) AppleWebKit/537.36",
        ];

        let user_agent = user_agents[fastrand::usize(..user_agents.len())];

        let client = Client::builder()
            .user_agent(user_agent)
            .timeout(Duration::from_secs(30))
            .build()?;

        let (ollama, use_ai) = if args.no_ai {
            Self::log(&logger, &format!("{}", "=".repeat(64)));
            Self::log(&logger, "AI validation is disabled (--no-ai flag)");
            Self::log(&logger, &format!("{}\n", "=".repeat(64)));
            (None, false)
        } else {
            let url = args.ollama_url.trim_end_matches('/');
            let (host, port) = if let Some(idx) = url.rfind(':') {
                let port_str = &url[idx+1..];
                if let Ok(port) = port_str.parse::<u16>() {
                    (&url[..idx], port)
                } else {
                    (url, 11434)
                }
            } else {
                (url, 11434)
            };
            
            let ollama_client = Ollama::new(host, port);
            match ollama_client.list_local_models().await {
                Ok(_) => {
                    Self::log(&logger, &format!("{}", "=".repeat(64)));
                    Self::log(&logger, &format!("Ollama available at: {}:{}", host, port));
                    Self::log(&logger, &format!("Model: {}", args.model));
                    Self::log(&logger, &format!("{}\n", "=".repeat(64)));
                    (Some(ollama_client), true)
                }
                Err(_) => {
                    Self::log(&logger, &format!("{}", "=".repeat(64)));
                    Self::log(&logger, &format!("Ollama not available at: {}:{}", host, port));
                    Self::log(&logger, "AI validation disabled");
                    Self::log(&logger, &format!("{}\n", "=".repeat(64)));
                    (None, false)
                }
            }
        };

        let processed_dois = Self::load_processed_dois(&args.output)?;
        let doi_regex = Regex::new(r"10\.\d{4,9}/[-._;()/:A-Za-z0-9]+").unwrap();

        Self::log(&logger, &format!("{}", "=".repeat(64)));
        Self::log(&logger, "   SearXNG Scientific DOI Scraper with AI Validation");
        Self::log(&logger, &format!("{}", "=".repeat(64)));
        Self::log(&logger, &format!("\nSubject: {}", args.subject));
        Self::log(&logger, &format!("Instance: {}", args.instance));
        Self::log(&logger, &format!("Engines: {}", args.engines));
        
        if !args.time_range.is_empty() {
            Self::log(&logger, &format!("Time range: {}", args.time_range));
        } else {
            Self::log(&logger, "Time range: all time");
        }
        
        Self::log(&logger, &format!("Max results: {}", args.max_results));
        Self::log(&logger, &format!("Min score: {:.1}", args.min_score));
        Self::log(&logger, &format!("Output: {}", args.output));
        Self::log(&logger, &format!("Previously processed: {} DOIs\n", processed_dois.len()));

        Ok(Self {
            client,
            ollama,
            processed_dois,
            args,
            doi_regex,
            use_ai,
            logger,
        })
    }

    fn log(logger: &Option<Arc<Mutex<Vec<String>>>>, message: &str) {
        println!("{}", message);
        if let Some(log) = logger {
            if let Ok(mut logs) = log.lock() {
                let timestamp = chrono::Local::now().format("%H:%M:%S");
                let log_entry = format!("[{}] {}", timestamp, message);
                logs.push(log_entry);
                if logs.len() > 500 {
                    logs.remove(0);
                }
            }
        }
    }

    fn load_processed_dois(filepath: &str) -> Result<HashSet<String>> {
        let mut dois = HashSet::new();
        if let Ok(contents) = fs::read_to_string(filepath) {
            for line in contents.lines() {
                if let Some(doi) = line.split('|').next() {
                    dois.insert(doi.trim().to_string());
                }
            }
        }
        Ok(dois)
    }

    fn clean_doi(&self, doi: &str) -> String {
        let mut cleaned = doi.trim().to_string();
        
        if cleaned.starts_with("https://doi.org/") {
            cleaned = cleaned[16..].to_string();
        } else if cleaned.starts_with("http://doi.org/") {
            cleaned = cleaned[15..].to_string();
        }
        
        if cleaned.starts_with("doi:") {
            cleaned = cleaned[4..].to_string();
        }
        
        cleaned.trim().to_string()
    }

    fn extract_doi_from_text(&self, text: &str) -> Option<String> {
        if let Some(captures) = self.doi_regex.find(text) {
            return Some(self.clean_doi(captures.as_str()));
        }
        None
    }

    fn extract_doi_from_url(&self, url: &str) -> Option<String> {
        if url.contains("doi.org/") {
            if let Some(doi_part) = url.split("doi.org/").nth(1) {
                let cleaned = self.clean_doi(doi_part);
                if self.doi_regex.is_match(&cleaned) {
                    return Some(cleaned);
                }
            }
        }

        if url.contains("arxiv.org") {
            if let Some(arxiv_id) = url.split("/abs/").nth(1).or_else(|| url.split("/pdf/").nth(1)) {
                let id = arxiv_id.split('?').next()
                    .unwrap_or(arxiv_id)
                    .trim_end_matches(".pdf");
                return Some(format!("arXiv:{}", id));
            }
        }

        self.extract_doi_from_text(url)
    }

    async fn fetch_doi_metadata(&self, doi: &str) -> Result<(String, String)> {
        let clean_doi = self.clean_doi(doi);
        
        if self.args.verbose {
            Self::log(&self.logger, &format!("      [API] Trying doi.org for: {}", clean_doi));
        }
        
        if let Ok(response) = self.client
            .get(&format!("https://doi.org/{}", clean_doi))
            .header("Accept", "application/vnd.citationstyles.csl+json")
            .header("User-Agent", "DOI-APA-Generator/2.0")
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            if response.status().is_success() {
                if let Ok(text) = response.text().await {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                        if data.get("DOI").is_some() {
                            let title = data["title"].as_str()
                                .or_else(|| data["title"].as_array().and_then(|arr| arr[0].as_str()))
                                .unwrap_or("")
                                .to_string();
                            let abstract_text = data["abstract"].as_str().unwrap_or("").to_string();
                            
                            if !title.is_empty() {
                                if self.args.verbose {
                                    Self::log(&self.logger, "      [API] doi.org success");
                                }
                                return Ok((title, abstract_text));
                            }
                        }
                    }
                }
            }
        }

        if self.args.verbose {
            Self::log(&self.logger, "      [API] Attempting via CrossRef");
        }
        
        if let Ok(response) = self.client
            .get(&format!("https://api.crossref.org/works/{}", clean_doi))
            .header("Accept", "application/json")
            .header("User-Agent", "DOI-APA-Generator/2.0")
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            if response.status().is_success() {
                if let Ok(data) = response.json::<CrossRefResponse>().await {
                    let title = data.message.title.first()
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    let abstract_text = data.message.abstract_text.unwrap_or_default();
                    
                    if !title.is_empty() {
                        if self.args.verbose {
                            Self::log(&self.logger, "      [API] CrossRef success");
                        }
                        return Ok((title, abstract_text));
                    }
                }
            }
        }

        if self.args.verbose {
            Self::log(&self.logger, "      [API] Trying DataCite");
        }
        
        if let Ok(response) = self.client
            .get(&format!("https://api.datacite.org/dois/{}", clean_doi))
            .header("Accept", "application/json")
            .header("User-Agent", "DOI-APA-Generator/2.0")
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            if response.status().is_success() {
                if let Ok(data) = response.json::<DataCiteResponse>().await {
                    let title = data.data.attributes.titles.first()
                        .map(|t| t.title.clone())
                        .unwrap_or_default();
                    let abstract_text = data.data.attributes.descriptions.first()
                        .map(|d| d.description.clone())
                        .unwrap_or_default();
                    
                    if !title.is_empty() {
                        if self.args.verbose {
                            Self::log(&self.logger, "      [API] DataCite success");
                        }
                        return Ok((title, abstract_text));
                    }
                }
            }
        }

        Err(anyhow!("All DOI APIs failed"))
    }

    async fn fetch_page_content(&self, url: &str) -> Result<(String, Option<String>)> {
        let response = self.client
            .get(url)
            .timeout(Duration::from_secs(15))
            .send()
            .await?;

        if !response.status().is_success() {
            return Ok((String::new(), None));
        }

        let html = response.text().await?;
        let document = Html::parse_document(&html);

        let meta_selectors = vec![
            "meta[name='citation_doi']",
            "meta[name='DC.Identifier']",
            "meta[property='citation_doi']",
            "meta[name='DOI']",
        ];

        let mut doi = None;
        for selector_str in meta_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                for element in document.select(&selector) {
                    if let Some(content) = element.value().attr("content") {
                        if let Some(extracted) = self.extract_doi_from_text(content) {
                            doi = Some(extracted);
                            break;
                        }
                    }
                }
                if doi.is_some() {
                    break;
                }
            }
        }

        let abstract_meta_selectors = vec![
            "meta[name='citation_abstract']",
            "meta[name='description']",
            "meta[property='og:description']",
            "meta[name='DC.Description']",
        ];

        let mut abstract_text = String::new();
        for selector_str in abstract_meta_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    if let Some(content) = element.value().attr("content") {
                        if content.len() > 50 {
                            abstract_text = content.to_string();
                            break;
                        }
                    }
                }
            }
        }

        if abstract_text.is_empty() {
            let content_selectors = vec![
                "abstract", ".abstract", "#abstract", "div.abstract",
                "section.abstract", "div[class*='abstract']", "p[class*='abstract']",
            ];

            for selector_str in content_selectors {
                if let Ok(selector) = Selector::parse(selector_str) {
                    if let Some(element) = document.select(&selector).next() {
                        let text = element.text().collect::<Vec<_>>().join(" ");
                        if text.len() > 50 {
                            abstract_text = text.trim().to_string();
                            break;
                        }
                    }
                }
            }
        }

        Ok((abstract_text, doi))
    }

    async fn validate_with_ai(&self, title: &str, abstract_text: &str, subject: &str) -> Result<(bool, f32, String)> {
        let ollama = match &self.ollama {
            Some(o) => o,
            None => return Ok((true, 1.1, "AI disabled -_-".to_string())),
        };

        let abstract_preview = Self::safe_truncate(abstract_text, 400);

        let prompt = format!(
            "You are evaluating if a scientific paper is relevant to a research topic.\n\n\
            Research Topic: \"{}\"\n\n\
            Paper Title: \"{}\"\n\n\
            Abstract: \"{}\"\n\n\
            Rate the relevance from 0.0 to 1.0 and give a ONE to TWO sentence explanation.\n\n\
            Format your response EXACTLY like this:\n\
            SCORE: 0.85\n\
            REASON: This paper directly addresses machine learning algorithms for classification tasks.\n\n\
            Be very strict only give high scores (0.85+) if the paper is directly about the topic.",
            subject, title, abstract_preview
        );

        let request = GenerationRequest::new(self.args.model.clone(), prompt);
        
        match ollama.generate(request).await {
            Ok(response) => {
                let text = response.response.trim();
                
                let score = if let Some(score_line) = text.lines().find(|l| l.to_uppercase().contains("SCORE:")) {
                    score_line.split(':')
                        .nth(1)
                        .and_then(|s| s.trim().parse::<f32>().ok())
                        .unwrap_or(0.5)
                } else {
                    text.split_whitespace()
                        .find_map(|word| word.parse::<f32>().ok())
                        .unwrap_or(0.5)
                };

                let reason = if let Some(reason_line) = text.lines().find(|l| l.to_uppercase().contains("REASON:")) {
                    reason_line.split(':').skip(1).collect::<Vec<_>>().join(":").trim().to_string()
                } else {
                    text.lines().skip(1).collect::<Vec<_>>().join(" ").trim().to_string()
                };

                let is_relevant = score >= self.args.min_score;
                Ok((is_relevant, score, reason))
            }
            Err(e) => {
                if self.args.verbose {
                    Self::log(&self.logger, &format!("  [AI] Error: {}", e));
                }
                Ok((true, 0.7, "AI error, accepted by default".to_string()))
            }
        }
    }

    async fn process_result(&mut self, result: &SearchResult, index: usize) -> Result<Option<ScientificPaper>> {
        Self::log(&self.logger, &format!("\n{}", "=".repeat(64)));
        Self::log(&self.logger, &format!("[{}/{}] {}", index + 1, self.args.max_results, &result.title));
        Self::log(&self.logger, &format!("{}", "=".repeat(64)));
        Self::log(&self.logger, &format!("URL: {}", result.url));

        let mut doi = self.extract_doi_from_url(&result.url);
        let mut abstract_text = result.content.clone();
        let mut title = result.title.clone();

        if doi.is_none() || abstract_text.len() < 100 {
            if self.args.verbose {
                Self::log(&self.logger, "   [FETCH] Scraping page for metadata");
            }
            if let Ok((page_abstract, page_doi)) = self.fetch_page_content(&result.url).await {
                if doi.is_none() {
                    doi = page_doi;
                }
                if !page_abstract.is_empty() && page_abstract.len() > abstract_text.len() {
                    abstract_text = page_abstract;
                }
            }
        }

        if let Some(ref doi_str) = doi {
            Self::log(&self.logger, &format!("DOI: {}", doi_str));
            
            if self.processed_dois.contains(doi_str) {
                Self::log(&self.logger, "SKIPPED: Already processed\n");
                return Ok(None);
            }

            if abstract_text.len() < 100 {
                if self.args.verbose {
                    Self::log(&self.logger, "   [API] Fetching metadata from DOI APIs");
                }
                if let Ok((api_title, api_abstract)) = self.fetch_doi_metadata(doi_str).await {
                    if !api_title.is_empty() {
                        title = api_title;
                    }
                    if !api_abstract.is_empty() && api_abstract.len() > abstract_text.len() {
                        abstract_text = api_abstract;
                    }
                }
            }
        } else {
            Self::log(&self.logger, "DOI: Not found");
        }

        if abstract_text.len() > 50 {
            Self::log(&self.logger, &format!("Abstract: {} chars", abstract_text.len()));
            let preview = if abstract_text.len() > 200 {
                format!("{}...", Self::safe_truncate(&abstract_text, 200))
            } else {
                abstract_text.clone()
            };
            Self::log(&self.logger, &format!("   \"{}\"", preview));
        } else {
            Self::log(&self.logger, "Abstract: None found (using title only)");
            abstract_text = title.clone();
        }

        let (is_relevant, score, reason) = if self.use_ai {
            Self::log(&self.logger, "\nAI Evaluation:");
            self.validate_with_ai(&title, &abstract_text, &self.args.subject).await?
        } else {
            (true, 0.8, "AI disabled".to_string())
        };

        Self::log(&self.logger, &format!("   Score: {:.2}/1.0", score));
        Self::log(&self.logger, &format!("   Reason: {}", reason));

        if is_relevant {
            Self::log(&self.logger, "Relevant: Saving");
        } else {
            Self::log(&self.logger, "NOT Relevant: Skipping");
        }

        if !is_relevant {
            return Ok(None);
        }

        sleep(Duration::from_millis(300)).await;

        Ok(Some(ScientificPaper {
            title,
            url: result.url.clone(),
            doi,
            abstract_text,
            relevance_score: score,
        }))
    }

    fn save_doi(&mut self, paper: &ScientificPaper) -> Result<()> {
        let doi_str = paper.doi.as_ref().map(|s| s.as_str()).unwrap_or("NA");
        
        if let Some(doi) = &paper.doi {
            self.processed_dois.insert(doi.clone());
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.args.output)?;

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let separator = "=".repeat(70);
        
        writeln!(file, "\n{}", separator)?;
        writeln!(file, "DOI: {}", doi_str)?;
        writeln!(file, "Title: {}", paper.title)?;
        writeln!(file, "URL: {}", paper.url)?;
        writeln!(file, "Score: {:.2}", paper.relevance_score)?;
        writeln!(file, "Saved: {}", timestamp)?;
        writeln!(file, "Abstract:\n{}", paper.abstract_text)?;
        writeln!(file, "{}\n", separator)?;

        Self::log(&self.logger, &format!("SAVED to: {}", self.args.output));
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        let results = self.search_searxng().await?;
        
        let results_to_process = results.iter()
            .take(self.args.max_results)
            .cloned()
            .collect::<Vec<_>>();

        Self::log(&self.logger, &format!("\nProcessing results: {}\n", results_to_process.len()));

        let mut validated = 0;
        let mut saved = 0;
        let mut skipped = 0;

        for (i, result) in results_to_process.iter().enumerate() {
            match self.process_result(result, i).await {
                Ok(Some(paper)) => {
                    validated += 1;
                    if self.save_doi(&paper).is_ok() {
                        saved += 1;
                    }
                }
                Ok(None) => {
                    skipped += 1;
                }
                Err(e) => {
                    Self::log(&self.logger, &format!("An error occured: {}", e));
                }
            }
            
            if i < results_to_process.len() - 1 {
                sleep(Duration::from_millis(500)).await;
            }
        }

        Self::log(&self.logger, &format!("\n{}", "=".repeat(64)));
        Self::log(&self.logger, "Results");
        Self::log(&self.logger, &format!("{}", "=".repeat(64)));
        Self::log(&self.logger, &format!("Total processed: {}", results_to_process.len()));
        Self::log(&self.logger, &format!("Validated as relevant: {}", validated));
        Self::log(&self.logger, &format!("Saved to file: {}", saved));
        Self::log(&self.logger, &format!("Skipped: {}", skipped));
        Self::log(&self.logger, &format!("Output: {}\n", self.args.output));

        Ok(())
    }
    
    async fn search_searxng(&self) -> Result<Vec<SearchResult>> {
        Self::log(&self.logger, "Searching SearXNG instance\n");
        
        let mut params = vec![
            ("q", self.args.subject.as_str()),
            ("format", "json"),
            ("categories", self.args.category.as_str()),
            ("engines", self.args.engines.as_str()),
        ];

        if !self.args.time_range.is_empty() {
            let time_range_value = self.args.time_range.as_str();
            
            let standard_ranges = ["day", "week", "month", "year"];
            
            let is_multiyear = time_range_value.ends_with("year") && 
                               time_range_value.len() > 4 && 
                               time_range_value[..time_range_value.len()-4].parse::<u32>().is_ok();
            
            if is_multiyear {
                let years = time_range_value[..time_range_value.len()-4].parse::<u32>().unwrap();
                Self::log(&self.logger, &format!("Warning!: Multi-year range '{}year' requested.", years));
                Self::log(&self.logger, "   Most SearXNG instances only support: day, week, month, year");
                Self::log(&self.logger, "   Falling back to 'year' (last 12 months)");
                Self::log(&self.logger, "   Tip: Use --time-range year and manually filter results by date\n");
                params.push(("time_range", "year"));
            } else if standard_ranges.contains(&time_range_value) {
                params.push(("time_range", time_range_value));
                Self::log(&self.logger, &format!("Applying time filter: {}\n", time_range_value));
            } else {
                Self::log(&self.logger, &format!("Warning: Invalid time range '{}'. Valid options: day, week, month, year", time_range_value));
                Self::log(&self.logger, "   Continuing without time filter\n");
            }
        }

        let url = format!("{}/search", self.args.instance.trim_end_matches('/'));
        
        if self.args.verbose {
            Self::log(&self.logger, &format!("[DEBUG] URL: {}", url));
            Self::log(&self.logger, &format!("[DEBUG] Params: {:?}\n", params));
        }
        
        let response = self.client
            .get(&url)
            .query(&params)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "Unable to read error body".to_string());
            let error_msg = format!("\nSearXNG Request Failed:\n   Status: {}\n   URL: {}\n   Params: {:?}\n   Error body: {}\n", status, url, params, error_body);
            Self::log(&self.logger, &error_msg);
            return Err(anyhow!("SearXNG error: {} - {}", status, error_body));
        }

        let data: SearxngResponse = response.json().await?;
        Self::log(&self.logger, &format!("Found {} results from SearXNG\n", data.results.len()));
        
        if self.args.verbose && !data.results.is_empty() {
            Self::log(&self.logger, &format!("[DEBUG] First result engine: {}", data.results[0].engine));
        }
        
        Ok(data.results)
    }
}


#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    if std::env::args().len() <= 1 {
        println!("{}", "=".repeat(64));
        println!("  Researcher");
        println!("{}", "=".repeat(64));
        println!("No CL flags detected");
        println!("Starting web interface on port {}\n", args.web_poort);
        
        web::start_web_server(args.web_poort).await;
        
        Ok(())
    } else {
        let mut scraper = DOIScraper::new(args).await?;
        scraper.run().await
    }
}