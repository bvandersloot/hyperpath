mod aspath;
extern crate clap;

use aspath::HyperPath;

use clap::*;

use std::io::{self, BufRead};
use std::net::Ipv4Addr;

fn main() {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();
    let bgpdump = matches.value_of("bgpdump").expect("Requires bgpdump file.");
    let asrelations = matches.value_of("asrelations").expect("Requires asrelations file.");
    let mut path_predictor = HyperPath::new();
    path_predictor.read_bgpdump(bgpdump);
    path_predictor.read_as_relations(asrelations);

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let l = line.expect("Failed to read stdin");
        let mut fields : Vec<&str> = l.split(" ").collect();
        if fields.len() == 2 {
            let source : Ipv4Addr = fields[0].parse().expect("Must be a parsable IP address");
            let destination : Ipv4Addr = fields[1].parse().expect("Must be a parsable IP address");
            let empty : Vec<u64> = vec![];
            let path = path_predictor.path(&source, &destination).unwrap_or(empty);
            let output = path.iter().map(|&x| x.to_string()).collect::<Vec<String>>().join(" ");
            println!("{}", output);
        } else {
            continue;
        }
    }
}
