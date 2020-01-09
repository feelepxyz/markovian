use rand::Rng;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use structopt::StructOpt;
use std::path::PathBuf;
use log::{debug, error, warn, info, trace};

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone)]
enum Symbol {
    Start,
    End,
    Char(u8),
    Compound(Vec<u8>),
}

#[derive(Debug)]
struct SymbolWeights {
    counts: BTreeMap<Symbol, usize>,
    total: usize,
}

impl SymbolWeights {
    fn new() -> SymbolWeights {
        SymbolWeights {
            counts: BTreeMap::new(),
            total: 0,
        }
    }

    fn add_symbol(&mut self, s: &Symbol) {
        self.total += 1;
        *self.counts.entry(s.clone()).or_insert(0) += 1;
    }

    fn sample_next_symbol<R: Rng>(&self, rng: &mut R) -> Symbol {
        let mut v = rng.gen_range(0, self.total);
        for (s, c) in self.counts.iter() {
            if v < *c {
                return s.clone();
            }
            v -= *c;
        }
        unreachable!();
    }
}

#[derive(Debug)]
struct MarkovModel {
    contexts: BTreeMap<Vec<Symbol>, SymbolWeights>,
    order: usize,
}

impl MarkovModel {
    fn new(order: usize) -> MarkovModel {
        MarkovModel {
            contexts: BTreeMap::new(),
            order,
        }
    }

    fn add(&mut self, s: &[Symbol]) {
        //println!("{:?} => {:?}", s , ss);
        use std::iter;
        let ss:Vec<Symbol> = iter::repeat(Symbol::Start)
                .take(self.order)
                .chain(s.iter().cloned())
                .chain(iter::repeat(Symbol::End).take(1))
                .collect();

        for w in ss.windows(self.order + 1) {
            for cl in 0..self.order {
                self.contexts
                    .entry(w[cl..self.order].to_vec())
                    .or_insert_with(SymbolWeights::new)
                    .add_symbol(&w[self.order]);
            }
        }
    }

    fn initial_context(&self) -> Vec<Symbol> {
        std::iter::repeat(Symbol::Start).take(self.order).collect()
    }

    fn sample_next_symbol<R: Rng>(&self, context: &[Symbol], rng: &mut R) -> Symbol {
        for i in 0..context.len() {
            let weights = self.contexts.get(&context[i..]);
            if let Some(ws) = weights {
                return ws.sample_next_symbol(rng);
            }
        }
        unreachable!();
    }

    fn sample<R: Rng>(&self, rng: &mut R, print_sep:bool) -> String {
        let mut context = self.initial_context();
        let L = context.len();
        let mut symbols = vec![];
        loop {
            let s = self.sample_next_symbol(&context, rng);
            match &s {
                Symbol::End => break,
                Symbol::Start => unimplemented!(),
                _ => {}
            }
            symbols.push(s.clone());
            context.rotate_left(1);
                    //println!("Emitting {:?}", c);
            context[L - 1] = s;
        }
        let symbol_refs: Vec<_> = symbols.iter().collect();
        symbols_to_word(&symbol_refs, print_sep)
    }
}

fn raw_symbolify_word(s :&str) -> Vec<Symbol> {
    s.as_bytes().iter().cloned().map(Symbol::Char).collect()
}

fn reduce_symbols( v:Vec<Symbol>, key:(&Symbol, &Symbol), value:&Symbol) -> Vec<Symbol> {
    let mut result: Vec<Symbol> = vec![];
    let mut skip = false;
    for i in 0.. v.len()-1 {
        if skip {
            skip = false;
            continue
        }
        if (&v[i] == key.0) && (&v[i+1] == key.1) {
            result.push(value.clone());
            skip = true;
        } else {
            result.push(v[i].clone());
        }
    }
    if ! skip {
        v.last().iter().cloned().for_each(|s:&Symbol| result.push(s.clone()));
    }
    result
}

