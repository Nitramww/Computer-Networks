use std::io::{self, BufRead, BufReader, Write};
use native_tls::{TlsConnector};
use std::net::TcpStream;
use std::io::Read;
use std::time::Duration;
use std::env;
use chrono::Utc;
use base64::{engine::general_purpose, Engine as _};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "smtp-client")]
#[command(about = "Simple SMTP client over raw sockets")]
struct Args {
    /// SMTP server host
    #[arg(long)]
    host: String,

    /// SMTP server port
    #[arg(long, default_value_t = 25)]
    port: u16,

    /// Sender email
    #[arg(long)]
    from: String,

    /// Recipient email
    #[arg(long)]
    to: Vec<String>,

    // Subject of email
    #[arg(long)]
    subject: String,

    /// Path to message file
    #[arg(long)]
    file: String,

    /// Enable debug output
    #[arg(long, default_value_t = false)]
    debug: bool,

    /// Use STARTTLS
    #[arg(long, default_value_t = false)]
    starttls: bool,
}

/// SMTP Client implementation following RFC 5321
pub struct SmtpClient {
    stream: SmtpStream,
    domain: String,
    debug: bool,
}

enum SmtpStream {
    Plain(TcpStream),
    Tls(native_tls::TlsStream<TcpStream>),
}

/// SMTP Response codes
#[derive(Debug, PartialEq)]
pub enum SmtpCode {
    ServiceReady,           // 220
    ServiceClosing,         // 221
    AuthSuccess,            // 235
    Ok,                     // 250
    UserNotLocal,           // 251
    CannotVrfy,             // 252
    AuthContinue,           // 334
    StartMailInput,         // 354
    ServiceNotAvailable,    // 421
    MailboxUnavailable,     // 450
    LocalError,             // 451
    InsufficientStorage,    // 452
    SyntaxError,            // 500
    SyntaxErrorParams,      // 501
    CommandNotImplemented,  // 502
    BadSequence,            // 503
    ParameterNotImplemented,// 504
    MailboxUnavailablePerm, // 550
    UserNotLocalPerm,       // 551
    ExceededStorage,        // 552
    MailboxNameNotAllowed,  // 553
    TransactionFailed,      // 554
    Unknown(u16),
}

impl SmtpCode {
    fn from_code(code: u16) -> Self {
        match code {
            220 => SmtpCode::ServiceReady,
            221 => SmtpCode::ServiceClosing,
            235 => SmtpCode::AuthSuccess,
            250 => SmtpCode::Ok,
            251 => SmtpCode::UserNotLocal,
            252 => SmtpCode::CannotVrfy,
            334 => SmtpCode::AuthContinue,
            354 => SmtpCode::StartMailInput,
            421 => SmtpCode::ServiceNotAvailable,
            450 => SmtpCode::MailboxUnavailable,
            451 => SmtpCode::LocalError,
            452 => SmtpCode::InsufficientStorage,
            500 => SmtpCode::SyntaxError,
            501 => SmtpCode::SyntaxErrorParams,
            502 => SmtpCode::CommandNotImplemented,
            503 => SmtpCode::BadSequence,
            504 => SmtpCode::ParameterNotImplemented,
            550 => SmtpCode::MailboxUnavailablePerm,
            551 => SmtpCode::UserNotLocalPerm,
            552 => SmtpCode::ExceededStorage,
            553 => SmtpCode::MailboxNameNotAllowed,
            554 => SmtpCode::TransactionFailed,
            _ => SmtpCode::Unknown(code),
        }
    }

    fn is_success(&self) -> bool {
        matches!(self, 
            SmtpCode::ServiceReady 
            | SmtpCode::ServiceClosing 
            | SmtpCode::Ok 
            | SmtpCode::UserNotLocal 
            | SmtpCode::CannotVrfy 
            | SmtpCode::StartMailInput
            | SmtpCode::AuthSuccess
            | SmtpCode::AuthContinue
        )
    }
}

