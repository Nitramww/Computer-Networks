use std::collections::HashSet;
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const BAZINIS_PORTAS: u16 = 59001;
const SERVERIU_KIEKIS: u16 = 6;

struct ServerState {
    id: String,
    vardai: HashSet<String>,
    klientai: Vec<Arc<Mutex<TcpStream>>>,
    kaimynai: Vec<Arc<Mutex<TcpStream>>>,
    matyti: Vec<(String, Instant)>,
}

impl ServerState {
    fn new(id: String) -> Self {
        ServerState {
            id,
            vardai: HashSet::new(),
            klientai: Vec::new(),
            kaimynai: Vec::new(),
            matyti: Vec::new(),
        }
    }

    fn jau_matyta(&mut self, raktas: &str) -> bool {
        let dabar = Instant::now();
        self.matyti.retain(|(_, t)| dabar.duration_since(*t) < Duration::from_secs(10));
        if self.matyti.iter().any(|(r, _)| r == raktas) {
            return true;
        }
        self.matyti.push((raktas.to_string(), dabar));
        false
    }
}

fn kairys_portas(nr: u16) -> u16 {
    BAZINIS_PORTAS + (nr - 1 + SERVERIU_KIEKIS - 1) % SERVERIU_KIEKIS
}
fn desinys_portas(nr: u16) -> u16 {
    BAZINIS_PORTAS + nr % SERVERIU_KIEKIS
}

fn dabar_laikas() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Naudojimas: {} <serverio_nr 1-6>", args[0]);
        std::process::exit(1);
    }

    let nr: u16 = args[1].parse().expect("Neteisingas serverio numeris");
    if nr < 1 || nr > SERVERIU_KIEKIS {
        eprintln!("Serverio numeris turi būti 1–{}", SERVERIU_KIEKIS);
        std::process::exit(1);
    }

    let serverio_id = format!("S{}", nr);
    let savas_portas = BAZINIS_PORTAS + nr - 1;
    let kairys = kairys_portas(nr);
    let desinys = desinys_portas(nr);

    println!(
        "[{}] portas={} ← S{}({}) | S{}({}) →",
        serverio_id, savas_portas,
        (kairys - BAZINIS_PORTAS + 1), kairys,
        (desinys - BAZINIS_PORTAS + 1), desinys
    );

    let busena = Arc::new(Mutex::new(ServerState::new(serverio_id.clone())));

    for (kryptis, portas) in [("<-", kairys), ("->", desinys)] {
        let busena2 = Arc::clone(&busena);
        let sid = serverio_id.clone();
        thread::spawn(move || prijungti_kaimyna(sid, kryptis, portas, busena2));
    }

    let adresas = format!("[::]:{}", savas_portas);
    let klausytojas = TcpListener::bind(&adresas).expect("Nepavyko sukurti serverio soketo");

    for srautas in klausytojas.incoming() {
        match srautas {
            Ok(srautas) => {
                let busena = Arc::clone(&busena);
                thread::spawn(move || tvarkyti_klienta(srautas, busena));
            }
            Err(e) => eprintln!("Klaida priimant ryšį: {}", e),
        }
    }
}

fn siusti_visiems(busena: &Arc<Mutex<ServerState>>, vietinis: &str, kaimynams: &str) {
    let b = busena.lock().unwrap();
    for arc in &b.klientai {
        let mut s = arc.lock().unwrap();
        let _ = s.write_all(vietinis.as_bytes());
    }
    for arc in &b.kaimynai {
        let mut s = arc.lock().unwrap();
        let _ = s.write_all(kaimynams.as_bytes());
    }
}

fn prijungti_kaimyna(
    serverio_id: String,
    kryptis: &'static str,
    portas: u16,
    busena: Arc<Mutex<ServerState>>,
) {
    let adresas = format!("localhost:{}", portas);
    let srautas = loop {
        match TcpStream::connect(&adresas) {
            Ok(s) => break s,
            Err(_) => thread::sleep(Duration::from_secs(1)),
        }
    };
    println!("[{}] Prisijungta prie kaimyno {} {}", serverio_id, kryptis, adresas);

    let skaitymo_srautas = srautas.try_clone().expect("Klonavimo klaida");
    let rasymo_srautas = Arc::new(Mutex::new(srautas));
    let mut skaitytuvas = BufReader::new(skaitymo_srautas);

    let kaimyno_vardas = format!("__SERVER_{}_{}", serverio_id, kryptis);
    loop {
        let mut eilute = String::new();
        if skaitytuvas.read_line(&mut eilute).unwrap_or(0) == 0 { return; }
        match eilute.trim() {
            "ATSIUSKVARDA" => {
                let mut isvestis = rasymo_srautas.lock().unwrap();
                let _ = isvestis.write_all(format!("{}\n", kaimyno_vardas).as_bytes());
            }
            "VARDASOK" => break,
            _ => {}
        }
    }

    {
        let mut b = busena.lock().unwrap();
        b.kaimynai.push(Arc::clone(&rasymo_srautas));
    }
    println!("[{}] Kaimynas {} {} įregistruotas", serverio_id, kryptis, adresas);

    loop {
        let mut eilute = String::new();
        match skaitytuvas.read_line(&mut eilute) {
            Ok(0) | Err(_) => break,
            _ => {}
        }
        let ivesta = eilute.trim().to_string();
        if !ivesta.is_empty() {
            apdoroti_zinute_is_kaimyno(&ivesta, &busena);
        }
    }

    let mut b = busena.lock().unwrap();
    b.kaimynai.retain(|i| !Arc::ptr_eq(i, &rasymo_srautas));
}

