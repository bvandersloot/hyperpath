extern crate treebitmap;

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::net::Ipv4Addr;
use std::str::FromStr;

use std::fs::File;

use aspath::treebitmap::IpLookupTableOps;

#[derive(Debug, PartialEq, Eq)]
enum ASRelation {
    Peers,
    Provides,
    Consumes,
}

#[derive(Debug, PartialEq, Eq)]
enum PathType {
    Null,
    U,
    P, // This is only built to allow PD to be found
    D,
    UD,
    UP,
    PD,
    UPD,
    Valleyed,
}

pub type ASN = u64;

#[allow(dead_code)]
pub struct HyperPath {
    trees: HashMap<ASN, treebitmap::IpLookupTable<Ipv4Addr, Vec<ASN>>>,
    relations: Option<HashMap<(ASN, ASN), ASRelation>>,
    pub networks: Vec<ASN>,
}

impl HyperPath {
    pub fn new() -> HyperPath {
        HyperPath {
            trees: HashMap::new(),
            relations: None,
            networks: Vec::new(),
        }
    }

    pub fn read_bgpdump(&mut self, bgpump_file: &str) {
        let f = File::open(bgpump_file).expect("file not found");
        for line in BufReader::new(f).lines().map(|x| x.unwrap()) {
            let fields: Vec<&str> = line.split("|").collect();
            let notifier = fields[4].parse::<ASN>().expect(line.as_str());
            let prefix = fields[5].to_string();
            let split_prefix: Vec<&str> = prefix.split("/").collect();
            let addr = Ipv4Addr::from_str(split_prefix[0]).unwrap();
            let prefix_length = split_prefix[1].parse::<u32>().expect(prefix.as_str());
            let path: Vec<ASN> = fields[6]
                .split(" ")
                .map(|x| {
                    if x.starts_with("{") {
                        let ases: Vec<&str> =
                            x.split(|x| x == '{' || x == '}' || x == ',').collect();
                        ases[1]
                    } else {
                        x
                    }
                })
                .map(|x| x.parse::<ASN>().expect(x))
                .collect();

            let tree = self.trees
                .entry(notifier)
                .or_insert(treebitmap::IpLookupTable::new());
            tree.insert(addr, prefix_length, path);
        }
        for (asn, _) in self.trees.iter() {
            self.networks.push(*asn);
        }
    }

    pub fn read_as_relations(&mut self, as_relation_file: &str) {
        let mut result = HashMap::new();
        let f = File::open(as_relation_file).expect("file not found");
        for line in BufReader::new(f).lines().map(|x| x.unwrap()) {
            let fields: Vec<&str> = line.split("|").collect();
            let a = fields[0].parse::<ASN>().unwrap();
            let b = fields[1].parse::<ASN>().unwrap();
            let r = fields[2].parse::<i64>().unwrap();
            if r == -1 {
                result.insert((a, b), ASRelation::Provides);
                result.insert((b, a), ASRelation::Consumes);
            } else if r == 0 {
                result.insert((a, b), ASRelation::Peers);
                result.insert((b, a), ASRelation::Peers);
            }
        }
        self.relations = Some(result)
    }

    pub fn path(&self, a1: &Ipv4Addr, a2: &Ipv4Addr) -> Option<Vec<ASN>> {
        let mut final_path = None;
        let mut final_unvalleyed_path = None;
        for (_, tree) in self.trees.iter() {
            if let Some((_, _, path1)) = tree.longest_match(*a1) {
                if let Some((_, _, path2)) = tree.longest_match(*a2) {
                    if let Some(path) = build_path(&path1, &path2) {
                        if self.relations.is_some() && self.valley_free(&path) {
                            let chooser = choose_shortest_path(path);
                            final_unvalleyed_path =
                                final_unvalleyed_path.or(Some(vec![])).map(chooser);
                        } else {
                            let chooser = choose_shortest_path(path);
                            final_path = final_path.or(Some(vec![])).map(chooser);
                        }
                    }
                }
            }
        }
        final_unvalleyed_path.or(final_path)
    }

    fn valley_free(&self, path: &Vec<ASN>) -> bool {
        if !self.relations.is_some() {
            panic!("calling valley_free without relations information");
        }
        let mut path_type = PathType::Null;
        if let Some(ref relations) = self.relations {
            for i in 0..path.len() - 1 {
                let edge = (path[i], path[i + 1]);
                if let Some(relationship) = relations.get(&edge) {
                    path_type = append_relation(path_type, relationship);
                }
            }
        }
        path_type != PathType::Valleyed
    }
}

fn append_relation(current_type: PathType, relationship: &ASRelation) -> PathType {
    let mut path_type = current_type;
    match relationship {
        ASRelation::Peers => match path_type {
            PathType::U => path_type = PathType::UP,
            PathType::Null => path_type = PathType::P,
            PathType::D | PathType::UD | PathType::PD | PathType::UPD => {
                path_type = PathType::Valleyed
            }
            _ => (),
        },
        ASRelation::Consumes => match path_type {
            PathType::Null => path_type = PathType::U,
            PathType::P
            | PathType::D
            | PathType::UD
            | PathType::UP
            | PathType::PD
            | PathType::UPD => path_type = PathType::Valleyed,
            _ => (),
        },
        ASRelation::Provides => match path_type {
            PathType::Null => path_type = PathType::D,
            PathType::P => path_type = PathType::PD,
            PathType::U => path_type = PathType::UD,
            PathType::UP => path_type = PathType::UPD,
            _ => (),
        },
    }
    if path_type == PathType::P {
        PathType::Valleyed
    } else {
        path_type
    }
}

fn choose_shortest_path(b: Vec<ASN>) -> impl FnOnce(Vec<ASN>) -> Vec<ASN> {
    move |path| {
        if path.len() == 0 || b.len() < path.len() {
            b
        } else {
            path
        }
    }
}

fn choose_best_option(pair: (usize, usize)) -> impl FnOnce((usize, usize)) -> (usize, usize) {
    move |current| {
        if current.0 + current.1 > pair.0 + pair.1 {
            current
        } else {
            pair
        }
    }
}

fn find_branching_point(a: &Vec<ASN>, b: &Vec<ASN>) -> Option<(usize, usize)> {
    let mut ret: Option<(usize, usize)> = None;
    for (ai, asn) in a.iter().enumerate() {
        if let Some(bi) = b.iter().position(|x| x == asn) {
            let default = (ai, bi);
            let chooser = choose_best_option(default);
            ret = Some(ret.map_or(default, chooser));
        }
    }
    ret
}

fn build_path(a: &Vec<ASN>, b: &Vec<ASN>) -> Option<Vec<ASN>> {
    if let Some(branches) = find_branching_point(&a, &b) {
        let (_, suba) = a.split_at(branches.0);
        let (_, subb) = b.split_at(branches.1 + 1);
        let mut ret = Vec::new();
        for n in (0..suba.len()).rev() {
            ret.push(suba[n])
        }
        ret.extend_from_slice(subb);
        Some(ret)
    } else {
        None
    }
}
