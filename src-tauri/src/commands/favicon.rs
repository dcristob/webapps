use std::fs;

use crate::config::storage;

#[tauri::command]
pub async fn fetch_site_info(url: String) -> Result<(String, String), String> {
    let response = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    let body = response.text().await.map_err(|e| e.to_string())?;

    let title = extract_title(&body).unwrap_or_else(|| {
        url::Url::parse(&url)
            .ok()
            .and_then(|u| u.host_str().map(String::from))
            .unwrap_or_else(|| "Untitled".to_string())
    });

    let favicon_url = extract_favicon_url(&body, &url)
        .unwrap_or_else(|| {
            if let Ok(parsed) = url::Url::parse(&url) {
                format!("{}://{}/favicon.ico", parsed.scheme(), parsed.host_str().unwrap_or(""))
            } else {
                String::new()
            }
        });

    let icon_path = if !favicon_url.is_empty() {
        download_favicon(&favicon_url, &title).await.unwrap_or_else(|_| "auto".to_string())
    } else {
        "auto".to_string()
    };

    Ok((title, icon_path))
}

fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title")?;
    let tag_end = lower[start..].find('>')?;
    let content_start = start + tag_end + 1;
    let end = lower[content_start..].find("</title>")?;
    let title = &html[content_start..content_start + end];
    Some(title.trim().to_string())
}

fn extract_favicon_url(html: &str, page_url: &str) -> Option<String> {
    let lower = html.to_lowercase();
    for rel in &["icon", "shortcut icon", "apple-touch-icon"] {
        if let Some(pos) = lower.find(&format!("rel=\"{}\"", rel)) {
            let search_start = if pos > 200 { pos - 200 } else { 0 };
            let search_end = std::cmp::min(pos + 200, html.len());
            let snippet = &html[search_start..search_end];
            let snippet_lower = snippet.to_lowercase();
            if let Some(href_pos) = snippet_lower.find("href=\"") {
                let href_start = href_pos + 6;
                if let Some(href_end) = snippet[href_start..].find('"') {
                    let href = &snippet[href_start..href_start + href_end];
                    return Some(resolve_url(href, page_url));
                }
            }
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
    if let Ok(base_url) = url::Url::parse(base) {
        if let Ok(resolved) = base_url.join(href) {
            return resolved.to_string();
        }
    }
    href.to_string()
}

async fn download_favicon(favicon_url: &str, title: &str) -> Result<String, String> {
    let response = reqwest::get(favicon_url).await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err("Failed to download favicon".to_string());
    }
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    let icons_dir = storage::config_dir().map_err(|e| e.to_string())?.join("icons");
    fs::create_dir_all(&icons_dir).map_err(|e| e.to_string())?;
    let ext = if favicon_url.contains(".png") {
        "png"
    } else if favicon_url.contains(".svg") {
        "svg"
    } else {
        "ico"
    };
    let safe_name: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let filename = format!("{}_{}.{}", safe_name, &uuid::Uuid::new_v4().to_string()[..8], ext);
    let path = icons_dir.join(&filename);
    fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}
