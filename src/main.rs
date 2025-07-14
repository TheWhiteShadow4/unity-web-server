use std::convert::Infallible;
use warp::Filter;
use clap::Parser;
use tracing::{info, error};
use std::net::TcpListener;

#[derive(Debug)]
struct ServerError;
impl warp::reject::Reject for ServerError {}

/// Find the next available port starting from the given port
fn find_available_port(start_port: u16) -> Option<u16> {
    (start_port..=start_port + 10).find(|port| {
        TcpListener::bind(("127.0.0.1", *port)).is_ok()
    })
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Server port
    #[arg(short, long, default_value_t = 8080)]
    port: u16,
    
    /// Host address
    #[arg(short = 'H', long, default_value = "127.0.0.1")]
    host: String,

    /// Start server without opening browser
    #[arg(short = 'n', long)]
    headless: bool,
}

#[derive(Debug)]
struct CanvasSize {
    width: u32,
    height: u32,
}

fn detect_canvas_size(build_path: &str) -> Option<CanvasSize> {
    use std::path::Path;
    use regex::Regex;
    
    let index_path = Path::new(build_path).join("index.html");
    if let Ok(content) = std::fs::read_to_string(index_path) {
        // Search for Unity Canvas pattern
        let re = Regex::new(r#"<canvas[^>]*id="unity-canvas"[^>]*width=(\d+)[^>]*height=(\d+)"#).unwrap();
        if let Some(caps) = re.captures(&content) {
            return Some(CanvasSize {
                width: caps.get(1).unwrap().as_str().parse().unwrap_or(1600),
                height: caps.get(2).unwrap().as_str().parse().unwrap_or(900)
            });
        }
    }
    None
}

fn find_unity_build_path() -> String {
    use std::path::Path;
    
    // Possible build paths
    let possible_paths = [
        // Server is next to Build folder
        "../Build",
        // Server is in Build folder
        ".",
        // Server is next to build folder (lowercase)
        "../build",
        // Absolute fallback paths
        "./Build",
        "./build"
    ];
    
    // Search for index.html as indicator for Unity WebGL Build
    for path in possible_paths.iter() {
        let full_path = Path::new(path).join("index.html");
        if full_path.exists() {
            info!("Unity Build found in: {}", path);
            return path.to_string();
        }
    }
    
    // Fallback to first path if nothing found
    info!("No Unity Build found, using default path: {}", possible_paths[0]);
    possible_paths[0].to_string()
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    info!("Unity WebGL Development Server  Copyright (C) by TheWhiteShadow");
    info!("This program comes with ABSOLUTELY NO WARRANTY.");
    info!("This is free software, and you are welcome to redistribute it");
    info!("under the terms of the GNU General Public License v3.0");
    info!("");
    
    let args = Args::parse();
    
    // Find build path automatically
    let unity_build_path = find_unity_build_path();
    
    // Try to find an available port
    let port = match find_available_port(args.port) {
        Some(port) => port,
        None => {
            error!("Could not find an available port in range {}-{}!", args.port, args.port + 10);
            std::process::exit(1);
        }
    };

    // If we're using a different port than requested, inform the user
    if port != args.port {
        info!("Port {} was in use, using port {} instead", args.port, port);
    }
    
    info!("Starting Unity WebGL Server...");
    info!("Host: {}", args.host);
    info!("Port: {}", port);
    info!("Unity Build Path: {}", unity_build_path);
    info!("Mode: {}", if args.headless { "Headless" } else { "Browser" });
    
    // CORS configuration for Unity WebGL
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type", "authorization"])
        .allow_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"]);
    
    // Route for main HTML page
    let index_route = warp::path::end()
        .and(warp::get())
        .and_then(serve_index_html);
    
    // Route for Unity WebGL Build files with special handling
    let unity_build_path_clone = unity_build_path.clone();
    let unity_files = warp::path("Build")
        .and(warp::path::tail())
        .and(warp::get())
        .and_then(move |tail: warp::path::Tail| {
            let build_path = unity_build_path_clone.clone();
            serve_unity_file(build_path, tail.as_str().to_string())
        });
    
    // Health check endpoint
    let health = warp::path("health")
        .and(warp::path::end())
        .and(warp::get())
        .map(|| "OK");
    
    // Combine all routes
    let routes = index_route
        .or(unity_files)
        .or(health)
        .with(cors)
        .with(warp::log("unity_webgl_server"));
    
    // Parse host address with new port
    let addr: std::net::SocketAddr = format!("{}:{}", args.host, port)
        .parse()
        .expect("Invalid host:port combination");
    
    info!("Server running at http://{}", addr);
    
    // Open browser only if not in headless mode
    if !args.headless {
        let url = format!("http://{}", addr);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if let Err(e) = open::that(&url) {
                error!("Could not open browser: {}", e);
            }
        });
    }

    warp::serve(routes)
        .run(addr)
        .await;
}

