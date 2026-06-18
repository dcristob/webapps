use std::fs;

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use tauri::State;

use crate::config::storage;
use crate::state::AppState;

/// Build a reqwest client with a browser-like User-Agent
fn http_client() -> Result<reqwest::Client, String> {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        ),
    );
    headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
    reqwest::Client::builder()
        .default_headers(headers)
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fetch_site_info(url: String) -> Result<(String, String), String> {
    let client = http_client()?;
    let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let body = response.text().await.map_err(|e| e.to_string())?;

    let title = extract_title(&body).unwrap_or_else(|| {
        url::Url::parse(&url)
            .ok()
            .and_then(|u| u.host_str().map(String::from))
            .unwrap_or_else(|| "Untitled".to_string())
    });

    // Try multiple favicon strategies in order
    let icon_path = try_download_favicon(&client, &body, &url, &title).await;

    Ok((title, icon_path))
}

/// Try multiple strategies to get a favicon, returning the local path or "auto".
async fn try_download_favicon(
    client: &reqwest::Client,
    html: &str,
    page_url: &str,
    title: &str,
) -> String {
    // Strategy 1: Extract candidate URLs from the page's HTML.
    let favicon_urls = extract_favicon_urls(html, page_url);
    download_first_favicon(client, &favicon_urls, page_url, title).await
}

/// Download the first working favicon from a prioritized URL list, then fall
/// back to the root `/favicon.ico` and Google's favicon service.
///
/// `urls` are strategy-1 candidates (highest priority first). `page_url` is the
/// page the icons belong to, used to build the fallback URLs. Returns a local
/// file path on success or `"auto"` if every strategy fails.
async fn download_first_favicon(
    client: &reqwest::Client,
    urls: &[String],
    page_url: &str,
    title: &str,
) -> String {
    // Strategy 1: try each provided candidate URL in priority order.
    for favicon_url in urls {
        if let Ok(path) = download_favicon(client, favicon_url, title).await {
            return path;
        }
    }

    // Strategy 2: Try /favicon.ico at the root (skip if already in the list).
    if let Ok(parsed) = url::Url::parse(page_url) {
        let root_favicon = format!(
            "{}://{}/favicon.ico",
            parsed.scheme(),
            parsed.host_str().unwrap_or("")
        );
        if !urls.iter().any(|u| u == &root_favicon) {
            if let Ok(path) = download_favicon(client, &root_favicon, title).await {
                return path;
            }
        }
    }

    // Strategy 3: Use Google's favicon service as fallback.
    if let Ok(parsed) = url::Url::parse(page_url) {
        if let Some(domain) = parsed.host_str() {
            let google_url = format!(
                "https://www.google.com/s2/favicons?domain={}&sz=64",
                domain
            );
            if let Ok(path) = download_favicon(client, &google_url, title).await {
                return path;
            }
        }
    }

    "auto".to_string()
}

fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title")?;
    let tag_end = lower[start..].find('>')?;
    let content_start = start + tag_end + 1;
    let end = lower[content_start..].find("</title>")?;
    let title = &html[content_start..content_start + end];
    let title = title.trim();
    if title.is_empty() {
        None
    } else {
        Some(decode_html_entities(title))
    }
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

/// Extract all favicon URLs from HTML, ordered by preference (larger/better formats first)
fn extract_favicon_urls(html: &str, page_url: &str) -> Vec<String> {
    let mut results: Vec<(i32, String)> = Vec::new();
    let lower = html.to_lowercase();

    // Find all <link> tags
    let mut search_from = 0;
    while let Some(link_start) = lower[search_from..].find("<link") {
        let abs_start = search_from + link_start;
        let tag_end = match lower[abs_start..].find('>') {
            Some(end) => abs_start + end,
            None => break,
        };
        let tag = &html[abs_start..=tag_end];
        let tag_lower = &lower[abs_start..=tag_end];

        search_from = tag_end + 1;

        // Check if this link tag has a relevant rel attribute
        let rel = extract_attr(tag, tag_lower, "rel");
        let rel_lower = rel.as_deref().unwrap_or("").to_lowercase();

        let is_icon = rel_lower.contains("icon");
        if !is_icon {
            continue;
        }

        let href = match extract_attr(tag, tag_lower, "href") {
            Some(h) if !h.is_empty() => resolve_url(&h, page_url),
            _ => continue,
        };

        // Prioritize: apple-touch-icon > larger sizes > generic icon
        let priority = if rel_lower.contains("apple-touch-icon") {
            10
        } else {
            // Check sizes attribute for larger icons
            let sizes = extract_attr(tag, tag_lower, "sizes")
                .unwrap_or_default();
            parse_icon_size(&sizes)
        };

        results.push((priority, href));
    }

    // Also look for <meta> og:image as last resort
    if let Some(og_image) = extract_og_image(html, &lower, page_url) {
        results.push((-1, og_image));
    }

    // Sort by priority descending (highest = best)
    results.sort_by(|a, b| b.0.cmp(&a.0));
    results.into_iter().map(|(_, url)| url).collect()
}

