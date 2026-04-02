use std::net::{TcpListener, TcpStream};
use std::io::{Write, Read};

fn handle_client(stream: TcpStream) {
    let mut writer = stream;

    let mut buffer = [0u8; 8192];
    loop {
        match writer.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                println!("Received: {} bytes", n);

                let message = String::from_utf8_lossy(&buffer[..n]);
                println!("Received: {}", &message);

                for b in &mut buffer[..n] {
                    *b = b.to_ascii_uppercase();
                }

                if let Err(e) = writer.write_all(&buffer[..n]) {
                    eprintln!("Write error: {}", e);
                    break;
                }
            }
            Err(e) => {
                eprintln!("Read error: {}", e);
                break;
            }
        }
    }
}

fn main() {
    let port = 20000;
    let host = "::1"; // local ipv6
    let address = format!("[{}]:{}", host, port);

    let listener = TcpListener::bind(&address)
        .expect("Can't open socket");

    println!("Server is listening on {}", address);

    match listener.accept() {
        Ok((stream, addr)) => {
            println!("Client connected to: {}", addr);
            handle_client(stream);
        }
        Err(e) => {
            println!("Accept error: {}", e);
        }
    };
}