fn symbols_to_vec(v:&[&Symbol], insert_sep:bool) -> Vec<u8> {
    let mut result:Vec<u8> = vec![];
    let mut is_start = true;
    for s in v {
        if insert_sep { 
            if is_start {
                is_start = false
            } else {
                result.push(b'|')
            }
        }
        match s {
            Symbol::Start => { result.push(b'^'); },
            Symbol::End => { result.push(b'$'); },
            Symbol::Char(c) => { result.push(*c); },
            Symbol::Compound(cs) => { result.append(&mut cs.clone()); }
        }
    }
    result
}

fn symbols_to_word(v:&[&Symbol], insert_sep:bool) -> String {
    String::from_utf8(symbols_to_vec(v, insert_sep)).unwrap()
}


fn get_symbol_counts(input_names:&[Vec<Symbol>]) -> BTreeMap<Symbol, usize> {
    let mut symbol_counts: BTreeMap<Symbol, usize> = BTreeMap::new();
    for name in input_names {
        for w in name {
            *symbol_counts.entry(w.clone()).or_insert(0) += 1;
        }
    }
    symbol_counts
}

fn get_bigram_counts(input_names:&[Vec<Symbol>]) -> BTreeMap<(Symbol, Symbol), usize> {
    let mut bigram_counts: BTreeMap<(Symbol, Symbol), usize> = BTreeMap::new();
    for name in input_names {
        for w in name.windows(2) {
            *bigram_counts.entry((w[0].clone(), w[1].clone())).or_insert(0) += 1;
        }
    }
    bigram_counts
}

