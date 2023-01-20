use clap::Parser;
use html_escape::encode_text_to_string;
use std::borrow::Cow;
use std::env;
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::Path;
use std::time::Duration;
use ureq::AgentBuilder;

const URL: &str = concat!(
    "https://api.telegram.org/bot",
    env!("BOT_TOKEN"),
    "/sendMessage"
);
const CHAT_ID: &str = env!("CHAT_ID");

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    body: Option<String>,
    #[arg(short)]
    title: Option<String>,
}

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

fn detect_proxy() -> Option<impl AsRef<str>> {
    if let Ok(proxy) = env::var("HTTP_PROXY") {
        return Some(Cow::Owned(proxy));
    }
    env::var_os("HOME")
        .and_then(|h| detect_proxy_in(Path::new(&h).join(".krr_proxy")))
        .or_else(|| detect_proxy_in("/etc/krr_proxy"))
}

fn main() {
    let args = Args::parse();
    let buf = if let Some(s) = args.body {
        s
    } else {
        let mut r = String::new();
        io::stdin().lock().read_to_string(&mut r).unwrap();
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

    let timeout = Duration::from_secs(10);
    let http = AgentBuilder::new()
        .timeout_read(timeout)
        .timeout_connect(timeout)
        .timeout_write(timeout);
    let http = if let Some(proxy) = detect_proxy() {
        eprintln!("using proxy {}", proxy.as_ref());
        http.proxy(ureq::Proxy::new(proxy).unwrap())
    } else {
        http
    }
    .build();
    let res =
        http.post(URL)
            .send_form(&[("chat_id", CHAT_ID), ("text", &msg), ("parse_mode", "HTML")]);
    drop(msg);
    if let Err(e) = res {
        match e {
            ureq::Error::Status(code, resp) => {
                eprintln!("status: {} {}", code, resp.status_text());
                match resp.into_string() {
                    Ok(s) => eprintln!("{}", s),
                    Err(e) => eprintln!("read: {:?}", e),
                }
            }
            _ => {
                eprintln!("error: {}", e);
            }
        }
    }
}
