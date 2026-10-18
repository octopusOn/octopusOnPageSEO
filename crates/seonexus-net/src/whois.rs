use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::NetError;

/// Largest accepted WHOIS answer.
const MAX_ANSWER_BYTES: u64 = 1024 * 1024;

/// Asks a WHOIS server (TCP port 43, RFC 3912) and returns its answer as text.
///
/// # Errors
/// [`NetError`] for an invalid server name, connection failures and timeouts.
pub fn whois(server: &str, query: &str, timeout: Duration) -> Result<String, NetError> {
    let error = |message: String| NetError {
        url: format!("whois://{server}"),
        message,
    };
    if server.is_empty() || server.contains(|ch: char| ch.is_whitespace() || ch == '/') {
        return Err(error("invalid WHOIS server".to_owned()));
    }
    if query.contains(['\r', '\n']) {
        return Err(error("invalid WHOIS query".to_owned()));
    }
    let address = (server, 43)
        .to_socket_addrs()
        .map_err(|e| error(e.to_string()))?
        .next()
        .ok_or_else(|| error("the server has no address".to_owned()))?;
    ask(address, query, timeout).map_err(|e| error(e.to_string()))
}

fn ask(address: SocketAddr, query: &str, timeout: Duration) -> std::io::Result<String> {
    let mut stream = TcpStream::connect_timeout(&address, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    stream.write_all(format!("{query}\r\n").as_bytes())?;
    let mut answer = Vec::new();
    stream.take(MAX_ANSWER_BYTES).read_to_end(&mut answer)?;
    // Registries answer in UTF-8 or Latin-1; keep what decodes.
    Ok(String::from_utf8(answer).unwrap_or_else(|bytes| {
        bytes
            .into_bytes()
            .iter()
            .map(|&byte| char::from(byte))
            .collect()
    }))
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    #[test]
    fn rejects_invalid_servers_and_queries() {
        assert!(whois("", "x.com", Duration::from_secs(1)).is_err());
        assert!(whois("a b", "x.com", Duration::from_secs(1)).is_err());
        assert!(whois("whois.example", "x\r\ny", Duration::from_secs(1)).is_err());
    }

    #[test]
    fn sends_one_line_and_reads_until_closed() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            let answer = format!("Domain: {}\nCreation Date: 2001-03-15\n", line.trim_end());
            stream.write_all(answer.as_bytes()).unwrap();
        });
        let answer = ask(address, "ornek.com.tr", Duration::from_secs(5)).unwrap();
        assert!(answer.contains("Domain: ornek.com.tr\n"));
        assert!(answer.contains("2001-03-15"));
    }
}
