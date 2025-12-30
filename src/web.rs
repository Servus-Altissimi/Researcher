// A lot of the code here is taken from an older project: https://github.com/Servus-Altissimi/marktplaats-monitor

use crate::{DOIScraper, Args};
use std::sync::{Arc, Mutex};
use std::fs;
use std::io::{BufRead, BufReader};
use warp::{Filter, Reply};
use serde::{Deserialize, Serialize};
use chrono::Local;

#[derive(Debug, Serialize)]
struct PaperResult {
    doi: String,
    title: String,
    url: String,
    score: f32,
    abstract_text: String,
    timestamp: String,
}

#[derive(Debug, Serialize)]
struct StatusMessage {
    status: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchRequest {
    subject: String,
    instance: String,
    max_results: usize,
    model: String,
    no_ai: bool,
    time_range: String,
    category: String,
    engines: String,
    min_score: f32,
    ollama_url: String,
}

#[derive(Debug, Deserialize)]
struct ValidateRequest {
    url: String,
    service_type: String,
}

pub async fn start_web_server(port: u16) {
    let logs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    
    let logs_filter = warp::any().map(move || logs.clone());

    let index = warp::get()
        .and(warp::path::end())
        .map(|| {
            let html = index_html();
            warp::reply::html(html)
        });

    let results = warp::get()
        .and(warp::path("results"))
        .and(warp::query::<SearchQuery>())
        .and_then(get_results);

    let search = warp::post()
        .and(warp::path("search"))
        .and(warp::body::json())
        .and(logs_filter.clone())
        .map(|request: SearchRequest, logs: Arc<Mutex<Vec<String>>>| {
            let args = Args {
                subject: request.subject.clone(),
                instance: request.instance,
                max_results: request.max_results,
                output: "results.txt".to_string(),
                model: request.model,
                no_ai: request.no_ai,
                time_range: request.time_range,
                category: request.category,
                engines: request.engines,
                min_score: request.min_score,
                verbose: false,
                web_poort: 6601,
                ollama_url: request.ollama_url,
            };
            
            add_log(&logs, &format!("Starting search for: {}", request.subject));
            
            tokio::spawn(async move {
                add_log(&logs, "Initializing scraper...");
                
                match DOIScraper::new_with_logger(args, Some(logs.clone())).await {
                    Ok(mut scraper) => {
                        add_log(&logs, "Scraper initialized successfully");
                        add_log(&logs, "Beginning search!");
                        
                        match scraper.run().await {
                            Ok(_) => add_log(&logs, "Search completed!"),
                            Err(e) => add_log(&logs, &format!("Search error: {}", e)),
                        }
                    }
                    Err(e) => add_log(&logs, &format!("Failed to init scraper: {}", e)),
                }
            });
            
            warp::reply::json(&StatusMessage {
                status: "ok".to_string(),
                message: "Search started in background".to_string(),
            })
        });

    let clear = warp::post()
        .and(warp::path("clear_results"))
        .and_then(clear_all_results);

    let validate = warp::post()
        .and(warp::path("validate"))
        .and(warp::body::json())
        .and_then(validate_service);

    let get_logs = warp::get()
        .and(warp::path("logs"))
        .and(logs_filter.clone())
        .map(|logs: Arc<Mutex<Vec<String>>>| {
            let logs = logs.lock().unwrap();
            warp::reply::json(&*logs)
        });

    let routes = index
        .or(results)
        .or(search)
        .or(clear)
        .or(validate)
        .or(get_logs);

    println!("Web interface running on http://localhost:{}", port);
    warp::serve(routes).run(([127, 0, 0, 1], port)).await;
}

async fn get_results(query: SearchQuery) -> Result<impl Reply, warp::Rejection> {
    let filepath = "results.txt";
    let mut results = Vec::new();
    
    if let Ok(file) = fs::File::open(filepath) {
        let reader = BufReader::new(file);
        let mut current_paper: Option<PaperResult> = None;
        let mut abstract_lines = Vec::new();
        let mut in_abstract = false;
        
        for line in reader.lines() {
            if let Ok(line) = line {
                if line.starts_with("====") {
                    if let Some(mut paper) = current_paper.take() {
                        if !abstract_lines.is_empty() {
                            paper.abstract_text = abstract_lines.join(" ").trim().to_string();
                            abstract_lines.clear();
                        }
                        
                        // Only add papers with score > 0.0, redundant safeguard,
                        if paper.score > 0.0 {
                            results.push(paper);
                        }
                    }
                    in_abstract = false;
                    current_paper = Some(PaperResult {
                        doi: String::new(),
                        title: String::new(),
                        url: String::new(),
                        score: 0.0,
                        abstract_text: String::new(),
                        timestamp: String::new(),
                    });
                } else if line.starts_with("DOI: ") {
                    if let Some(ref mut paper) = current_paper {
                        paper.doi = line.trim_start_matches("DOI: ").to_string();
                    }
                    in_abstract = false;
                } else if line.starts_with("Title: ") {
                    if let Some(ref mut paper) = current_paper {
                        paper.title = line.trim_start_matches("Title: ").to_string();
                    }
                    in_abstract = false;
                } else if line.starts_with("URL: ") {
                    if let Some(ref mut paper) = current_paper {
                        paper.url = line.trim_start_matches("URL: ").to_string();
                    }
                    in_abstract = false;
                } else if line.starts_with("Score: ") {
                    if let Some(ref mut paper) = current_paper {
                        if let Ok(score) = line.trim_start_matches("Score: ").parse::<f32>() {
                            paper.score = score;
                        }
                    }
                    in_abstract = false;
                } else if line.starts_with("Saved: ") {
                    if let Some(ref mut paper) = current_paper {
                        paper.timestamp = line.trim_start_matches("Saved: ").to_string();
                    }
                    in_abstract = false;
                } else if line.starts_with("Abstract:") {
                    in_abstract = true;
                    abstract_lines.clear();
                } else if in_abstract && !line.trim().is_empty() {
                    abstract_lines.push(line.trim().to_string());
                }
            }
        }
        
        if let Some(mut paper) = current_paper {
            if !abstract_lines.is_empty() {
                paper.abstract_text = abstract_lines.join(" ").trim().to_string();
            }
            // Only add papers with score > 0.0, redundant safeguard,
            if paper.score > 0.0 {
                results.push(paper);
            }
        }
    }
    
    if let Some(search_term) = query.q {
        let search_lower = search_term.to_lowercase();
        results.retain(|r| {
            r.title.to_lowercase().contains(&search_lower) ||
            r.abstract_text.to_lowercase().contains(&search_lower) ||
            r.doi.to_lowercase().contains(&search_lower)
        });
    }
    
    results.reverse();
    
    Ok(warp::reply::json(&results))
}

fn add_log(logs: &Arc<Mutex<Vec<String>>>, message: &str) {
    let timestamp = Local::now().format("%H:%M:%S");
    let log_entry = format!("[{}] {}", timestamp, message);
    
    if let Ok(mut logs) = logs.lock() {
        logs.push(log_entry.clone());
        if logs.len() > 500 {
            logs.remove(0);
        }
    }
    
    println!("{}", log_entry);
}

async fn clear_all_results() -> Result<impl Reply, warp::Rejection> {
    let filepath = "results.txt";
    
    if let Err(_) = fs::write(filepath, "") {
        return Ok(warp::reply::json(&StatusMessage {
            status: "error".to_string(),
            message: "Could not clear results".to_string(),
        }));
    }
    
    Ok(warp::reply::json(&StatusMessage {
        status: "ok".to_string(),
        message: "All results permanently cleared".to_string(),
    }))
}

async fn validate_service(request: ValidateRequest) -> Result<impl Reply, warp::Rejection> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();