/// SMTP Response
#[derive(Debug)]
pub struct SmtpResponse {
    code: SmtpCode,
    pub lines: Vec<String>,
}

impl SmtpResponse {
    pub fn is_success(&self) -> bool {
        self.code.is_success()
    }

    pub fn message(&self) -> String {
        self.lines.join("\n")
    }
}

impl SmtpClient {
    /// Connect to an SMTP server
    /// 
    /// # Arguments
    /// * `server` - Server address (e.g., "smtp.example.com")
    /// * `port` - Port number (typically 25 for SMTP, 587 for submission)
    /// * `domain` - Client domain name (used in EHLO/HELO)
    pub fn connect(server: &str, port: u16, domain: &str) -> io::Result<Self> {
        let addr = format!("{}:{}", server, port);
        let tcp = TcpStream::connect(&addr)?;
        
        // Set read timeout (RFC 5321 Section 4.5.3.2)
        tcp.set_read_timeout(Some(Duration::from_secs(300)))?;
        tcp.set_write_timeout(Some(Duration::from_secs(300)))?;

        let mut client = SmtpClient {
            stream: SmtpStream::Plain(tcp),
            domain: domain.to_string(),
            debug: false,
        };

        // Read greeting (220 Service ready)
        let response = client.read_response()?;
        if !response.is_success() {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                format!("Server greeting failed: {}", response.message()),
            ));
        }

        Ok(client)
    }

    pub fn starttls(&mut self) -> io::Result<()> {
        let resp = self.send_command("STARTTLS")?;
        if !resp.is_success() {
            return Err(io::Error::new(io::ErrorKind::Other, "STARTTLS failed"));
        }

        let connector = TlsConnector::new()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let tcp = match &mut self.stream {
            SmtpStream::Plain(s) => s,
            SmtpStream::Tls(_) => {
                return Err(io::Error::new(io::ErrorKind::Other, "Already TLS"));
            }
        };

        let tls = connector
            .connect(&self.domain, tcp.try_clone()?)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        self.stream = SmtpStream::Tls(tls);

        Ok(())
    }

    pub fn auth_login(&mut self, user: &str, pass: &str) -> io::Result<()> {
        let resp = self.send_command("AUTH LOGIN")?;
        if !resp.is_success() {
            return Err(io::Error::new(io::ErrorKind::Other, "AUTH LOGIN failed"));
        }

        let user_b64 = general_purpose::STANDARD.encode(user);
        let pass_b64 = general_purpose::STANDARD.encode(pass);

        let resp = self.send_command(&user_b64)?;
        if !resp.is_success() {
            return Err(io::Error::new(io::ErrorKind::Other, "Username rejected"));
        }

        let resp = self.send_command(&pass_b64)?;
        if !resp.is_success() {
            return Err(io::Error::new(io::ErrorKind::Other, "Password rejected"));
        }

        Ok(())
    }

    /// Enable debug output
    pub fn set_debug(&mut self, debug: bool) {
        self.debug = debug;
    }

    /// Send EHLO command (Extended HELLO) - RFC 5321 Section 4.1.1.1
    pub fn ehlo(&mut self) -> io::Result<SmtpResponse> {
        self.send_command(&format!("EHLO {}", self.domain))
    }

    /// Send HELO command (fallback for servers that don't support EHLO)
    pub fn helo(&mut self) -> io::Result<SmtpResponse> {
        self.send_command(&format!("HELO {}", self.domain))
    }

    /// Send MAIL FROM command - RFC 5321 Section 4.1.1.2
    /// 
    /// # Arguments
    /// * `from` - Sender email address
    pub fn mail_from(&mut self, from: &str) -> io::Result<SmtpResponse> {
        // Validate and format the reverse-path
        let reverse_path = if from.is_empty() {
            "<>".to_string() // Null reverse-path for bounce messages
        } else {
            format!("<{}>", from)
        };
        self.send_command(&format!("MAIL FROM:{}", reverse_path))
    }

    /// Send RCPT TO command - RFC 5321 Section 4.1.1.3
    /// 
    /// # Arguments
    /// * `to` - Recipient email address
    pub fn rcpt_to(&mut self, to: &str) -> io::Result<SmtpResponse> {
        let forward_path = format!("<{}>", to);
        self.send_command(&format!("RCPT TO:{}", forward_path))
    }

    /// Send DATA command and message content - RFC 5321 Section 4.1.1.4
    /// 
    /// # Arguments
    /// * `message` - Complete message including headers and body
    pub fn data(&mut self, message: &str) -> io::Result<SmtpResponse> {
        // Send DATA command
        let response = self.send_command("DATA")?;
        
        // Check for 354 intermediate response
        if response.code != SmtpCode::StartMailInput {
            return Ok(response);
        }

        // Send message with transparency procedure (RFC 5321 Section 4.5.2)
        // Lines starting with "." need to be prefixed with an extra "."
        for line in message.lines() {
            let line_to_send = if line.starts_with('.') {
                format!(".{}", line)
            } else {
                line.to_string()
            };
            self.write_line(&line_to_send)?;
        }

        // Send end of data indicator: <CRLF>.<CRLF>
        self.write_line(".")?;

        // Read final response
        self.read_response()
    }

    /// Send RSET command - RFC 5321 Section 4.1.1.5
    pub fn rset(&mut self) -> io::Result<SmtpResponse> {
        self.send_command("RSET")
    }

    /// Send VRFY command - RFC 5321 Section 4.1.1.6
    /// 
    /// # Arguments
    /// * `user` - User name or email address to verify
    pub fn vrfy(&mut self, user: &str) -> io::Result<SmtpResponse> {
        self.send_command(&format!("VRFY {}", user))
    }

    /// Send EXPN command - RFC 5321 Section 4.1.1.7
    /// 
    /// # Arguments
    /// * `list` - Mailing list name to expand
    pub fn expn(&mut self, list: &str) -> io::Result<SmtpResponse> {
        self.send_command(&format!("EXPN {}", list))
    }

    /// Send HELP command - RFC 5321 Section 4.1.1.8
    pub fn help(&mut self, topic: Option<&str>) -> io::Result<SmtpResponse> {
        match topic {
            Some(t) => self.send_command(&format!("HELP {}", t)),
            None => self.send_command("HELP"),
        }
    }

    /// Send NOOP command - RFC 5321 Section 4.1.1.9
    pub fn noop(&mut self) -> io::Result<SmtpResponse> {
        self.send_command("NOOP")
    }

    /// Send QUIT command and close connection - RFC 5321 Section 4.1.1.10
    pub fn quit(&mut self) -> io::Result<SmtpResponse> {
        let response = self.send_command("QUIT")?;
        // Connection will be closed when SmtpClient is dropped
        Ok(response)
    }

    /// Send a complete email message
    /// 
    /// This is a high-level helper that performs the complete mail transaction:
    /// MAIL FROM -> RCPT TO (for each recipient) -> DATA
    pub fn send_email(
        &mut self,
        from: &str,
        to: &[&str],
        message: &str,
    ) -> io::Result<SmtpResponse> {
        // MAIL FROM
        let response = self.mail_from(from)?;
        if !response.is_success() {
            return Ok(response);
        }

        // RCPT TO for each recipient
        for recipient in to {
            let response = self.rcpt_to(recipient)?;
            if !response.is_success() {
                return Ok(response);
            }
        }

        // DATA
        self.data(message)
    }

    /// Send a command and read response
    fn send_command(&mut self, command: &str) -> io::Result<SmtpResponse> {
        self.write_line(command)?;
        self.read_response()
    }

    /// Write a line to the server (adds CRLF)
    fn write_line(&mut self, line: &str) -> io::Result<()> {
        if self.debug {
            println!("C: {}", line);
        }
        
        // RFC 5321 Section 2.3.8: Lines are terminated with CRLF
        match &mut self.stream {
            SmtpStream::Plain(s) => {
                write!(s, "{}\r\n", line)?;
                s.flush()?;
            }
            SmtpStream::Tls(s) => {
                write!(s, "{}\r\n", line)?;
                s.flush()?;
            }
        }

        Ok(())
    }

    /// Read response from server
    /// 
    /// RFC 5321 Section 4.2: SMTP replies
    /// Handles both single-line and multi-line responses
    fn read_response(&mut self) -> io::Result<SmtpResponse> {
        let lines = {
            let stream: &mut dyn Read = match &mut self.stream {
                SmtpStream::Plain(s) => s,
                SmtpStream::Tls(s) => s,
            };

            let mut reader = BufReader::new(stream);

            let mut lines = Vec::new();
            let mut code: Option<u16> = None;

            loop {
                let mut line = String::new();
                reader.read_line(&mut line)?;

                if line.is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Connection closed by server",
                    ));
                }

                let line = line.trim_end_matches(&['\r', '\n'][..]);

                if self.debug {
                    println!("S: {}", line);
                }

                if line.len() < 3 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Invalid response: {}", line),
                    ));
                }

                let response_code = line[0..3].parse::<u16>().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Invalid response code: {}", &line[0..3]),
                    )
                })?;

                if code.is_none() {
                    code = Some(response_code);
                }

                let continuation = line.chars().nth(3).unwrap_or(' ');

                let message = if line.len() > 4 {
                    line[4..].to_string()
                } else {
                    String::new()
                };

                lines.push(message);

                if continuation == ' ' {
                    break;
                }
            }

            (SmtpCode::from_code(code.unwrap()), lines)
        };

        Ok(SmtpResponse {
            code: lines.0,
            lines: lines.1,
        })
    }
}