async fn serve_unity_file(build_path: String, file_path: String) -> Result<impl warp::Reply, warp::Rejection> {
    use std::path::Path;
    use warp::http::{Response, StatusCode};
    
    let full_path = dunce::canonicalize(Path::new(&build_path).join(&file_path)).unwrap();
    
    info!("Serving Unity file: {}", full_path.display());

    // Check if it's a directory
    if full_path.is_dir() {
        // Check for index.html first
        let index_path = full_path.join("index.html");
        if index_path.exists() {
            info!("Serving index.html from directory");
            match tokio::fs::read(&index_path).await {
                Ok(contents) => {
                    return Ok(Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "text/html")
                        .body(contents)
                        .unwrap());
                }
                Err(e) => {
                    error!("Error reading index.html {}: {}", index_path.display(), e);
                    return Err(warp::reject::not_found());
                }
            }
        }

        // Fallback: Directory listing
        match std::fs::read_dir(&full_path) {
            Ok(entries) => {
                let mut html = String::from("<!DOCTYPE html><html><head><title>Directory Listing</title></head><body><h1>Directory Listing</h1><ul>");
                
                // Link to parent directory if not in root
                if !file_path.is_empty() {
                    html.push_str("<li><a href=\"../\">..</a></li>");
                }
                
                for entry in entries {
                    if let Ok(entry) = entry {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                        // Add "/" at the end of directories
                        let display_name = if is_dir { format!("{}/", name) } else { name.clone() };
                        html.push_str(&format!("<li><a href=\"{}\">{}</a></li>", name, display_name));
                    }
                }
                
                html.push_str("</ul></body></html>");
                
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "text/html")
                    .body(html.into_bytes())
                    .unwrap());
            }
            Err(e) => {
                error!("Error reading directory {}: {}", full_path.display(), e);
                return Err(warp::reject::not_found());
            }
        }
    }
    
    // Rest of function for normal files
    match tokio::fs::read(&full_path).await {
        Ok(contents) => {
            let content_type = get_content_type(&file_path);
            let mut builder = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", content_type)
                .header("cache-control", "public, max-age=31536000");
            
            // Unity WebGL specific headers for compressed files
            if file_path.ends_with(".br") {
                // Brotli compressed files
                builder = builder.header("content-encoding", "br");
                info!("Serving Brotli-compressed file: {}", file_path);
            } else if file_path.ends_with(".gz") {
                // Gzip compressed files
                builder = builder.header("content-encoding", "gzip");
                info!("Serving Gzip-compressed file: {}", file_path);
            }
            
            match builder.body(contents) {
                Ok(response) => Ok(response),
                Err(e) => {
                    error!("Error building response: {}", e);
                    Err(warp::reject::custom(ServerError))
                }
            }
        }
        Err(e) => {
            error!("Error reading Unity file {}: {}", full_path.display(), e);
            Err(warp::reject::not_found())
        }
    }
}

