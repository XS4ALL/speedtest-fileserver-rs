use std::net::{AddrParseError, IpAddr, SocketAddr};
use std::panic;
use std::path::PathBuf;

use futures::stream::{FuturesUnordered, StreamExt};
use serde::Deserialize;
use structopt::StructOpt;
use tokio::task;

mod lehmer64;
//mod remoteip;
mod server;
mod template;

const CONFIG_FILE: &'static str = "/etc/speedtest-fileserver.cfg";

// Configuration file settings.
#[derive(Clone, Deserialize, Debug)]
pub struct Config {
    // Settings for http.
    http: Option<Http>,

    // Settings for https.
    https: Option<Https>,

    // Settings for the index file.
    pub index: Index,

    // Use X-Forwarded-For/X-Real-Ip/Forwarded headers (unused for now).
    #[serde(rename = "xff-headers", default)]
    pub xff: bool,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Index {
    pub file: Option<PathBuf>,
    pub sizes: Vec<String>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Http {
    // [addr:]port to listen on (4000)
    pub listen: Vec<String>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Https {
    // [addr:]port to listen on (4000)
    pub listen: Vec<String>,

    // TLS certificate chain file
    pub chain: String,

    // TLS certificate key file
    pub key: String,
}

// Add a sockaddr to the list of listeners.
// If "addr" specifies just a port, add two sockaddrs: one for IPv4, one for IPv6.
fn add_listener(addr: &str, listen: &mut Vec<(SocketAddr, String)>) -> Result<(), AddrParseError> {
    if let Ok(port) = addr.parse::<u16>() {
        listen.push((
            SocketAddr::new(IpAddr::V4(0u32.into()), port),
            format!("*:{}", port),
        ));
        listen.push((
            SocketAddr::new(IpAddr::V6(0u128.into()), port),
            format!("[::]:{}", port),
        ));
        return Ok(());
    }
    // "*:port" is IPv4 wildcard. "[::]:port" for IPv6.
    if addr.starts_with("*") {
        let addr2 = addr.replacen("*", "0.0.0.0", 1);
        listen.push((addr2.parse::<SocketAddr>()?, addr.to_string()));
    } else {
        listen.push((addr.parse::<SocketAddr>()?, addr.to_string()));
    }
    Ok(())
}

macro_rules! die {
    (log => $($tt:tt)*) => ({
        log::error!($($tt)*);
        std::process::exit(1);
    });
    (std => $($tt:tt)*) => ({
        eprintln!($($tt)*);
        std::process::exit(1);
    });
}

// add
fn resolve_path(dir: &str, file: &str) -> PathBuf {
    let mut p = file.parse::<PathBuf>().unwrap();
    if p.is_relative() && p.metadata().is_err() {
        let mut d = dir.parse::<PathBuf>().unwrap();
        d.push(&p);
        p = d;
    }
    if let Err(e) = p.metadata() {
        die!(std => "{:?}: {}", p, e);
    }
    p
}

#[derive(Debug, StructOpt)]
#[structopt(name = "speedtest-fileserver", about = "Speedtest file server.")]
struct Opts {
    /// location of config file.
    #[structopt(short, long)]
    config: Option<String>,
}

async fn async_main() {
    // Parse options.
    let opts = Opts::from_args();

    // Read config file.
    let config_file = opts.config.unwrap_or(CONFIG_FILE.to_string());
    let config: Config = curlyconf::from_file(&config_file)
        .map_err(|e| die!(std => "config: {}", e))
        .unwrap();

    if config.http.is_none() && config.https.is_none() {
        die!(std => "{}: at least one of 'http' or 'https' must be enabled", config_file);
    }

    // Parse the http config section.
    let mut http_listen = Vec::new();
    if let Some(http) = config.http.as_ref() {
        for l in &http.listen {
            if let Err(e) = add_listener(l, &mut http_listen) {
                die!(std => "{}: {}", l, e);
            }
        }
    }

    // Parse the https config section.
    let mut https_listen = Vec::new();
    let https = config.https.as_ref().map(|https| {
        for l in &https.listen {
            if let Err(e) = add_listener(l, &mut https_listen) {
                die!(std => "{}: {}", l, e);
            }
        }
        let https_key = resolve_path("/etc/ssl/private", &https.key);
        let https_chain = resolve_path("/etc/ssl/certs", &https.chain);
        (https_key, https_chain)
    });

    // build routes.
    let server = server::FileServer::new(&config);
    let routes = server.routes();

    // Run all servers.
    let mut handles = Vec::new();
    for (addr, name) in &http_listen {
        match warp::serve(routes.clone()).try_bind_ephemeral(addr.clone()) {
            Ok((_, srv)) => {
                log::info!("Listening on {}", name);
                handles.push(task::spawn(srv));
            }
            Err(e) => die!(log => "{}: {}", name, e),
        }
    }

    if let Some((https_key, https_chain)) = https {
        for (addr, name) in &https_listen {
            // why no try_bind_ephemeral in the TlsServer?
            let srv = warp::serve(routes.clone());
            let srv = srv
                .tls()
                .key_path(&https_key)
                .cert_path(&https_chain)
                .bind(addr.clone());
            log::info!("Listening on {}", name);
            handles.push(task::spawn(srv));
        }
    }

    // The tasks should never return, only on error. So _if_ one
    // returns, abort the entire process.
    let mut task_waiter = FuturesUnordered::new();
    for handle in handles.drain(..) {
        task_waiter.push(handle);
    }
    if let Some(Err(err)) = task_waiter.next().await {
        if let Ok(cause) = err.try_into_panic() {
            if let Some(err) = cause.downcast_ref::<String>() {
                die!(log => "fatal: {}", err);
            }
        }
    }
    die!(log => "server exited unexpectedly");
}

fn main() {
    let env = env_logger::Env::default().default_filter_or("info");
    env_logger::init_from_env(env);

    let mut rt = tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .on_thread_start(|| {
            let hook = panic::take_hook();
            panic::set_hook(Box::new(move |info| {
                match info.payload().downcast_ref::<String>() {
                    Some(msg) if msg.contains("error binding to") => {}
                    _ => hook(info),
                }
            }));
        })
        .build()
        .unwrap();
    rt.block_on(async_main());
}
