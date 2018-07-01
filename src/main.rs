mod aspath;
extern crate clap;

use aspath::HyperPath;

use clap::*;

use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::net::{Ipv4Addr, TcpListener};

fn single_query(path_predictor : &HyperPath, l : &str) -> String {
    let fields : Vec<&str> = l.split(" ").collect();
    if fields.len() == 2 {
        let source : Ipv4Addr = fields[0].parse().expect("Must be a parsable IP address");
        let destination : Ipv4Addr = fields[1].parse().expect("Must be a parsable IP address");
        let empty : Vec<u64> = vec![0];
        let path = path_predictor.path(&source, &destination).unwrap_or(empty);
        let output = path.iter().map(|&x| x.to_string()).collect::<Vec<String>>().join(" ");
        output
    } else {
       String::from("0\n")
    }
}

fn main() {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();
    let bgpdump = matches.value_of("bgpdump").expect("Requires bgpdump file.");
    let asrelations = matches.value_of("asrelations").expect("Requires asrelations file.");
    let mut path_predictor = HyperPath::new();
    path_predictor.read_bgpdump(bgpdump);
    path_predictor.read_as_relations(asrelations);

    if let Some(port) = matches.value_of("servermode") {
        print!("Listening on localhost:{}\n", port);
        let listener = TcpListener::bind(format!("127.0.0.1:{}",port)).unwrap();
        for stream in listener.incoming() {
            print!("new connection\n");
            let mut stream = stream.unwrap();
            let br = BufReader::new(&stream);
            let mut bw = BufWriter::new(&stream);
            for l in br.lines() {
                let l = l.unwrap();
                if l.trim() == "q" || l.trim() == "" {
                    break
                }
                print!("{} {}\n", l, l.len());
                bw.write(&format!("{}\n", single_query(&path_predictor, l.as_str())).into_bytes()).unwrap();
                bw.flush().unwrap();
            }
        }
    } else {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let l = line.expect("Failed to read stdin");
            print!("{}",single_query(&path_predictor, l.as_str()))
        }
    }
}