/// Extract an attribute value from a tag, handling both single and double quotes
fn extract_attr(tag: &str, tag_lower: &str, attr_name: &str) -> Option<String> {
    let patterns = [
        format!("{}=\"", attr_name),
        format!("{}='", attr_name),
        format!("{} = \"", attr_name),
        format!("{} = '", attr_name),
    ];
    let closers = ['"', '\'', '"', '\''];

    for (pattern, closer) in patterns.iter().zip(closers.iter()) {
        if let Some(pos) = tag_lower.find(pattern.as_str()) {
            let value_start = pos + pattern.len();
            if let Some(value_end) = tag[value_start..].find(*closer) {
                return Some(tag[value_start..value_start + value_end].to_string());
            }
        }
    }
    None
}

/// Parse icon size string like "32x32" or "192x192" into a priority score
fn parse_icon_size(sizes: &str) -> i32 {
    if sizes.is_empty() {
        return 0;
    }
    if sizes.to_lowercase() == "any" {
        return 8; // SVG, very good
    }
    // Parse "NxN" format
    let parts: Vec<&str> = sizes.split('x').collect();
    if parts.len() == 2 {
        if let Ok(size) = parts[0].trim().parse::<i32>() {
            return match size {
                s if s >= 128 => 7,
                s if s >= 64 => 5,
                s if s >= 32 => 3,
                _ => 1,
            };
        }
    }
    0
}

/// Try to extract og:image from meta tags
fn extract_og_image(html: &str, lower: &str, page_url: &str) -> Option<String> {
    let marker = "property=\"og:image\"";
    if let Some(pos) = lower.find(marker) {
        let search_start = if pos > 300 { pos - 300 } else { 0 };
        let search_end = std::cmp::min(pos + 300, html.len());
        let snippet = &html[search_start..search_end];
        let snippet_lower = &lower[search_start..search_end];
        if let Some(href) = extract_attr(snippet, snippet_lower, "content") {
            return Some(resolve_url(&href, page_url));
        }
    }
    None
}

fn resolve_url(href: &str, base: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    if href.starts_with("//") {
        return format!("https:{}", href);
    }
    if href.starts_with("data:") {
        return href.to_string();
    }
    if let Ok(base_url) = url::Url::parse(base) {
        if let Ok(resolved) = base_url.join(href) {
            return resolved.to_string();
        }
    }
    href.to_string()
}