    match request.service_type.as_str() {
        "searxng" => {
            let url = format!("{}/search?q=test&format=json", request.url.trim_end_matches('/'));
            
            match client.get(&url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        Ok(warp::reply::json(&StatusMessage {
                            status: "ok".to_string(),
                            message: "SearXNG instance is reachable".to_string(),
                        }))
                    } else {
                        Ok(warp::reply::json(&StatusMessage {
                            status: "error".to_string(),
                            message: format!("SearXNG returned status: {}", response.status()),
                        }))
                    }
                }
                Err(e) => {
                    Ok(warp::reply::json(&StatusMessage {
                        status: "error".to_string(),
                        message: format!("Cannot reach SearXNG: {}", e),
                    }))
                }
            }
        }
        "ollama" => {
            let url = format!("{}/api/tags", request.url.trim_end_matches('/'));
            
            match client.get(&url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        if let Ok(text) = response.text().await {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                if let Some(models) = json.get("models").and_then(|m| m.as_array()) {
                                    let model_names: Vec<String> = models.iter()
                                        .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                                        .map(|s| s.to_string())
                                        .collect();
                                    
                                    return Ok(warp::reply::json(&serde_json::json!({
                                        "status": "ok",
                                        "message": "Ollama is reachable",
                                        "models": model_names
                                    })));
                                }
                            }
                        }
                        Ok(warp::reply::json(&StatusMessage {
                            status: "ok".to_string(),
                            message: "Ollama is reachable".to_string(),
                        }))
                    } else {
                        Ok(warp::reply::json(&StatusMessage {
                            status: "error".to_string(),
                            message: format!("Ollama returned status: {}", response.status()),
                        }))
                    }
                }
                Err(e) => {
                    Ok(warp::reply::json(&StatusMessage {
                        status: "error".to_string(),
                        message: format!("Cannot reach Ollama: {}", e),
                    }))
                }
            }
        }
        _ => {
            Ok(warp::reply::json(&StatusMessage {
                status: "error".to_string(),
                message: "Invalid service type".to_string(),
            }))
        }
    }
}

