use std::net::TcpStream;
use std::io::{self, Read, Write};

fn main() {
    let host = "::1";
    let port = 20000;
    let address = format!("[{}]:{}", host, port);
    
    println!("Connecting to {}", address);

    let mut stream = match TcpStream::connect(&address) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error opening socket: {}", e);
            return;
        }
    };

    loop {
        print!("Enter a line to send: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Failed to read line");

        stream.write_all(input.as_bytes()).expect("Failed to send data");

        let mut buffer = Vec::new();
        let mut tmp = [0; 1024];

        loop {
            let n = stream.read(&mut tmp).expect("Failed to receive data");
            if n == 0 {
                break;
            }
            buffer.extend_from_slice(&tmp[..n]);
            if tmp[..n].contains(&b'\n') {
                break;
            }
        }

        let received = String::from_utf8_lossy(&buffer);
        print!("Received: {}", received);
    }
}