use std::collections::HashSet;
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

struct ServerState {
    vardai: HashSet<String>,
    isvedimai: Vec<Arc<Mutex<TcpStream>>>,
}

impl ServerState {
    fn new() -> Self {
        ServerState {
            vardai: HashSet::new(),
            isvedimai: Vec::new(),
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Naudojimas: {} <portas> [adresas]", args[0]);
        std::process::exit(1);
    }

    let portas: u16 = args[1].parse().expect("Neteisingas porto numeris");

    // Pasirinktinas IP adreso argumentas
    let ip_adresas = if args.len() >= 3 {
        args[2].clone()
    } else {
        "0.0.0.0".to_string()
    };

    let adresas = if ip_adresas.contains(":") && !ip_adresas.starts_with('[') {
        format!("[{}]:{}", ip_adresas, portas)
    } else {
        format!("{}:{}", ip_adresas, portas)
    };

    let klausytojas = TcpListener::bind(&adresas).expect("Nepavyko sukurti serverio soketo");
    println!("Serveris veikia: {}", adresas);

    let busena = Arc::new(Mutex::new(ServerState::new()));

    for srautas in klausytojas.incoming() {
        match srautas {
            Ok(srautas) => {
                let busena = Arc::clone(&busena);
                thread::spawn(move || {
                    tvarkyti_klienta(srautas, busena);
                });
            }
            Err(e) => eprintln!("Klaida priimant ryšį: {}", e),
        }
    }
}

fn tvarkyti_klienta(srautas: TcpStream, busena: Arc<Mutex<ServerState>>) {
    let skaitymo_srautas = match srautas.try_clone() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Nepavyko klonuoti soketo: {}", e);
            return;
        }
    };

    let rasymo_srautas = Arc::new(Mutex::new(srautas));
    let mut skaitytuvas = BufReader::new(skaitymo_srautas);

    let vardas = loop {
        {
            let mut isvestis = rasymo_srautas.lock().unwrap();
            if isvestis.write_all(b"ATSIUSKVARDA\n").is_err() {
                return;
            }
        }

        let mut eilute = String::new();
        match skaitytuvas.read_line(&mut eilute) {
            Ok(0) | Err(_) => return,
            _ => {}
        }

        let vardas = eilute.trim().to_string();
        if vardas.is_empty() {
            continue;
        }

        let mut busena = busena.lock().unwrap();
        if !busena.vardai.contains(&vardas) {
            busena.vardai.insert(vardas.clone());
            busena.isvedimai.push(Arc::clone(&rasymo_srautas));
            break vardas;
        }
    };

    {
        let mut isvestis = rasymo_srautas.lock().unwrap();
        if isvestis.write_all(b"VARDASOK\n").is_err() {
            issregistruoti(&vardas, &rasymo_srautas, &busena);
            return;
        }
    }

    println!("Prisijungė: {}", vardas);

    loop {
        let mut eilute = String::new();
        match skaitytuvas.read_line(&mut eilute) {
            Ok(0) | Err(_) => break,
            _ => {}
        }

        let ivesta = eilute.trim();
        if ivesta.is_empty() {
            continue;
        }

        let pranesimas = format!("PRANESIMAS {}: {}\n", vardas, ivesta);
        println!("{}: {}", vardas, ivesta);

        // Siunciame visiems prisijungusiems
        let busena = busena.lock().unwrap();
        for isvestis in &busena.isvedimai {
            let mut isvestis = isvestis.lock().unwrap();
            let _ = isvestis.write_all(pranesimas.as_bytes());
        }
    }

    println!("Atsijungė: {}", vardas);
    issregistruoti(&vardas, &rasymo_srautas, &busena);
}

fn issregistruoti(
    vardas: &str,
    isvestis: &Arc<Mutex<TcpStream>>,
    busena: &Arc<Mutex<ServerState>>,
) {
    let mut busena = busena.lock().unwrap();
    busena.vardai.remove(vardas);
    busena
        .isvedimai
        .retain(|i| !Arc::ptr_eq(i, isvestis));
}