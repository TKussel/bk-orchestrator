use std::path::PathBuf;

use http::{Uri, HeaderValue};
use reqwest::{Proxy, Client, Certificate};
use tracing::{debug, info, warn};
use clap::Parser;

use crate::{error::ExecutorError, beam::AppId};

const CLAP_FOOTER: &str = "For proxy support, environment variables HTTP_PROXY, HTTPS_PROXY, ALL_PROXY and NO_PROXY (and their lower-case variants) are supported. Usually, you want to set HTTP_PROXY *and* HTTPS_PROXY or set ALL_PROXY if both values are the same.\n\nFor updates and detailed usage instructions, visit https://github.com/tkussel/bk-orchestrator";

#[derive(Parser,Debug)]
#[clap(name("🎼 BK-Orchestrator"), version, arg_required_else_help(true), after_help(CLAP_FOOTER))]
struct CliArgs {
    /// The beam proxy's base URL, e.g. https://proxy1.beam.samply.de
    #[clap(long, short='p', env, value_parser)]
    beam_proxy_url: Uri,

    /// This application's beam AppId, e.g. focus.proxy1.broker.samply.de
    #[clap(long, short='i', env, value_parser)]
    beam_app_id: String,

    /// This applications beam API key
    #[clap(long, short='k', env, value_parser)]
    beam_api_key: String,

    /// Outgoing HTTP proxy: Directory with CA certificates to trust for TLS connections (e.g. /etc/samply/cacerts/)
    #[clap(long, short='c', env, value_parser)]
    tls_ca_certificates_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct BeamConfig {
    pub app_id: AppId,
    pub(crate) app_key: String,
    pub beam_proxy_url: Uri,
    pub(crate) client: reqwest::Client,
}

impl BeamConfig {
    pub fn load() -> Result<Self,ExecutorError> {
        let cli_args = CliArgs::parse(); 
        info!("Successfully read config and API keys from CLI and secrets files.");
        let tls_ca_certificates_dir = cli_args.tls_ca_certificates_dir;
        let tls_ca_certificates = load_certificates_from_dir(tls_ca_certificates_dir.clone())
            .map_err(|e| ExecutorError::ConfigurationError(format!("Unable to read from TLS CA directory: {}", e)))?;
        debug!("Post loading");
        let client = prepare_reqwest_client(&tls_ca_certificates)?;
        let config = BeamConfig {
            beam_proxy_url: cli_args.beam_proxy_url,
            app_id: AppId::new(cli_args.beam_app_id)?,
            app_key: cli_args.beam_api_key,
            client
        };
        Ok(config)
    }
}


pub fn load_certificates_from_dir(ca_dir: Option<PathBuf>) -> Result<Vec<Certificate>, std::io::Error> {
    let mut result = Vec::new();
    if let Some(ca_dir) = ca_dir {
        for file in ca_dir.read_dir()? { //.map_err(|e| SamplyBeamError::ConfigurationFailed(format!("Unable to read from TLS CA directory {}: {}", ca_dir.to_string_lossy(), e)))
            let path = file?.path();
            let content = std::fs::read(&path)?;
            let cert = Certificate::from_pem(&content);
            if let Err(e) = cert {
                warn!("Unable to read certificate from file {}: {}", path.to_string_lossy(), e);
                continue;
            }
            result.push(cert.unwrap());
        }
    }
    Ok(result)
}

pub fn prepare_reqwest_client(certs: &Vec<Certificate>) -> Result<reqwest::Client, ExecutorError>{
    let version = String::from(env!("CARGO_PKG_VERSION"));
    let user_agent = format!("BK-Orchestrator/{}", version);
    let mut client = reqwest::Client::builder()
        .tcp_nodelay(true)
        .user_agent(user_agent);
    for cert in certs {
        client = client.add_root_certificate(cert.to_owned());
    }
    let mut proxies: Vec<Proxy> = Vec::new();
    let no_proxy = reqwest::NoProxy::from_env();
    for var in ["http_proxy", "https_proxy", "all_proxy", "no_proxy"] {
        for (k,v) in std::env::vars().filter(|(k,_)| k.to_lowercase() == var) {
            std::env::set_var(k.to_uppercase(), v.clone());
            match k.as_str() {
                "http_proxy" => proxies.push(Proxy::http(v).map_err(|e|ExecutorError::InvalidProxyConfig(e))?.no_proxy(no_proxy.clone())),
                "https_proxy" => proxies.push(Proxy::https(v).map_err(|e|ExecutorError::InvalidProxyConfig(e))?.no_proxy(no_proxy.clone())),
                "all_proxy" => proxies.push(Proxy::all(v).map_err(|e|ExecutorError::InvalidProxyConfig(e))?.no_proxy(no_proxy.clone())),
                _ => ()
            };
        }
    }
    client.build().map_err(|e|ExecutorError::ConfigurationError(format!("Cannot create http client: {}",e)))

}