async fn download_favicon(
    client: &reqwest::Client,
    favicon_url: &str,
    title: &str,
) -> Result<String, String> {
    // Skip data: URIs for now (could be handled separately)
    if favicon_url.starts_with("data:") {
        return handle_data_uri(favicon_url, title);
    }

    let response = client
        .get(favicon_url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    let bytes = response.bytes().await.map_err(|e| e.to_string())?;

    if bytes.len() < 4 {
        return Err("Downloaded file too small to be a valid image".to_string());
    }

    // Detect actual file type from magic bytes (don't trust Content-Type)
    let ext = detect_image_format(&bytes)
        .ok_or_else(|| "Not a recognized image format".to_string())?;

    save_icon(&bytes, title, ext)
}

/// Handle data: URIs (inline base64 icons)
fn handle_data_uri(data_uri: &str, title: &str) -> Result<String, String> {
    use std::io::Read;
    let parts: Vec<&str> = data_uri.splitn(2, ',').collect();
    if parts.len() != 2 {
        return Err("Invalid data URI".to_string());
    }

    let header = parts[0]; // e.g. "data:image/png;base64"
    let data = parts[1];

    let is_base64 = header.contains("base64");
    let bytes = if is_base64 {
        // Simple base64 decode
        let cleaned: String = data.chars().filter(|c| !c.is_whitespace()).collect();
        base64_decode(&cleaned)?
    } else {
        // URL-encoded
        urlencoding::decode_binary(data.as_bytes())
            .into_owned()
            .bytes()
            .collect::<Result<Vec<u8>, _>>()
            .map_err(|e| e.to_string())?
    };

    if bytes.len() < 4 {
        return Err("Data URI too small".to_string());
    }

    let ext = detect_image_format(&bytes)
        .or_else(|| {
            // Guess from MIME type in header
            if header.contains("image/png") {
                Some("png")
            } else if header.contains("image/svg") {
                Some("svg")
            } else if header.contains("image/x-icon") || header.contains("image/vnd.microsoft.icon") {
                Some("ico")
            } else {
                None
            }
        })
        .ok_or_else(|| "Unknown image format in data URI".to_string())?;

    save_icon(&bytes, title, ext)
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    // Simple base64 decoder
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;

    for &byte in input.as_bytes() {
        let val = if byte == b'=' {
            break;
        } else if let Some(pos) = TABLE.iter().position(|&b| b == byte) {
            pos as u32
        } else {
            continue; // skip unknown chars
        };
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(output)
}

fn save_icon(bytes: &[u8], title: &str, ext: &str) -> Result<String, String> {
    let icons_dir = storage::config_dir()
        .map_err(|e| e.to_string())?
        .join("icons");
    fs::create_dir_all(&icons_dir).map_err(|e| e.to_string())?;

    let safe_name: String = title
        .chars()
        .take(50)
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let filename = format!(
        "{}_{}.{}",
        safe_name,
        &uuid::Uuid::new_v4().to_string()[..8],
        ext
    );
    let path = icons_dir.join(&filename);
    fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

fn detect_image_format(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() < 4 {
        return None;
    }

    // PNG: 89 50 4E 47
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return Some("png");
    }

    // JPEG: FF D8 FF
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg");
    }

    // GIF: 47 49 46 38
    if bytes.starts_with(&[0x47, 0x49, 0x46, 0x38]) {
        return Some("gif");
    }

    // ICO: 00 00 01 00 (Windows icon)
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return Some("ico");
    }

    // WebP: RIFF....WEBP
    if bytes.len() >= 12
        && bytes.starts_with(&[0x52, 0x49, 0x46, 0x46])
        && bytes[8..12] == [0x57, 0x45, 0x42, 0x50]
    {
        return Some("webp");
    }

    // BMP: 42 4D
    if bytes.starts_with(&[0x42, 0x4D]) {
        return Some("bmp");
    }

    // SVG: look for <svg or <?xml in first 1KB (may have BOM, whitespace, comments)
    let check_len = std::cmp::min(bytes.len(), 1024);
    let start = std::str::from_utf8(&bytes[..check_len]).unwrap_or("");
    let trimmed = start.trim_start_matches('\u{feff}').trim();
    if trimmed.starts_with("<?xml") || trimmed.starts_with("<svg") || trimmed.contains("<svg") {
        return Some("svg");
    }

    None
}

/// Receiver half of the icon-capture bridge. Called from JS (via
/// `__TAURI_INTERNALS__.invoke`) once `build_favicon_capture_js` has collected
/// the favicon URLs from the live webview DOM. Resolves the oneshot that
/// `refetch_app_icon` is awaiting. No-op if no capture is pending for this app
/// (e.g. a stale injection arriving after `refetch_app_icon` timed out).
#[tauri::command]
pub fn capture_favicon_done(app_id: String, urls: Vec<String>, state: State<'_, AppState>) -> Result<(), String> {
    let mut pending = state.pending_icon_captures.lock().map_err(|e| e.to_string())?;
    if let Some(sender) = pending.remove(&app_id) {
        // Receiver may have been dropped on timeout; ignore send errors.
        let _ = sender.send(urls);
    }
    Ok(())
}

