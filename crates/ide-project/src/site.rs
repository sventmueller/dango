//! A small GET-only HTTP server for the live preview of static pages (E131): it serves one folder at its root on
//! 127.0.0.1 and a random port, only to requests that name that host and carry the cookie the app set in its browser
//! tab, and never hidden or ignored files or anything a symlink leads to outside the folder.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The name of the cookie a request to the server on `port` must carry: 127.0.0.1 shares its cookies across ports, so
/// each server takes only its own.
pub fn cookie_name(port: u16) -> String {
    format!("site_preview_{port}")
}

const LONGEST_HEAD: usize = 16 * 1024;
/// How long a connection may take to send its request.
const HEAD_TIME: Duration = Duration::from_secs(2);

/// A running server for one folder; it stops when dropped.
pub struct Site {
    root: PathBuf,
    port: u16,
    secret: String,
    stop: Arc<AtomicBool>,
}

impl Site {
    /// Serves `root` on a free port of 127.0.0.1 to requests that carry `secret` as the cookie [`cookie_name`] names.
    pub fn start(root: &Path, secret: String) -> io::Result<Site> {
        let root = root.canonicalize()?;
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let rules = Arc::new(Rules {
            root: root.clone(),
            hosts: [format!("127.0.0.1:{port}"), format!("localhost:{port}")],
            cookie: cookie_name(port),
            secret: secret.clone(),
            ignore: ignore_matcher(&root),
        });
        let stopped = stop.clone();
        std::thread::Builder::new()
            .name("preview-site".into())
            .spawn(move || accept(&listener, &stopped, &rules))?;
        Ok(Site {
            root,
            port,
            secret,
            stop,
        })
    }

    /// The folder it serves.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The port it listens on, which names its cookie.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The cookie's value requests must carry.
    pub fn secret(&self) -> &str {
        &self.secret
    }

    /// Where it serves its origin, such as `http://127.0.0.1:52011`.
    pub fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// The address of `file` below the folder, or `None` for a file outside it.
    pub fn url_of(&self, file: &Path) -> Option<String> {
        let file = file.canonicalize().ok()?;
        let relative = file.strip_prefix(&self.root).ok()?;
        let path: Vec<String> = relative
            .components()
            .map(|c| percent_encode(&c.as_os_str().to_string_lossy()))
            .collect();
        Some(format!("{}/{}", self.origin(), path.join("/")))
    }

    /// The file below the folder that `url` of this server names.
    pub fn file_of(&self, url: &str) -> Option<PathBuf> {
        let rest = url.strip_prefix(&self.origin())?;
        let path = rest.split(['?', '#']).next().unwrap_or_default();
        Some(self.root.join(percent_decode(path.trim_start_matches('/'))?))
    }
}

impl Drop for Site {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

struct Rules {
    root: PathBuf,
    hosts: [String; 2],
    cookie: String,
    secret: String,
    ignore: Option<ignore::gitignore::Gitignore>,
}

fn ignore_matcher(root: &Path) -> Option<ignore::gitignore::Gitignore> {
    let mut builder = ignore::gitignore::GitignoreBuilder::new(root);
    let _ = builder.add(root.join(".gitignore"));
    let _ = builder.add(root.join(".git/info/exclude"));
    builder.build().ok()
}

fn accept(listener: &TcpListener, stop: &AtomicBool, rules: &Arc<Rules>) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => serve(stream, rules),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

/// What a request asked for, as far as the server cares.
#[derive(Debug, Default, PartialEq, Eq)]
struct Request {
    method: String,
    target: String,
    host: Option<String>,
    cookies: Vec<(String, String)>,
}

fn read_request(reader: &mut impl BufRead) -> io::Result<Request> {
    let mut request = Request::default();
    let mut read = 0;
    let mut first = true;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        read += n;
        if n == 0 || read > LONGEST_HEAD {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "an incomplete or overlong request",
            ));
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if first {
            let mut parts = line.split(' ');
            request.method = parts.next().unwrap_or_default().to_string();
            request.target = parts.next().unwrap_or_default().to_string();
            first = false;
            continue;
        }
        if line.is_empty() {
            return Ok(request);
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("host") {
            request.host = Some(value.to_string());
        } else if name.eq_ignore_ascii_case("cookie") {
            request.cookies.extend(value.split(';').filter_map(|pair| {
                let (name, value) = pair.trim().split_once('=')?;
                Some((name.to_string(), value.to_string()))
            }));
        }
    }
}

