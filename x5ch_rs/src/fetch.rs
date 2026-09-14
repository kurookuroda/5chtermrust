//! HTTP取得層。Python版 fetch.py の Fetcher クラスに対応。
//!
//! Pythonの `class Fetcher: def __init__(self, user_agent): ...` と同じ発想で、
//! 「設定(user_agent)を持つオブジェクト」として構造体+implブロックにまとめる。

use crate::errors::FetchError;

const REQUEST_TIMEOUT_SECS: u64 = 30;
const MAX_REDIRECTS: u32 = 5;

pub struct Fetcher {
    user_agent: String,
}

impl Fetcher {
    pub fn new(user_agent: impl Into<String>) -> Self {
        Self {
            user_agent: user_agent.into(),
        }
    }

    /// URLを取得し、本文(生バイト列)と最終URL(リダイレクト後)を返す。
    pub async fn fetch(&self, url: &str) -> Result<(Vec<u8>, String), FetchError> {
        let user_agent = self.user_agent.clone();
        let url = url.to_string();

        tokio::task::spawn_blocking(move || {
            let tls_connector =
                native_tls::TlsConnector::new().expect("TLSコネクタの初期化に失敗");
            let agent = ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
                .redirects(MAX_REDIRECTS)
                .user_agent(&user_agent)
                .tls_connector(std::sync::Arc::new(tls_connector))
                .build();

            let resp = agent.get(&url).call().map_err(|e| match e {
                ureq::Error::Transport(t)
                    if t.to_string().contains("too many redirects") =>
                {
                    FetchError::TooManyRedirects(MAX_REDIRECTS as usize)
                }
                ureq::Error::Status(code, _) => FetchError::HttpStatus(code),
                ureq::Error::Transport(t) => {
                    if t.kind() == ureq::ErrorKind::Io {
                        FetchError::Network(t.to_string())
                    } else {
                        FetchError::Timeout(t.to_string())
                    }
                }
            })?;

            let final_url = resp.get_url().to_string();

            let mut body = Vec::new();
            resp.into_reader()
                .read_to_end(&mut body)
                .map_err(|e| FetchError::Network(e.to_string()))?;

            Ok((body, final_url))
        })
        .await
        .expect("blockingタスクがpanicした")
    }
}
