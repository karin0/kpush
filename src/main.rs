use clap::Parser;
use html_escape::encode_text_to_string;
use std::io::{Read, stdin};
use std::process::ExitCode;
use std::time::Duration;
use ureq::Agent;

#[cfg(feature = "proxy")]
mod proxy {
    use std::borrow::Cow;
    use std::env;
    use std::fs;
    use std::io::ErrorKind;
    use std::path::Path;

    fn detect_proxy_in(file: impl AsRef<Path>) -> Option<Cow<'static, str>> {
        let proxy = match fs::read_to_string(file) {
            Err(e) if e.kind() == ErrorKind::NotFound => return None,
            r => r.unwrap(),
        };
        let proxy = proxy.trim_end();
        Some(if proxy.is_empty() {
            Cow::Borrowed("http://127.0.0.1:10808")
        } else {
            Cow::Owned(proxy.to_owned())
        })
    }

    pub fn detect_proxy() -> Option<Cow<'static, str>> {
        if let Ok(proxy) = env::var("HTTP_PROXY") {
            return Some(Cow::Owned(proxy));
        }
        env::var_os("HOME")
            .and_then(|h| detect_proxy_in(Path::new(&h).join(".krr_proxy")))
            .or_else(|| detect_proxy_in("/etc/krr_proxy"))
    }
}

// Embedded on purpose. The bot is restricted to a single account, so a leaked
// token buys spam and 48h message deletion until it is revoked. Moving it to a
// runtime config file does not change that.
const URL: &str = concat!(
    "https://api.telegram.org/bot",
    env!("BOT_TOKEN"),
    "/sendMessage"
);
const CHAT_ID: &str = env!("CHAT_ID");
const CHAT_ID_SILENT: &str = env!("CHAT_ID_SILENT");

// Telegram counts the text in UTF-16 units after entity parsing, so the markup
// and the escape sequences stay out of the budget.
const TEXT_LIMIT: usize = 4096;
const TRUNCATED: &str = "\n[truncated]";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    body: Option<String>,
    #[arg(short)]
    title: Option<String>,
    #[arg(short)]
    silent: bool,
    /// Chat to send to, in place of the built-in one
    #[arg(short, allow_hyphen_values = true)]
    chat: Option<String>,
}

/// Longest prefix of `body` that leaves `TRUNCATED` room within `limit` units.
fn clamp(body: &str, limit: usize) -> (&str, &str) {
    let mut units = 0;
    let mut fits = 0;
    for (i, c) in body.char_indices() {
        units += c.len_utf16();
        if units > limit {
            return (&body[..fits], TRUNCATED);
        }
        if units + TRUNCATED.len() <= limit {
            fits = i + c.len_utf8();
        }
    }
    (body, "")
}

/// Message text for `parse_mode=HTML`, clamped to what Telegram accepts.
fn compose(title: &str, body: &str) -> String {
    let title = title.trim_end();
    let used = if title.is_empty() {
        0
    } else {
        title.encode_utf16().count() + 1
    };
    let (body, truncated) = clamp(body.trim_end(), TEXT_LIMIT.saturating_sub(used));

    let mut msg = String::with_capacity(body.len() + title.len() + 32);
    encode_text_to_string(title, &mut msg);
    if !title.is_empty() {
        msg.push('\n');
    }
    msg.push_str("<pre>");
    encode_text_to_string(body, &mut msg);
    msg.push_str(truncated);
    msg.push_str("</pre>");
    msg
}

fn main() -> ExitCode {
    #[cfg(feature = "log")]
    env_logger::init();

    let args = Args::parse();
    let buf = match args.body {
        Some(s) => s,
        None => {
            let mut r = Vec::new();
            stdin().lock().read_to_end(&mut r).unwrap();
            String::from_utf8_lossy(&r).into_owned()
        }
    };

    let msg = compose(args.title.as_deref().unwrap_or_default(), &buf);

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

    let (chat_id, silent) = if args.silent {
        (CHAT_ID_SILENT, "true")
    } else {
        (CHAT_ID, "false")
    };
    let result = http.post(URL).send_form([
        ("chat_id", args.chat.as_deref().unwrap_or(chat_id)),
        ("text", &msg),
        ("parse_mode", "HTML"),
        ("disable_notification", silent),
    ]);

    match result {
        Ok(mut resp) => {
            let st = resp.status();
            if st.is_success() {
                return ExitCode::SUCCESS;
            }
            eprintln!("status: {} {:?}", st, st.canonical_reason());
            match resp.body_mut().read_to_string() {
                Ok(s) => eprintln!("{s}"),
                Err(e) => eprintln!("read: {e:?}"),
            }
        }
        Err(e) => eprintln!("error: {e}"),
    }
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::{TRUNCATED, clamp, compose};

    fn units(s: &str) -> usize {
        s.encode_utf16().count()
    }

    #[test]
    fn escapes_the_title() {
        assert_eq!(
            compose("build & test", "ok"),
            "build &amp; test\n<pre>ok</pre>"
        );
    }

    #[test]
    fn omits_the_separator_without_a_title() {
        assert_eq!(compose("", "ok"), "<pre>ok</pre>");
    }

    #[test]
    fn keeps_a_body_that_fits() {
        assert_eq!(clamp("hello", 5), ("hello", ""));
    }

    #[test]
    fn cuts_on_a_char_boundary() {
        let body = "日".repeat(100);
        let (kept, marker) = clamp(&body, 20);
        assert_eq!(marker, TRUNCATED);
        assert_eq!(units(kept) + units(marker), 20);
        assert!(kept.ends_with('日'));
    }

    #[test]
    fn counts_surrogate_pairs_as_two() {
        let body = "🦀".repeat(10);
        let (kept, marker) = clamp(&body, 16);
        assert_eq!(kept, "🦀🦀");
        assert_eq!(units(kept) + units(marker), 16);
    }
}