fn tvarkyti_klienta(srautas: TcpStream, busena: Arc<Mutex<ServerState>>) {
    let skaitymo_srautas = match srautas.try_clone() {
        Ok(s) => s,
        Err(e) => { eprintln!("Nepavyko klonuoti soketo: {}", e); return; }
    };
    let rasymo_srautas = Arc::new(Mutex::new(srautas));
    let mut skaitytuvas = BufReader::new(skaitymo_srautas);

    let vardas = loop {
        {
            let mut isvestis = rasymo_srautas.lock().unwrap();
            if isvestis.write_all(b"ATSIUSKVARDA\n").is_err() { return; }
        }
        let mut eilute = String::new();
        match skaitytuvas.read_line(&mut eilute) {
            Ok(0) | Err(_) => return,
            _ => {}
        }
        let vardas = eilute.trim().to_string();
        if vardas.is_empty() { continue; }

        if vardas.starts_with("__SERVER_") {
            {
                let mut isvestis = rasymo_srautas.lock().unwrap();
                let _ = isvestis.write_all(b"VARDASOK\n");
            }
            loop {
                let mut eilute = String::new();
                match skaitytuvas.read_line(&mut eilute) {
                    Ok(0) | Err(_) => break,
                    _ => {}
                }
                let ivesta = eilute.trim().to_string();
                if !ivesta.is_empty() {
                    apdoroti_zinute_is_kaimyno(&ivesta, &busena);
                }
            }
            return;
        }


        let mut b = busena.lock().unwrap();
        if !b.vardai.contains(&vardas) {
            b.vardai.insert(vardas.clone());
            b.klientai.push(Arc::clone(&rasymo_srautas));
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

    let sid = busena.lock().unwrap().id.clone();
    println!("[{}] Prisijungė: {}", sid, vardas);

    loop {
        let mut eilute = String::new();
        match skaitytuvas.read_line(&mut eilute) {
            Ok(0) | Err(_) => break,
            _ => {}
        }
        let ivesta = eilute.trim().to_string();
        if ivesta.is_empty() { continue; }

        let timestamp = dabar_laikas();
        let raktas = format!("{}|{}|{}", timestamp, vardas, ivesta);

        let sid = {
            let mut b = busena.lock().unwrap();
            if b.jau_matyta(&raktas) { continue; }
            b.id.clone()
        };

        println!("[{}] {}: {}", sid, vardas, ivesta);

        // be prefikso
        let vietinis  = format!("PRANESIMAS {} {}: {}\n", timestamp, vardas, ivesta);
        // Kaimynams su prefiksu
        let kaimynams = format!("PRANESIMAS ({}) {} {}: {}\n", sid, timestamp, vardas, ivesta);

        siusti_visiems(&busena, &vietinis, &kaimynams);
    }

    let sid = busena.lock().unwrap().id.clone();
    println!("[{}] Atsijungė: {}", sid, vardas);
    issregistruoti(&vardas, &rasymo_srautas, &busena);
}

fn apdoroti_zinute_is_kaimyno(eilute: &str, busena: &Arc<Mutex<ServerState>>) {
    if !eilute.starts_with("PRANESIMAS ") { return; }
    let turinys = &eilute["PRANESIMAS ".len()..];

    // Raktas: "ts | vardas | zinute"   
    let raktas = {
        let be_prefikso = if turinys.starts_with('(') {
            turinys.find(')').map(|p| turinys[p + 1..].trim()).unwrap_or(turinys)
        } else {
            turinys
        };
        let mut dalys = be_prefikso.splitn(2, ' ');
        let ts = dalys.next().unwrap_or("");
        let likutis = dalys.next().unwrap_or("");
        let mut vd = likutis.splitn(2, ": ");
        let vardas = vd.next().unwrap_or("");
        let zinute = vd.next().unwrap_or("");
        format!("{}|{}|{}", ts, vardas, zinute)
    };

    let sid = {
        let mut b = busena.lock().unwrap();
        if b.jau_matyta(&raktas) { return; }
        b.id.clone()
    };

    println!("[{}] ← {}", sid, turinys);

    // Vietiniams siunciame su prefiksu
    // Kaimynams perduodame toliau
    let wire = format!("PRANESIMAS {}\n", turinys);
    siusti_visiems(busena, &wire, &wire);
}

fn issregistruoti(vardas: &str, isvestis: &Arc<Mutex<TcpStream>>, busena: &Arc<Mutex<ServerState>>) {
    let mut b = busena.lock().unwrap();
    b.vardai.remove(vardas);
    b.klientai.retain(|i| !Arc::ptr_eq(i, isvestis));
    b.kaimynai.retain(|i| !Arc::ptr_eq(i, isvestis));
}