use anyhow::{anyhow, Result};
use log::debug;
use reqwest::blocking::multipart::{Form, Part};
use reqwest::blocking::Client;
use reqwest::header::AUTHORIZATION;
use serde::Deserialize;
use std::io::Cursor;
use std::path::PathBuf;

use crate::managers::model::EngineType;
use crate::settings::AppSettings;

const CODEX_ENDPOINT: &str = "https://chatgpt.com/backend-api/transcribe";
const CODEX_ORIGINATOR: &str = "codex_desktop";
const CODEX_USER_AGENT: &str = "Codex Desktop/26.611.62324";
const GROQ_ENDPOINT: &str = "https://api.groq.com/openai/v1/audio/transcriptions";
const GROQ_MODEL: &str = "whisper-large-v3-turbo";

/// A remote, online-only transcription backend. These providers have no local
/// model to load — they POST the recorded audio to an HTTP API and return text.
#[derive(Clone, Debug)]
pub enum RemoteProvider {
    /// OpenAI Codex dictation endpoint, authenticated via the local Codex CLI.
    Codex,
    /// Groq OpenAI-compatible audio transcription API.
    Groq,
}

impl RemoteProvider {
    pub fn from_engine(engine: &EngineType) -> Option<Self> {
        match engine {
            EngineType::CodexDictation => Some(RemoteProvider::Codex),
            EngineType::Groq => Some(RemoteProvider::Groq),
            _ => None,
        }
    }
}

#[derive(Deserialize)]
struct TranscribeResponse {
    text: String,
}

#[derive(Deserialize)]
struct CodexTokens {
    access_token: Option<String>,
    account_id: Option<String>,
}

#[derive(Deserialize)]
struct CodexAuth {
    tokens: Option<CodexTokens>,
}

/// Transcribe audio with a remote provider. `transcribe()` runs inside Tauri's
/// async runtime, so `reqwest::blocking` is dispatched on a dedicated thread to
/// avoid a nested-runtime panic.
pub fn transcribe(
    provider: &RemoteProvider,
    samples: &[f32],
    language: &str,
    settings: &AppSettings,
) -> Result<String> {
    let wav = encode_wav(samples)?;
    let lang = map_language(language);
    let provider = provider.clone();
    let groq_api_key = settings.groq_api_key.clone();

    std::thread::spawn(move || -> Result<String> {
        match provider {
            RemoteProvider::Codex => codex_request(wav, lang),
            RemoteProvider::Groq => groq_request(wav, lang, &groq_api_key),
        }
    })
    .join()
    .map_err(|_| anyhow!("Remote transcription thread panicked"))?
}

fn encode_wav(samples: &[f32]) -> Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)?;
        for sample in samples {
            let sample_i16 = (sample * i16::MAX as f32) as i16;
            writer.write_sample(sample_i16)?;
        }
        writer.finalize()?;
    }
    Ok(cursor.into_inner())
}

/// Map Handy's language codes to what the remote APIs expect. `auto` is dropped
/// (let the server auto-detect); the Chinese script variants collapse to `zh`.
fn map_language(language: &str) -> Option<String> {
    match language {
        "auto" | "" => None,
        "zh-Hans" | "zh-Hant" => Some("zh".to_string()),
        other => Some(other.to_string()),
    }
}

fn wav_part(wav: Vec<u8>) -> Result<Part> {
    Part::bytes(wav)
        .file_name("handy.wav")
        .mime_str("audio/wav")
        .map_err(|e| anyhow!("Failed to build audio part: {}", e))
}

fn codex_auth_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| anyhow!("Could not determine home directory"))?;
    Ok(PathBuf::from(home).join(".codex").join("auth.json"))
}

fn codex_credentials() -> Result<(String, Option<String>)> {
    let path = codex_auth_path()?;
    let data = std::fs::read_to_string(&path)
        .map_err(|e| anyhow!("Failed to read {}: {}", path.display(), e))?;
    let auth: CodexAuth = serde_json::from_str(&data)
        .map_err(|e| anyhow!("Failed to parse {}: {}", path.display(), e))?;
    let tokens = auth
        .tokens
        .ok_or_else(|| anyhow!("No `tokens` object in {}", path.display()))?;
    let access_token = tokens
        .access_token
        .filter(|t| !t.is_empty())
        .ok_or_else(|| anyhow!("No `tokens.access_token` in {}", path.display()))?;
    Ok((access_token, tokens.account_id.filter(|a| !a.is_empty())))
}

fn codex_request(wav: Vec<u8>, lang: Option<String>) -> Result<String> {
    let (access_token, account_id) = codex_credentials()?;

    let mut form = Form::new().part("file", wav_part(wav)?);
    if let Some(language) = lang {
        form = form.text("language", language);
    }

    let client = Client::new();
    let mut request = client
        .post(CODEX_ENDPOINT)
        .header("originator", CODEX_ORIGINATOR)
        .header(reqwest::header::USER_AGENT, CODEX_USER_AGENT)
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .multipart(form);
    if let Some(account_id) = account_id {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    debug!("Sending audio to Codex dictation endpoint");
    let response = request
        .send()
        .map_err(|e| anyhow!("Codex transcribe request failed: {}", e))?;
    parse_response(response, "Codex")
}

fn groq_request(wav: Vec<u8>, lang: Option<String>, api_key: &str) -> Result<String> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(anyhow!(
            "Groq API key is not set. Add it in Settings → Models."
        ));
    }

    let mut form = Form::new()
        .part("file", wav_part(wav)?)
        .text("model", GROQ_MODEL)
        .text("response_format", "json");
    if let Some(language) = lang {
        form = form.text("language", language);
    }

    let client = Client::new();
    debug!("Sending audio to Groq transcription endpoint");
    let response = client
        .post(GROQ_ENDPOINT)
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .map_err(|e| anyhow!("Groq transcribe request failed: {}", e))?;
    parse_response(response, "Groq")
}

fn parse_response(response: reqwest::blocking::Response, provider: &str) -> Result<String> {
    let status = response.status();
    let body = response
        .text()
        .map_err(|e| anyhow!("Failed to read {} response: {}", provider, e))?;

    if !status.is_success() {
        return Err(anyhow!(
            "{} transcribe failed ({}): {}",
            provider,
            status,
            body
        ));
    }

    let parsed: TranscribeResponse = serde_json::from_str(&body)
        .map_err(|e| anyhow!("Unexpected {} response: {} (body: {})", provider, e, body))?;
    Ok(parsed.text)
}