/// What the server answers to `request`: a status and, for a file it serves, the file.
#[derive(Debug, PartialEq, Eq)]
enum Answer {
    File(PathBuf),
    Status(u16, &'static str),
}

fn answer(rules: &Rules, request: &Request) -> Answer {
    if request.method != "GET" && request.method != "HEAD" {
        return Answer::Status(405, "Method Not Allowed");
    }
    if !request.host.as_ref().is_some_and(|host| rules.hosts.contains(host)) {
        return Answer::Status(421, "Misdirected Request");
    }
    if !request
        .cookies
        .iter()
        .any(|(name, value)| *name == rules.cookie && *value == rules.secret)
    {
        return Answer::Status(403, "Forbidden");
    }
    let path = request.target.split(['?', '#']).next().unwrap_or_default();
    let Some(relative) = percent_decode(path.trim_start_matches('/')) else {
        return Answer::Status(400, "Bad Request");
    };
    let relative = PathBuf::from(relative);
    let plain = relative.components().all(|c| match c {
        Component::Normal(name) => !name.to_string_lossy().starts_with('.'),
        _ => false,
    });
    if !plain {
        return Answer::Status(404, "Not Found");
    }
    let mut file = rules.root.join(&relative);
    if file.is_dir() {
        file = file.join("index.html");
    }
    let Ok(real) = file.canonicalize() else {
        return Answer::Status(404, "Not Found");
    };
    if !real.starts_with(&rules.root) || !real.is_file() {
        return Answer::Status(404, "Not Found");
    }
    let ignored = rules
        .ignore
        .as_ref()
        .is_some_and(|ignore| ignore.matched_path_or_any_parents(&real, false).is_ignore());
    if ignored {
        return Answer::Status(404, "Not Found");
    }
    Answer::File(real)
}

fn serve(stream: TcpStream, rules: &Rules) {
    let _ = stream.set_nonblocking(false);
    let Ok(mut write) = stream.try_clone() else { return };
    let mut reader = BufReader::new(Timed {
        stream,
        until: Instant::now() + HEAD_TIME,
    });
    let Ok(request) = read_request(&mut reader) else {
        let _ = respond_status(&mut write, 400, "Bad Request");
        return;
    };
    let _ = match answer(rules, &request) {
        Answer::File(path) => respond_file(&mut write, &path, request.method == "HEAD"),
        Answer::Status(code, text) => respond_status(&mut write, code, text),
    };
    let _ = write.shutdown(Shutdown::Both);
}

/// A connection whose reads fail once `until` passed, so that a client sending its request a byte at a time holds the
/// server's one thread no longer than one that sends nothing.
struct Timed {
    stream: TcpStream,
    until: Instant,
}

impl Read for Timed {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self.until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        self.stream.set_read_timeout(Some(left))?;
        self.stream.read(buf)
    }
}

fn respond_status(out: &mut impl Write, code: u16, text: &str) -> io::Result<()> {
    write!(
        out,
        "HTTP/1.1 {code} {text}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{text}",
        text.len()
    )
}

fn respond_file(out: &mut impl Write, path: &Path, head_only: bool) -> io::Result<()> {
    let file = std::fs::File::open(path)?;
    let length = file.metadata()?.len();
    write!(
        out,
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         Connection: close\r\n\r\n",
        content_type(path),
        length
    )?;
    if !head_only {
        io::copy(&mut file.take(length), out)?;
    }
    Ok(())
}