fn index_html() -> String {
    r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Researcher</title>
    <style>
        body { font-family: Arial; margin: 20px; background: #f5f5f5; min-height: 100vh; display: flex; flex-direction: column; }
        .content { flex: 1; }
        h1 { color: #333; }

        .status-message { padding: 10px; margin: 10px 0; border-radius: 0; display: none; }
        .status-message.success { background: #d4edda; color: #155724; border: 1px solid #c3e6cb; }
        .status-message.error { background: #f8d7da; color: #721c24; border: 1px solid #f5c6cb; }

        .search-bar { margin: 20px 0; }

        input[type="text"], input[type="number"], select { padding: 8px; margin: 5px 0; }

        button { padding: 8px 16px; background: rgb(100, 149, 237); color: white; border: none; cursor: pointer; margin-right: 5px; border-radius: 0; }
        button:hover { background: #5a8dd4; }
        button.danger { background: #dc3545; }
        button.danger:hover { background: #c82333; }
        button.validate { background: #28a745; font-size: 12px; padding: 4px 8px; }
        button.validate:hover { background: #218838; }

        .result { background: white; padding: 15px; margin: 10px 0; border: 1px solid #ddd; border-radius: 0; position: relative; }
        .result h3 { margin: 0 0 10px 0; }

        .result a { color: #007bff; text-decoration: none; }
        .result a:hover { text-decoration: underline; }

        .score { font-weight: bold; color: rgb(0, 150, 255); }
        .info { color: #666; font-size: 14px; }

        .doi-badge { background: #28a745; color: white; padding: 3px 8px; border-radius: 0; font-size: 12px; font-family: monospace; }

        .tabs { margin: 20px 0; border-bottom: 2px solid #ddd; }
        .tab { display: inline-block; padding: 10px 20px; cursor: pointer; background: #e9ecef; margin-right: 5px; border-radius: 0; }
        .tab.active { background: white; border: 1px solid #ddd; border-bottom: none; }

        .tab-content { display: none; }
        .tab-content.active { display: block; }

        .search-form { background: white; padding: 20px; max-width: 800px; border-radius: 0; }
        .search-form label { display: block; margin: 10px 0 5px 0; font-weight: bold; }

        .search-form input[type="text"],
        .search-form input[type="number"],
        .search-form select { width: 100%; box-sizing: border-box; }

        .search-form input[type="checkbox"] { margin-right: 5px; }

        .form-row { display: grid; grid-template-columns: 1fr 1fr; gap: 15px; }
        .form-row-with-button { display: grid; grid-template-columns: 1fr auto; gap: 10px; align-items: end; }

        .abstract { margin-top: 10px; padding: 10px; background: #f9f9f9; border-left: 3px solid #007bff; font-size: 14px; }

        footer { margin-top: 40px; padding: 20px; text-align: center; color: black; border-radius: 0; }
        footer a { color: #4db8ff; text-decoration: none; }
        footer a:hover { text-decoration: underline; }

        .loading { display: none; padding: 10px; background: #fff3cd; border: 1px solid #ffc107; border-radius: 0; margin: 10px 0; }
        .loading.active { display: block; }

        .log-container {
            background: #1e1e1e;
            color: #d4d4d4;
            padding: 15px;
            border-radius: 0;
            max-height: 500px;
            overflow-y: auto;
            font-family: 'Courier New', monospace;
            font-size: 13px;
            margin-top: 15px;
        }

        .log-entry { margin: 3px 0; }

        .validation-status { display: inline-block; padding: 2px 6px; border-radius: 0; font-size: 11px; margin-left: 10px; }
        .validation-status.success { background: #d4edda; color: #155724; }
        .validation-status.error { background: #f8d7da; color: #721c24; }
        .validation-status.checking { background: #fff3cd; color: #856404; }
    </style>
</head>
<body>
    <div class="content">
        <h1>Researcher</h1>

        <div id="status-message" class="status-message"></div>
        <div id="loading" class="loading">Searching and validating...</div>
        
        <div class="tabs">
            <div class="tab active" onclick="showTab(event, 'search')">Search</div>
            <div class="tab" onclick="showTab(event, 'results')">Results</div>
            <div class="tab" onclick="showTab(event, 'logs')">Logs</div>
        </div>
        
        <div id="search-tab" class="tab-content active">
            <div class="search-form">
                <h2>Configure Search</h2>
                
                <label>Subject:</label>
                <input type="text" id="subject" value="machine learning" placeholder="e.g. quantum computing">
                
                <div class="form-row-with-button">
                    <div>
                        <label>SearXNG Instance: <span id="searxng_status" class="validation-status"></span></label>
                        <input type="text" id="instance" value="https://searxng.site/">
                    </div>
                    <button class="validate" onclick="validateSearXNG()">Test Connection</button>
                </div>
                
                <div class="form-row">
                    <div>
                        <label>Max Results:</label>
                        <input type="number" id="max_results" value="50" min="1" max="200">
                    </div>
                    <div>
                        <label>Min Relevance Score:</label>
                        <input type="number" id="min_score" value="0.6" step="0.1" min="0" max="1">
                    </div>
                </div>
                
                <div class="form-row-with-button">
                    <div>
                        <label>Ollama URL: <span id="ollama_status" class="validation-status"></span></label>
                        <input type="text" id="ollama_url" value="http://localhost:11434">
                    </div>
                    <button class="validate" onclick="validateOllama()">Test Connection</button>
                </div>
                
                <div class="form-row">
                    <div>
                        <label>AI Model:</label>
                        <input type="text" id="model" value="llama3.2:latest" placeholder="e.g. llama3.2:latest">
                    </div>
                    <div>
                        <label>
                            <input type="checkbox" id="no_ai"> Disable AI validation
                        </label>
                    </div>
                </div>
                
                <div class="form-row">
                    <div>
                        <label>Time Range:</label>
                        <select id="time_range">
                            <option value="">All time</option>
                            <option value="day">Past day</option>
                            <option value="week">Past week</option>
                            <option value="month">Past month</option>
                            <option value="year">Past year</option>
                        </select>
                    </div>
                    <div>
                        <label>Category:</label>
                        <input type="text" id="category" value="science">
                    </div>
                </div>
                
                <label>Engines (comma-separated):</label>
                <input type="text" id="engines" value="arxiv,pubmed,google scholar,crossref,openairepublications,openairedatasets,semantic scholar">
                
                <br><br>
                <button onclick="startSearch()">Start Search</button>
            </div>
        </div>
        
        <div id="results-tab" class="tab-content">
            <div class="search-bar">
                <input type="text" id="search_term" placeholder="Search in results..." style="width: 400px;">
                <button onclick="searchResults()">Search</button>
                <button onclick="loadResults()">Show All</button>
                <button class="danger" onclick="clearAllResults()">Clear All Results</button>
            </div>
            <div id="results"></div>
        </div>
        
        <div id="logs-tab" class="tab-content">
            <h2>Technical Logs</h2>
            <button onclick="loadLogs()">Refresh Logs</button>
            <button class="danger" onclick="clearLogs()">Clear Display</button>
            <div class="log-container" id="log-container"></div>
        </div>
    </div>
    
    <footer>
        <p><a href="https://github.com/Researcher" target="_blank">GitHub Repo</a></p>
    </footer>
    
    <script>
        // I hate JS
        let logInterval;
        
        function showStatusMessage(message, isSuccess) {
            const element = document.getElementById('status-message');
            element.textContent = message;
            element.className = 'status-message ' + (isSuccess ? 'success' : 'error');
            element.style.display = 'block';
            
            setTimeout(() => {
                element.style.display = 'none';
            }, 5000);
        }
        
        function showTab(e, tabId) {
            document.querySelectorAll('.tab').forEach(t =>
                t.classList.remove('active')
            );
            document.querySelectorAll('.tab-content').forEach(c =>
                c.classList.remove('active')
            );

            e.target.classList.add('active');
            document.getElementById(tabId + '-tab').classList.add('active');

            if (tabId === 'logs') {
                startLogPolling();
            } else {
                stopLogPolling();
            }
        }

        
        function validateSearXNG() {
            const url = document.getElementById('instance').value;
            const status = document.getElementById('searxng_status');
            
            status.textContent = 'Checking..';
            status.className = 'validation-status checking';
            
            fetch('/validate', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ url: url, service_type: 'searxng' })
            })
            .then(r => r.json())
            .then(data => {
                if (data.status === 'ok') {
                    status.textContent = 'Connected!';
                    status.className = 'validation-status success';
                } else {
                    status.textContent = 'Error:' + data.message;
                    status.className = 'validation-status error';
                }
            })
            .catch(err => {
                status.textContent = 'Failed';
                status.className = 'validation-status error';
            });
        }
        
        function validateOllama() {
            const url = document.getElementById('ollama_url').value;
            const status = document.getElementById('ollama_status');
            
            status.textContent = 'Checking..';
            status.className = 'validation-status checking';
            
            fetch('/validate', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ url: url, service_type: 'ollama' })
            })
            .then(r => r.json())
            .then(data => {
                if (data.status === 'ok') {
                    status.textContent = 'Connected!';
                    status.className = 'validation-status success';
                    if (data.models && data.models.length > 0) {
                        showStatusMessage('Ollama connected! Available models: ' + data.models.join(', '), true);
                    }
                } else {
                    status.textContent = 'Error: ' + data.message;
                    status.className = 'validation-status error';
                }
            })
            .catch(err => {
                status.textContent = 'Failed';
                status.className = 'validation-status error';
            });
        }
        
        function startSearch() {
            const request = {
                subject: document.getElementById('subject').value,
                instance: document.getElementById('instance').value,
                max_results: parseInt(document.getElementById('max_results').value),
                model: document.getElementById('model').value,
                no_ai: document.getElementById('no_ai').checked,
                time_range: document.getElementById('time_range').value,
                category: document.getElementById('category').value,
                engines: document.getElementById('engines').value,
                min_score: parseFloat(document.getElementById('min_score').value),
                ollama_url: document.getElementById('ollama_url').value,
            };
            
            document.getElementById('loading').classList.add('active');
            
            fetch('/search', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(request)
            })
            .then(r => r.json())
            .then(data => {
                showStatusMessage(data.message + ' Check the Logs tab for progress.', data.status === 'ok');
                
                setTimeout(() => {
                    document.getElementById('loading').classList.remove('active');
                }, 3000);
                
                setTimeout(() => {
                    document.querySelectorAll('.tab')[2].click();
                }, 1000);
            })
            .catch(err => {
                showStatusMessage('Something went wrong: ' + err, false);
                document.getElementById('loading').classList.remove('active');
            });
        }
        
        function startLogPolling() {
            loadLogs();
            logInterval = setInterval(loadLogs, 2000);
        }
        
        function stopLogPolling() {
            if (logInterval) {
                clearInterval(logInterval);
            }
        }
        
        function loadLogs() {
            fetch('/logs')
                .then(r => r.json())
                .then(logs => {
                    const container = document.getElementById('log-container');
                    const wasScrolledToBottom = container.scrollHeight - container.scrollTop === container.clientHeight;
                    
                    container.innerHTML = '';
                    
                    if (logs.length === 0) {
                        container.innerHTML = '<div class="log-entry">No logs yet. Start a search to see activity.</div>';
                        return;
                    }
                    
                    logs.forEach(log => {
                        const div = document.createElement('div');
                        div.className = 'log-entry';
                        div.textContent = log;
                        container.appendChild(div);
                    });
                    
                    if (wasScrolledToBottom || container.scrollTop === 0) {
                        container.scrollTop = container.scrollHeight;
                    }
                });
        }
        
        function clearLogs() {
            document.getElementById('log-container').innerHTML = '<div class="log-entry">Logs cleared (display only, server logs still active)</div>';
        }
        
        function loadResults() {
            fetch('/results')
                .then(r => r.json())
                .then(data => {
                    const container = document.getElementById('results');
                    container.innerHTML = '';
                    
                    if (data.length === 0) {
                        container.innerHTML = '<p>No results found. Start a new search!</p>';
                        return;
                    }
                    
                    data.forEach(paper => {
                        const div = document.createElement('div');
                        div.className = 'result';
                        
                        const abstractPreview = paper.abstract_text.length > 300
                            ? paper.abstract_text.substring(0, 300) + '...'
                            : paper.abstract_text;
                        
                        div.innerHTML = `
                            <h3><a href="${paper.url}" target="_blank">${paper.title}</a></h3>
                            <div class="info">
                                <span class="doi-badge">${paper.doi}</span>
                                <span class="score">Score: ${paper.score.toFixed(2)}/1.0</span>
                                <span style="float: right;">${paper.timestamp}</span>
                            </div>
                            <div class="abstract">${abstractPreview}</div>
                        `;
                        
                        container.appendChild(div);
                    });
                });
        }

    function searchResults() {
        const searchTerm = document.getElementById('search_term').value;
        fetch('/results?q=' + encodeURIComponent(searchTerm))
            .then(r => r.json())
            .then(data => {
                const container = document.getElementById('results');
                container.innerHTML = '';

                if (data.length === 0) {
                    container.innerHTML = '<p>No results found for this search, maybe check the SearXNG Instance settings.</p>';
                    return;
                }

                data.forEach(paper => {
                    const div = document.createElement('div');
                    div.className = 'result';

                    const abstractPreview = paper.abstract_text.length > 300 
                        ? paper.abstract_text.substring(0, 300) + '...'
                        : paper.abstract_text;

                    div.innerHTML = `
                        <h3><a href="${paper.url}" target="_blank">${paper.title}</a></h3>
                        <div class="info">
                            <span class="doi-badge">${paper.doi}</span>
                            <span class="score">Score: ${paper.score.toFixed(2)}/1.0</span>
                            <span style="float: right;">${paper.timestamp}</span>
                        </div>
                        <div class="abstract">${abstractPreview}</div>
                    `;

                    container.appendChild(div);
                });
            });
    }
    function clearAllResults() {
        if (!confirm('Are you sure you want to clear all results forever?')) {
            return;
        }
        
        fetch('/clear_results', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' }
        })
        .then(r => r.json())
        .then(data => {
            showStatusMessage(data.message, data.status === 'ok');
            if (data.status === 'ok') {
                loadResults();
            }
        })
        .catch(err => {
            showStatusMessage('Something went wrong: ' + err, false);
        });
    }
    
    loadResults();
</script>
</body>
</html>"#.to_string()
}