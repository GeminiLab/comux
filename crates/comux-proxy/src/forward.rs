use axum::http::Uri;
use axum::{body::Body, response::Response};
use bytes::Bytes;
use comux_core::{ConnectionConfig, RedirectPolicy, UpstreamConfig};
use futures_util::StreamExt;
use http::header::{
    HeaderName, AUTHORIZATION, CONNECTION, CONTENT_LENGTH, HOST, LOCATION, TE, TRAILER,
    TRANSFER_ENCODING, UPGRADE,
};
use http::{HeaderMap, Method};
use reqwest::redirect::Policy;
use reqwest::Client;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum ForwardError {
    #[error("invalid upstream URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("upstream request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("upstream redirect chain exceeded 10 hops")]
    TooManyRedirects,
    #[error("upstream redirect had an invalid location")]
    InvalidRedirectLocation,
    #[error("failed to build downstream response: {0}")]
    Response(#[from] http::Error),
}

pub async fn forward_request(
    method: Method,
    request_uri: &Uri,
    request_headers: &HeaderMap,
    request_body: Body,
    upstream: &UpstreamConfig,
    connection: &ConnectionConfig,
) -> Result<Response, ForwardError> {
    let url = join_upstream_url(&upstream.base_url, request_uri)?;
    let redirect = match upstream.redirect_policy {
        RedirectPolicy::Always | RedirectPolicy::Never => Policy::none(),
        RedirectPolicy::Default => Policy::default(),
    };
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(connection.connect_timeout_secs))
        .redirect(redirect)
        .build()?;

    let response = match upstream.redirect_policy {
        RedirectPolicy::Always => {
            let recorded = Arc::new(Mutex::new(Vec::<Bytes>::new()));
            let request_stream = request_body.into_data_stream().map({
                let recorded = Arc::clone(&recorded);
                move |chunk| {
                    chunk
                        .map(|bytes| {
                            recorded
                                .lock()
                                .expect("body recorder lock")
                                .push(bytes.clone());
                            bytes
                        })
                        .map_err(|error| std::io::Error::other(error.to_string()))
                }
            });
            let request_stream = request_stream.then({
                let idle = Duration::from_secs(connection.idle_timeout_secs);
                move |chunk| async move {
                    match tokio::time::timeout(idle, async { chunk }).await {
                        Ok(chunk) => chunk,
                        Err(_) => Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "upstream request body idle timeout",
                        )),
                    }
                }
            });
            let mut current_method = method;
            let mut current_url = url;
            let mut current_body = Some(reqwest::Body::wrap_stream(request_stream));
            let mut response = None;
            for _ in 0..=10 {
                let request = build_request(
                    &client,
                    &current_method,
                    &current_url,
                    request_headers,
                    &upstream.auth_token,
                    current_body.take(),
                );
                let next_response = request.send().await?;
                if !next_response.status().is_redirection() {
                    response = Some(next_response);
                    break;
                }
                let Some(location) = next_response.headers().get(LOCATION) else {
                    response = Some(next_response);
                    break;
                };
                let location = location
                    .to_str()
                    .map_err(|_| ForwardError::InvalidRedirectLocation)?;
                let next_url = current_url
                    .join(location)
                    .map_err(|_| ForwardError::InvalidRedirectLocation)?;
                let preserve_body = matches!(
                    next_response.status(),
                    http::StatusCode::TEMPORARY_REDIRECT | http::StatusCode::PERMANENT_REDIRECT
                );
                if preserve_body {
                    let body = recorded.lock().expect("body recorder lock").iter().fold(
                        Vec::new(),
                        |mut output, chunk| {
                            output.extend_from_slice(chunk);
                            output
                        },
                    );
                    current_body = Some(reqwest::Body::from(body));
                }
                if !preserve_body && !matches!(current_method, Method::GET | Method::HEAD) {
                    current_method = Method::GET;
                    current_body = None;
                }
                current_url = next_url;
            }
            response.ok_or(ForwardError::TooManyRedirects)?
        }
        _ => {
            build_request(
                &client,
                &method,
                &url,
                request_headers,
                &upstream.auth_token,
                Some(reqwest::Body::wrap_stream(
                    request_body.into_data_stream().then({
                        let idle = Duration::from_secs(connection.idle_timeout_secs);
                        move |chunk| async move {
                            match tokio::time::timeout(idle, async { chunk }).await {
                                Ok(chunk) => {
                                    chunk.map_err(|error| std::io::Error::other(error.to_string()))
                                }
                                Err(_) => Err(std::io::Error::new(
                                    std::io::ErrorKind::TimedOut,
                                    "upstream request body idle timeout",
                                )),
                            }
                        }
                    }),
                )),
            )
            .send()
            .await?
        }
    };

    let mut builder = Response::builder().status(response.status());
    for (name, value) in response.headers() {
        if !is_hop_by_hop(name) {
            builder = builder.header(name, value);
        }
    }
    let stream = response.bytes_stream().then({
        let idle = Duration::from_secs(connection.idle_timeout_secs);
        move |chunk| async move {
            match tokio::time::timeout(idle, async { chunk }).await {
                Ok(chunk) => chunk.map_err(std::io::Error::other),
                Err(_) => Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "upstream response idle timeout",
                )),
            }
        }
    });
    Ok(builder.body(Body::from_stream(stream))?)
}

fn build_request(
    client: &Client,
    method: &Method,
    url: &Url,
    request_headers: &HeaderMap,
    auth_token: &str,
    body: Option<reqwest::Body>,
) -> reqwest::RequestBuilder {
    let mut request = client
        .request(method.clone(), url.clone())
        .header(AUTHORIZATION, format!("Bearer {auth_token}"));
    for (name, value) in request_headers {
        if !is_hop_by_hop(name) && name != AUTHORIZATION {
            request = request.header(name, value);
        }
    }
    match body {
        Some(body) => request.body(body),
        None => request,
    }
}

fn is_hop_by_hop(name: &HeaderName) -> bool {
    matches!(
        name,
        &CONNECTION | &CONTENT_LENGTH | &HOST | &TE | &TRAILER | &TRANSFER_ENCODING | &UPGRADE
    )
}

pub fn join_upstream_url(base_url: &str, request_uri: &Uri) -> Result<Url, url::ParseError> {
    let mut base = Url::parse(base_url)?;
    let base_path = base.path().trim_end_matches('/');
    let request_path = request_uri.path().trim_start_matches('/');
    let path = if request_path.is_empty() {
        if base_path.is_empty() {
            "/".to_owned()
        } else {
            format!("{base_path}/")
        }
    } else if base_path.is_empty() {
        format!("/{request_path}")
    } else {
        format!("{base_path}/{request_path}")
    };
    base.set_path(&path);
    base.set_query(request_uri.query());
    Ok(base)
}

#[cfg(test)]
mod tests {
    use super::join_upstream_url;
    use axum::http::Uri;

    #[test]
    fn joins_base_path_and_request_query() {
        let uri: Uri = "/responses?stream=true".parse().unwrap();
        let url = join_upstream_url("https://api.example.test/v1", &uri).unwrap();
        assert_eq!(
            url.as_str(),
            "https://api.example.test/v1/responses?stream=true"
        );
    }

    #[test]
    fn avoids_duplicate_slashes_when_base_has_trailing_slash() {
        let uri: Uri = "/responses".parse().unwrap();
        let url = join_upstream_url("https://api.example.test/v1/", &uri).unwrap();
        assert_eq!(url.as_str(), "https://api.example.test/v1/responses");
    }
}
