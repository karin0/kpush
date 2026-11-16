# kpush

kpush sends a message to a Telegram chat from the command line.

```sh
kpush [-t TITLE] [-s] [-c CHAT_ID] [BODY]
```

kpush reads the body from stdin when `BODY` is omitted and sends it inside `<pre>`, escaped and truncated to Telegram's 4096-character limit. The title is sent above the body as HTML, so it may carry markup such as `<b>`. `-s` sends to the silent chat without a notification, and `-c` sends to the given chat in place of the built-in one. kpush exits 1 when Telegram rejects the message or the request fails, and gives up on a request after 30 seconds.

## Building

The bot token and chat IDs are compiled into the binary, so the build needs them in the environment:

| Variable | Meaning |
|---|---|
| `BOT_TOKEN` | Bot token |
| `CHAT_ID` | Default chat |
| `CHAT_ID_SILENT` | Chat used by `-s` |

The `proxy` feature sends through `$HTTP_PROXY`, or through the URL in `~/.krr_proxy` or `/etc/krr_proxy` (an empty file means `http://127.0.0.1:10808`). The `log` feature enables `env_logger`.

## Installing

The binary embeds the token. Install it with mode 0750, owned by root and by the group allowed to send, so that only root and that group can run it or read the token. Mode 0711 hides the file but not the token, since a binary without setuid honors `LD_PRELOAD`, and a preloaded library can read the process memory.