fn get_trigram_counts(input_names:&[Vec<Symbol>]) -> BTreeMap<(Symbol, Symbol, Symbol), usize> {
    let mut trigram_counts: BTreeMap<(Symbol, Symbol, Symbol), usize> = BTreeMap::new();
    for name in input_names {
        for w in name.windows(3) {
            *trigram_counts.entry((w[0].clone(), w[1].clone(), w[2].clone())).or_insert(0) += 1;
        }
    }
    trigram_counts
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ProductionRuleId(u64);

#[derive(Debug,Clone)]
struct ProductionRuleCompound {
    values:Vec<(u32, Vec<ProductionRuleId>)>
}

impl ProductionRuleCompound {
    pub fn evaluate<R: Rng>(&self, r:&mut R) -> Vec<ProductionRuleId> {
        let sum_w: u32 = self.values.iter().map(|v| v.0).sum();
        let mut v:u32 = r.gen_range(0, sum_w);
        for r in &self.values {
            if r.0 < v {
                //TODO: Would be nice to avoid this copy.
                return r.1.clone();
            }
            v -= r.0;
        }
        unreachable!()
    }
}

#[derive(Debug,Clone)]
enum ProductionRule {
    Terminal(u8),
    Compound(ProductionRuleCompound),
}

struct ProductionRuleSet {
    rules:BTreeMap<ProductionRuleId, ProductionRule>
}

fn reduce<R: Rng>(id:ProductionRuleId, rules:&ProductionRuleSet, r:&mut R) -> Vec<u8> {
    let mut pending:VecDeque<ProductionRuleId> = VecDeque::new();
    let mut result:Vec<u8> = vec![];
    pending.push_front(id);
    while !pending.is_empty() {
        let next_id = pending.pop_front().unwrap();
        let rule = rules.rules.get(&next_id).unwrap();
        match rule {
            ProductionRule::Terminal(v) => {
                result.push(*v);
            },
            ProductionRule::Compound(compound_rule) => {
                let mut v = compound_rule.evaluate(r);
                v.reverse();
                for x in v.iter() {
                    pending.push_front(*x)
                }
            }
        }
    }
    result
}

fn test_productions() {
    let T1_id = ProductionRuleId(1);
    let T1 = ProductionRule::Terminal(b'A');
    let T2_id = ProductionRuleId(2);
    let T2 = ProductionRule::Terminal(b'B');

    let P1_id = ProductionRuleId(0);
    let P1 = ProductionRule::Compound(
        ProductionRuleCompound{
            values:vec![
                (1, vec![T1_id]),
                (1, vec![T2_id]),
                (1, vec![P1_id,P1_id]),
            ]
        }
    );

    let mut rules_map = BTreeMap::new();
    rules_map.insert(T1_id, T1);
    rules_map.insert(T2_id, T2);
    rules_map.insert(P1_id, P1);

    let rules = ProductionRuleSet{
        rules:rules_map
    };
    let mut rng = rand::thread_rng();
    for i in 0..10 {
        println!("P1=>{:?}", String::from_utf8(reduce(P1_id, &rules, &mut rng)));
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "markovian", about = "Markov based name generator.")]
struct Opt {
    /// Input file
    #[structopt(short, long, parse(from_os_str))]
    name_file: PathBuf,

    /// Number of bigrams to reduce to their own symbols
    #[structopt(short, long, default_value="-1")]
    bigram_reduce_count: i32,

    /// Verbosity level
    #[structopt(short, long, default_value="0")]
    verbose: i32,

    /// Run productions test
    #[structopt(short, long)]
    run_productions_test: bool,
}

fn setup_logging(verbose:i32) {
    use fern::colors::{Color, ColoredLevelConfig};

    let level = match verbose {
        v if v <= 0 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        _ => log::LevelFilter::Trace,
    };
    // With fern, we can:

    // Configure logger at runtime
    fern::Dispatch::new()
        // Perform allocation-free log formatting
        .format(|out, message, record| {
            out.finish(format_args!(
                //"{}[{}][{}] {}",
                "[{}][{}] {}",
                //chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        // Add blanket level filter -
        .level(level)
        // Output to stdout, files, and other Dispatch configurations
        .chain(std::io::stdout())
        //.chain(fern::log_file("output.log").unwrap())
        // Apply globally
        .apply().unwrap();

    // and log using log crate macros!
    info!("helllo, world!");
}

fn main() {
    use log::log_enabled;
    use log::Level::{Info,Debug};

    let opt = Opt::from_args();
    println!("{:?}", opt);
    if opt.run_productions_test {
        test_productions();
        return;
    }

    let input_names_raw = std::fs::read_to_string(opt.name_file).unwrap();
    let verbose:i32 = opt.verbose;
    setup_logging(verbose);
    let order = 3;
    let print_sep = verbose >= 1;

    info!("Loading word list...");

    let input_names: Vec<Vec<Symbol>> = input_names_raw
        .lines()
        .map(|s| s.trim())
        .filter(|s| s.len() >= 3)
        .map(|s| raw_symbolify_word(s))
        .collect();
    //println!("{:?}", symbolify_word(input_names[0], order));

    info!("{:?} raw entries", input_names_raw.lines().count());
    info!("{:?} entries", input_names.len());

    let symbol_counts = get_symbol_counts(&input_names);
    let mut symbol_counts: Vec<_> = symbol_counts.into_iter().collect();
    symbol_counts.sort_by_key(|e| e.1);
    symbol_counts.reverse();
    if log_enabled!(Info) {
        for x in &symbol_counts {
            info!("{:?} {:?}", symbols_to_word(&[&x.0], false), x.1);
        }
    }

    //println!("{:?}", bigram_counts);
    let bigram_counts = get_bigram_counts(&input_names);
    let mut bigram_counts: Vec<_> = bigram_counts.into_iter().collect();
    bigram_counts.sort_by_key(|e| e.1);
    bigram_counts.reverse();
    if log_enabled!(Info) {
        for x in &bigram_counts[0..10] {
            //let v = [(x.0).0, (x.0).1];
            info!("{:?} {:?}", symbols_to_word(&[&(x.0).0, &(x.0).1], false), x.1);
        }
    }

    //println!("{:?}", bigram_counts);
    let trigram_counts = get_trigram_counts(&input_names);
    let mut trigram_counts: Vec<_> = trigram_counts.into_iter().collect();
    trigram_counts.sort_by_key(|e| e.1);
    trigram_counts.reverse();
    if log_enabled!(Info) {
        for x in &trigram_counts[0..10] {
            info!("{:?} {:?}", symbols_to_word(&[&(x.0).0, &(x.0).1, &(x.0).2], false), x.1);
        }
    }

    let mut input_names = input_names;
    if opt.bigram_reduce_count > 0 {
        let mut bigram_counts = bigram_counts;
        for _i in 0..opt.bigram_reduce_count {
            let most_common_bigram=bigram_counts[0].0.clone();
            info!("Removing bigram {:?}", most_common_bigram);
            let s = Symbol::Compound(symbols_to_vec(&[&most_common_bigram.0, &most_common_bigram.1], false));
            input_names = input_names.into_iter().map(
                |v| reduce_symbols(v, (&most_common_bigram.0, &most_common_bigram.1), &s)
            ).collect();
            let bigram_counts_map = get_bigram_counts(&input_names);
            bigram_counts = bigram_counts_map.into_iter().collect();
            bigram_counts.sort_by_key(|e| e.1);
            bigram_counts.reverse();
            if log_enabled!(Debug) {
                debug!("--------");
                for x in &bigram_counts[0..10] {
                    debug!("{:?} {:?}", symbols_to_word(&[&(x.0).0, &(x.0).1], false), x.1);
                }
            }
        }
    }
    let input_names = input_names;

    // Hunt for symbols that are only ever used before another
    // These can be reduced to a combined symbol cheaply
    let bigram_counts = get_bigram_counts(&input_names);
    let mut symbol_to_followers : BTreeMap<Symbol, BTreeSet<Symbol>> = BTreeMap::new();
    for s in bigram_counts.keys() {
        symbol_to_followers
            .entry(s.0.clone())
            .or_insert_with(BTreeSet::new)
            .insert(s.1.clone());
    }
    info!("====");
    for (k,ss) in &symbol_to_followers {
        if ss.len() > 3 {
            debug!("{} => {}", symbols_to_word(&[k], false), ss.len());
        } else {
            if log_enabled!(Info) {
                let sss: Vec<String> = ss.iter().map( |k| symbols_to_word(&[k], false)).collect();
                info!("{} => {:?}", symbols_to_word(&[k], false), sss);
            }
        }
    } 
    //println!("{:?}",symbol_to_followers );

    // Hunt for symbols that are only ever used after another
    // These can be reduced to a combined symbol cheaply
    let bigram_counts = get_bigram_counts(&input_names);
    let mut symbol_to_prefix : BTreeMap<Symbol, BTreeSet<Symbol>> = BTreeMap::new();
    for s in bigram_counts.keys() {
        symbol_to_prefix
            .entry(s.1.clone())
            .or_insert_with(BTreeSet::new)
            .insert(s.0.clone());
    }
    info!("====");
    for (k,ss) in &symbol_to_prefix {
        if ss.len() > 3 {
            debug!("{} => {}", ss.len(), symbols_to_word(&[k], false));
        } else {
            if log_enabled!(Info) {
                let sss: Vec<String> = ss.iter().map( |k| symbols_to_word(&[k], false)).collect();
                info!("{:?} => {}", sss, symbols_to_word(&[k], false));
            }
        }
    } 

    let mut model = MarkovModel::new(order);

    info!("Populating model...");
    for name in &input_names {
        //println!("Adding {:?}", name);
        model.add(name);
    }

    info!("total contexts = {:?}", model.contexts.len());
    let mut h: BTreeSet<Symbol> = BTreeSet::new();
    for k in model.contexts.keys() {
        for s in k {
            h.insert(s.clone());
        }
    }
    if log_enabled!(Info) {
        info!("total unique symbols = {:?}", h.len());
        for s in &h {
            info!("{:?}|", symbols_to_word(&[s], false));
        }
        info!("");
    }

    let mut rng = rand::thread_rng();
    info!("Sampling model...");

    for _ in 0..10 {
        if print_sep {
            println!("{:?}", model.sample(&mut rng, true));
        } else {
            println!("{}", model.sample(&mut rng, false));
        }
    }

    /*
        let mut model = MarkovModel::new(support, order, prior);
        for name in names {
            model.observe(name.chars().collect())
        }

        println!("{}", model.generate());

        println!("Hello, world!");
    */
}
