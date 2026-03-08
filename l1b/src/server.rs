use std::net::{TcpListener, TcpStream};
use std::io::{self, BufRead, BufReader, Write};

fn handle_client(stream: TcpStream) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut writer = stream;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                print!("Received: {}", line);

                let uppercased = line.to_uppercase();
                if let Err(e) = writer.write_all(uppercased.as_bytes()) {
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

    let listener = TcpListener::bind(&address).expect("Can't open socket");
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