use clap::Parser;
use html_escape::encode_text_to_string;
use std::env;
use std::io::{Read, stdin};
use std::time::Duration;
use ureq::Agent;

#[cfg(feature = "proxy")]
mod proxy {
    use std::borrow::Cow;
    use std::env;
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;

    fn detect_proxy_in<'a>(file: impl AsRef<Path>) -> Option<Cow<'a, str>> {
        if let Ok(mut f) = File::open(file) {
            let mut proxy = String::with_capacity(25);
            f.read_to_string(&mut proxy).unwrap();
            proxy.truncate(proxy.trim_end().len());
            if proxy.is_empty() {
                return Some(Cow::Borrowed("http://127.0.0.1:10808"));
            }
            proxy.shrink_to_fit();
            return Some(Cow::Owned(proxy));
        }
        None
    }

    pub fn detect_proxy() -> Option<impl AsRef<str>> {
        if let Ok(proxy) = env::var("HTTP_PROXY") {
            return Some(Cow::Owned(proxy));
        }
        env::var_os("HOME")
            .and_then(|h| detect_proxy_in(Path::new(&h).join(".krr_proxy")))
            .or_else(|| detect_proxy_in("/etc/krr_proxy"))
    }
}

const URL: &str = concat!(
    "https://api.telegram.org/bot",
    env!("BOT_TOKEN"),
    "/sendMessage"
);
const CHAT_ID: &str = env!("CHAT_ID");
const CHAT_ID_SILENT: &str = env!("CHAT_ID_SILENT");

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    body: Option<String>,
    #[arg(short)]
    title: Option<String>,
    #[arg(short)]
    silent: bool,
}

fn main() {
    #[cfg(feature = "log")]
    env_logger::init();

    let args = Args::parse();
    let buf = if let Some(s) = args.body {
        s
    } else {
        let mut r = String::new();
        stdin().lock().read_to_string(&mut r).unwrap();
        r
    };
    let n = buf.len() + 20;
    let mut msg = if let Some(mut r) = args.title {
        r.truncate(r.trim_end().len());
        r.reserve(n);
        r.push('\n');
        r
    } else {
        String::with_capacity(n)
    };
    msg.push_str("<pre>");
    encode_text_to_string(buf.trim_end(), &mut msg);
    drop(buf);
    msg.push_str("</pre>");

    let http = Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(30)))
        .ip_family(ureq::config::IpFamily::Ipv4Only);

    #[cfg(feature = "proxy")]
    let http = if let Some(proxy) = proxy::detect_proxy() {
        let proxy = proxy.as_ref();
        eprintln!("using proxy {}", proxy);
        http.proxy(Some(ureq::Proxy::new(proxy).unwrap()))
    } else {
        http
    };

    let http: Agent = http.build().into();

    let result = if args.silent {
        http.post(URL).send_form([
            ("chat_id", CHAT_ID_SILENT),
            ("text", &msg),
            ("parse_mode", "HTML"),
            ("disable_notification", "true"),
        ])
    } else {
        http.post(URL)
            .send_form([("chat_id", CHAT_ID), ("text", &msg), ("parse_mode", "HTML")])
    };

    match result {
        Ok(mut resp) => {
            let st = resp.status();
            if !st.is_success() {
                eprintln!("status: {} {:?}", st, st.canonical_reason());
                match resp.body_mut().read_to_string() {
                    Ok(s) => eprintln!("{}", s),
                    Err(e) => eprintln!("read: {:?}", e),
                }
            }
        }
        Err(e) => {
            eprintln!("error: {}", e);
        }
    }
}