/// Build the favicon-capture script injected into an app webview during an
/// icon refetch. Mirrors the priority logic of `extract_favicon_urls`
/// (`apple-touch-icon` > larger `sizes` > generic icon > `og:image`).
///
/// The script waits for the page to finish loading (+800 ms debounce so
/// SPA-injected favicons settle), collects the prioritized favicon URLs from
/// the live DOM, and invokes the `capture_favicon_done` Tauri command with the
/// result. An idempotency guard prevents a second capture if injected twice.
pub fn build_favicon_capture_js(app_id: &str) -> String {
    let app_id_literal = serde_json::to_string(app_id).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"
(function() {{
  if (window.__webapps_icon_captured) return;
  var APP_ID = {app_id_literal};

  function priorityFor(el) {{
    var rel = (el.getAttribute('rel') || '').toLowerCase();
    if (rel.indexOf('apple-touch-icon') !== -1) return 10;
    var sizes = (el.getAttribute('sizes') || '').toLowerCase();
    if (sizes === 'any') return 8; // SVG / scalable, very good
    var m = sizes.match(/(\d+)x\d+/);
    if (m) {{
      var n = parseInt(m[1], 10);
      return n >= 128 ? 7 : n >= 64 ? 5 : n >= 32 ? 3 : 1;
    }}
    return 0;
  }}

  function resolve(href, base) {{
    try {{ return new URL(href, base).href; }}
    catch (e) {{ return null; }}
  }}

  function run() {{
    if (window.__webapps_icon_captured) return;
    window.__webapps_icon_captured = true;

    var base = window.location.href;
    var results = [];
    var links = document.querySelectorAll('link[rel]');
    links.forEach(function(el) {{
      var rel = (el.getAttribute('rel') || '').toLowerCase();
      if (rel.indexOf('icon') === -1) return; // mirrors Rust rel.contains("icon")
      var href = el.getAttribute('href');
      if (!href) return;
      var resolved = resolve(href, base);
      if (resolved) results.push({{ p: priorityFor(el), u: resolved }});
    }});

    // og:image as a last-resort candidate.
    var og = document.querySelector('meta[property="og:image"]');
    if (og) {{
      var content = og.getAttribute('content');
      if (content) {{
        var resolved = resolve(content, base);
        if (resolved) results.push({{ p: -1, u: resolved }});
      }}
    }}

    // Highest priority first.
    results.sort(function(a, b) {{ return b.p - a.p; }});
    var urls = results.map(function(r) {{ return r.u; }});

    if (window.__TAURI_INTERNALS__) {{
      try {{
        window.__TAURI_INTERNALS__.invoke('capture_favicon_done', {{ appId: APP_ID, urls: urls }});
      }} catch (e) {{}}
    }}
  }}

  if (document.readyState === 'complete') {{
    setTimeout(run, 800);
  }} else {{
    window.addEventListener('load', function() {{ setTimeout(run, 800); }});
  }}
}})();
"#
    )
}

#[cfg(test)]
mod capture_tests {
    use super::build_favicon_capture_js;

    #[test]
    fn bakes_in_app_id_as_quoted_literal() {
        let js = build_favicon_capture_js("app-123");
        // The app id is embedded as a JS string literal.
        assert!(js.contains(r#"var APP_ID = "app-123";"#));
    }

    #[test]
    fn invokes_capture_favicon_done_command() {
        let js = build_favicon_capture_js("app-1");
        assert!(js.contains("capture_favicon_done"));
        assert!(js.contains("appId"));
        assert!(js.contains("urls"));
    }

    #[test]
    fn has_idempotency_guard() {
        let js = build_favicon_capture_js("app-1");
        // Prevents double-capture if injected twice (e.g. once by refetch's
        // direct eval and once by the on_page_load hook).
        assert!(js.contains("__webapps_icon_captured"));
    }

    #[test]
    fn waits_for_load_with_debounce() {
        let js = build_favicon_capture_js("app-1");
        // Waits for document.readyState === 'complete', then debounces so
        // SPA-set favicons (e.g. Google's client-side <link> injection) settle.
        assert!(js.contains("readyState"));
        assert!(js.contains("800"));
    }

    #[test]
    fn prioritizes_apple_touch_and_larger_sizes() {
        let js = build_favicon_capture_js("app-1");
        // apple-touch-icon gets the top priority.
        assert!(js.contains("apple-touch-icon"));
        assert!(js.contains("return 10"));
        // Sorts candidates by priority descending.
        assert!(js.contains("sort"));
    }

    #[test]
    fn resolves_relative_urls_and_includes_og_image() {
        let js = build_favicon_capture_js("app-1");
        assert!(js.contains("new URL("));
        assert!(js.contains("og:image"));
    }
}