/// Main
fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();

    let args = Args::parse();

    let body = std::fs::read_to_string(&args.file)?;

    let to_header = args.to.join(", ");

    let date = Utc::now().to_rfc2822();

    let message = format!(
    "From: {}\r\nTo: {}\r\nSubject: {}\r\nDate: {}\r\n\r\n{}",
    args.from,
    to_header,
    args.subject,
    date,
    body
    );

    let mut client = SmtpClient::connect(&args.host, args.port, &args.host)?;
    

    client.set_debug(args.debug);

    // EHLO
    let _ = client.ehlo();

    // STARTTLS if needed
    if args.starttls {
        client.starttls()?;
        let _ = client.ehlo();
    }

    if let (Some(user), Some(pass)) = (
        env::var("SMTP_USER").ok(),
        env::var("SMTP_PASS").ok(),
    ) {
        client.auth_login(&user, &pass)?;
    }

    let to_refs: Vec<&str> = args.to.iter().map(|s| s.as_str()).collect();
    // Send email
    let response = client.send_email(
        &args.from,
        &to_refs,
        &message,
    )?;

    println!("Result: {}", response.message());

    client.quit()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smtp_code_from_code() {
        assert_eq!(SmtpCode::from_code(220), SmtpCode::ServiceReady);
        assert_eq!(SmtpCode::from_code(250), SmtpCode::Ok);
        assert_eq!(SmtpCode::from_code(550), SmtpCode::MailboxUnavailablePerm);
        assert!(matches!(SmtpCode::from_code(999), SmtpCode::Unknown(999)));
    }

    #[test]
    fn test_smtp_code_is_success() {
        assert!(SmtpCode::ServiceReady.is_success());
        assert!(SmtpCode::Ok.is_success());
        assert!(!SmtpCode::SyntaxError.is_success());
        assert!(!SmtpCode::MailboxUnavailablePerm.is_success());
    }
}