/// The media type of `path` by its extension; ES modules need a JavaScript type to load.
fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("wasm") => "application/wasm",
        Some("txt" | "md") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder() -> ide_core::testing::Scratch {
        let dir = ide_core::testing::scratch("site");
        std::fs::create_dir_all(dir.join("css")).unwrap();
        std::fs::create_dir_all(dir.join("build")).unwrap();
        std::fs::write(dir.join("index.html"), "<h1>hi</h1>").unwrap();
        std::fs::write(dir.join("css/app.css"), "h1{}").unwrap();
        std::fs::write(dir.join(".env"), "SECRET=1").unwrap();
        std::fs::write(dir.join(".gitignore"), "build/\n").unwrap();
        std::fs::write(dir.join("build/out.js"), "x").unwrap();
        dir
    }

    fn get(site: &Site, path: &str, cookie: Option<&str>, host: Option<String>) -> (u16, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", site.port())).unwrap();
        let host = host.unwrap_or_else(|| format!("127.0.0.1:{}", site.port()));
        let name = cookie_name(site.port());
        let cookie = cookie
            .map(|c| format!("Cookie: other=1; {name}={c}\r\n"))
            .unwrap_or_default();
        write!(stream, "GET {path} HTTP/1.1\r\nHost: {host}\r\n{cookie}\r\n").unwrap();
        let mut text = String::new();
        stream.read_to_string(&mut text).unwrap();
        let code = text[9..12].parse().unwrap();
        let body = text.split("\r\n\r\n").nth(1).unwrap_or_default().to_string();
        (code, body)
    }

    #[test]
    fn it_serves_the_folder_at_its_root_to_requests_with_the_cookie() {
        let dir = folder();
        let site = Site::start(&dir, "s3cret".into()).unwrap();
        assert_eq!(get(&site, "/", Some("s3cret"), None), (200, "<h1>hi</h1>".into()));
        assert_eq!(
            get(&site, "/css/app.css?v=2", Some("s3cret"), None),
            (200, "h1{}".into())
        );
        assert_eq!(get(&site, "/index.html", None, None).0, 403);
        assert_eq!(get(&site, "/index.html", Some("wrong"), None).0, 403);
        assert_eq!(
            get(&site, "/index.html", Some("s3cret"), Some("evil.example:80".into())).0,
            421
        );
    }

    #[test]
    fn it_refuses_hidden_ignored_and_outside_files() {
        let dir = folder();
        let elsewhere = ide_core::testing::scratch("site-outside");
        let outside = elsewhere.join("private.txt");
        std::fs::write(&outside, "private").unwrap();
        std::os::unix::fs::symlink(&outside, dir.join("link.txt")).unwrap();
        let site = Site::start(&dir, "k".into()).unwrap();
        assert_eq!(get(&site, "/.env", Some("k"), None).0, 404);
        assert_eq!(get(&site, "/%2Eenv", Some("k"), None).0, 404);
        assert_eq!(get(&site, "/build/out.js", Some("k"), None).0, 404);
        assert_eq!(get(&site, "/link.txt", Some("k"), None).0, 404);
        assert_eq!(get(&site, "/../etc/passwd", Some("k"), None).0, 404);
        assert_eq!(get(&site, "/.git/config", Some("k"), None).0, 404);
    }

    #[test]
    fn addresses_and_files_map_both_ways() {
        let dir = folder();
        let site = Site::start(&dir, "k".into()).unwrap();
        let url = site.url_of(&dir.join("css/app.css")).unwrap();
        assert_eq!(url, format!("{}/css/app.css", site.origin()));
        assert_eq!(site.file_of(&format!("{url}#top")), Some(dir.join("css/app.css")));
        assert_eq!(site.url_of(Path::new("/etc/hosts")), None);
        assert_eq!(percent_decode("a%20b"), Some("a b".into()));
        assert_eq!(percent_encode("a b.html"), "a%20b.html");
    }

    #[test]
    fn a_head_request_gets_the_length_of_the_file_without_its_bytes() {
        let dir = folder();
        let site = Site::start(&dir, "k".into()).unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", site.port())).unwrap();
        let port = site.port();
        write!(
            stream,
            "HEAD /index.html HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nCookie: site_preview_{port}=k\r\n\r\n"
        )
        .unwrap();
        let mut text = String::new();
        stream.read_to_string(&mut text).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"), "{text}");
        assert!(text.contains("Content-Length: 11\r\n"), "{text}");
        assert!(text.ends_with("\r\n\r\n"), "no body: {text}");
    }

    #[test]
    fn a_request_that_trickles_in_is_cut_off_once_its_time_is_up() {
        let dir = folder();
        let site = Site::start(&dir, "k".into()).unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", site.port())).unwrap();
        let started = Instant::now();
        // Each byte comes long before a timeout per read would fire, so only a limit on the whole request ends it.
        for byte in b"GET / H" {
            if started.elapsed() + Duration::from_millis(300) >= HEAD_TIME {
                break;
            }
            stream.write_all(&[*byte]).unwrap();
            std::thread::sleep(Duration::from_millis(250));
        }
        let mut text = String::new();
        stream.read_to_string(&mut text).unwrap();
        let took = started.elapsed();
        assert!(text.starts_with("HTTP/1.1 400"), "{text}");
        assert!(took < HEAD_TIME + Duration::from_millis(900), "{took:?}");
    }

    #[test]
    fn only_get_and_head_are_answered() {
        let rules = Rules {
            root: PathBuf::from("/"),
            hosts: ["h:1".into(), "l:1".into()],
            cookie: cookie_name(1),
            secret: "k".into(),
            ignore: None,
        };
        let request = Request {
            method: "POST".into(),
            target: "/".into(),
            host: Some("h:1".into()),
            cookies: vec![(cookie_name(1), "k".into())],
        };
        assert_eq!(answer(&rules, &request), Answer::Status(405, "Method Not Allowed"));
    }

    #[test]
    fn each_server_takes_only_the_cookie_named_after_its_port() {
        assert_eq!(cookie_name(52011), "site_preview_52011");
        assert_ne!(cookie_name(52011), cookie_name(52012));
        let dir = folder();
        let site = Site::start(&dir, "k".into()).unwrap();
        let other = site.port().wrapping_add(1);
        let mut stream = TcpStream::connect(("127.0.0.1", site.port())).unwrap();
        let port = site.port();
        write!(
            stream,
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nCookie: site_preview_{other}=k; site_preview=k\r\n\r\n"
        )
        .unwrap();
        let mut text = String::new();
        stream.read_to_string(&mut text).unwrap();
        assert!(text.starts_with("HTTP/1.1 403"), "{text}");
        assert_eq!(get(&site, "/", Some("k"), None).0, 200);
    }
}