fn get_content_type(file_path: &str) -> &'static str {
    // Unity WebGL specific MIME-Types
    if file_path.ends_with(".wasm") || file_path.ends_with(".wasm.br") || file_path.ends_with(".wasm.gz") {
        "application/wasm"
    } else if file_path.ends_with(".js") || file_path.ends_with(".js.br") || file_path.ends_with(".js.gz") {
        "application/javascript"
    } else if file_path.ends_with(".data") || file_path.ends_with(".data.br") || file_path.ends_with(".data.gz") {
        "application/octet-stream"
    } else if file_path.ends_with(".symbols.json") || file_path.ends_with(".symbols.json.br") || file_path.ends_with(".symbols.json.gz") {
        "application/json"
    } else if file_path.ends_with(".html") {
        "text/html"
    } else if file_path.ends_with(".css") {
        "text/css"
    } else if file_path.ends_with(".png") {
        "image/png"
    } else if file_path.ends_with(".jpg") || file_path.ends_with(".jpeg") {
        "image/jpeg"
    } else if file_path.ends_with(".ico") {
        "image/x-icon"
    } else {
        "application/octet-stream"
    }
}

async fn serve_index_html() -> Result<impl warp::Reply, Infallible> {
    // Try to detect canvas size
    let canvas_size = detect_canvas_size(&find_unity_build_path())
        .unwrap_or(CanvasSize { width: 1600, height: 900 });

    // Calculate container size (game size + padding for controls)
    let container_width = canvas_size.width + 40;

    let html = format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Unity WebGL Game</title>
    <style>
        :root {{
            --bg-color: #1e1e1e;
            --container-bg: #2d2d2d;
            --text-color: #e0e0e0;
            --border-color: #404040;
            --accent-color: #0098ff;
            --accent-hover: #00b4ff;
            --status-bg: #264f3e;
            --status-border: #2d7355;
            --status-text: #a8e6c6;
            --input-bg: #363636;
        }}
        body {{
            font-family: Arial, sans-serif;
            margin: 0;
            padding: 20px;
            background-color: var(--bg-color);
            color: var(--text-color);
            display: flex;
            flex-direction: column;
            align-items: center;
        }}
        .game-container {{
            background-color: var(--container-bg);
            padding-y: 20px;
            border-radius: 10px;
            box-shadow: 0 4px 12px rgba(0,0,0,0.3);
            margin-bottom: 20px;
            width: 90%;
            max-width: {}px;
        }}
        .game-frame {{
            width: 100%;
            border: none;
            border-radius: 5px;
            margin-bottom: 15px;
            background-color: var(--container-bg);
        }}
        .controls {{
            display: flex;
            justify-content: center;
            gap: 10px;
            flex-wrap: wrap;
            margin: 15px 0;
        }}
        .button {{
            display: inline-block;
            padding: 10px 20px;
            background-color: var(--accent-color);
            color: var(--text-color);
            text-decoration: none;
            border-radius: 5px;
            transition: all 0.2s ease;
            border: none;
            cursor: pointer;
            font-size: 14px;
        }}
        .button:hover {{
            background-color: var(--accent-hover);
            transform: translateY(-1px);
        }}
        .status {{
            background-color: var(--status-bg);
            border: 1px solid var(--status-border);
            color: var(--status-text);
            padding: 15px;
            border-radius: 5px;
            margin: 20px 0;
            width: 90%;
            max-width: {}px;
            text-align: center;
        }}
        .host-select, .size-input {{
            padding: 8px;
            border-radius: 5px;
            border: 1px solid var(--border-color);
            background-color: var(--input-bg);
            color: var(--text-color);
            margin-right: 10px;
            font-size: 14px;
        }}
        .size-input {{
            width: 80px;
        }}
        .size-controls {{
            display: flex;
            align-items: center;
            gap: 10px;
            margin-right: 20px;
        }}
        select option {{
            background-color: var(--container-bg);
            color: var(--text-color);
        }}
        /* Improved visibility for × separator */
        .size-controls span {{
            color: var(--text-color);
            font-size: 16px;
            padding: 0 5px;
        }}
    </style>
</head>
<body>
    <div class="game-container">
        <div class="controls">
            <div class="size-controls">
                <select id="sizeSelect" class="host-select" onchange="updateSize()">
                    <option value="auto">Auto-Detect size</option>
                    <option value="manual">Manually set size</option>
                </select>
                <input type="number" id="widthInput" class="size-input" value="{}" style="display: none;" onchange="updateManualSize()">
                <span style="display: none;">×</span>
                <input type="number" id="heightInput" class="size-input" value="{}" style="display: none;" onchange="updateManualSize()">
            </div>
            <button onclick="toggleFullscreen()" class="button">Fullscreen</button>
            <a href="/Build/index.html" class="button" target="_blank">Open in new tab</a>
        </div>

        <iframe id="gameFrame" 
            src="/Build/index.html" 
            class="game-frame"
            allowtransparency="true"
            webkitallowfullscreen="true"
            mozallowfullscreen="true"
            msallowfullscreen="true"
            allowfullscreen="true"
            scrolling="no"
            frameborder="0"
            style="aspect-ratio: {}/{};"
            allow="autoplay; fullscreen *; geolocation; microphone; camera; midi; monetization; xr-spatial-tracking; gamepad; gyroscope; accelerometer; xr; cross-origin-isolated; web-share">
        </iframe>
    </div>
    
    <div class="status">
        <strong>✅ Server Status:</strong> Running with Unity WebGL support
        <br>
        <small>Detected game size: {}×{}</small>
    </div>

    <script>
        const originalSize = {{ width: {}, height: {} }};
        let currentSize = {{ ...originalSize }};

        function updateSize() {{
            const sizeSelect = document.getElementById('sizeSelect');
            const widthInput = document.getElementById('widthInput');
            const heightInput = document.getElementById('heightInput');
            const spans = document.querySelectorAll('.size-controls span');
            
            if (sizeSelect.value === 'manual') {{
                widthInput.style.display = 'inline-block';
                heightInput.style.display = 'inline-block';
                spans.forEach(span => span.style.display = 'inline');
            }} else {{
                widthInput.style.display = 'none';
                heightInput.style.display = 'none';
                spans.forEach(span => span.style.display = 'none');
                currentSize = {{ ...originalSize }};
                updateAspectRatio();
            }}
        }}

        function updateManualSize() {{
            const widthInput = document.getElementById('widthInput');
            const heightInput = document.getElementById('heightInput');
            
            currentSize.width = parseInt(widthInput.value) || originalSize.width;
            currentSize.height = parseInt(heightInput.value) || originalSize.height;
            
            updateAspectRatio();
        }}

        function updateAspectRatio() {{
            const frame = document.getElementById('gameFrame');
            frame.style.aspectRatio = `${{currentSize.width}}/${{currentSize.height}}`;
        }}

        function toggleFullscreen() {{
            const frame = document.getElementById('gameFrame');
            if (frame.requestFullscreen) {{
                frame.requestFullscreen();
            }} else if (frame.webkitRequestFullscreen) {{
                frame.webkitRequestFullscreen();
            }} else if (frame.msRequestFullscreen) {{
                frame.msRequestFullscreen();
            }}
        }}
    </script>
</body>
</html>"#, 
        container_width, // max-width for game-container
        container_width, // max-width for status
        canvas_size.width, // For input fields
        canvas_size.height, // For input fields
        canvas_size.width, // For aspect-ratio
        canvas_size.height, // For aspect-ratio
        canvas_size.width, // For status display
        canvas_size.height, // For status display
        canvas_size.width, // For JavaScript variables
        canvas_size.height  // For JavaScript variables
    );
    
    Ok(warp::reply::html(html))
}
