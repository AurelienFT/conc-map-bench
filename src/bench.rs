use std::{fmt::Debug, io, thread::sleep, time::Duration};

use bustle::*;
use massa_models::prehash::BuildMap;
use structopt::StructOpt;

use crate::{adapters::*, record::Record, workloads};

#[derive(Debug, StructOpt)]
pub struct Options {
    #[structopt(short, long)]
    pub workload: workloads::WorkloadKind,
    #[structopt(short, long, default_value = "1")]
    pub operations: f64,
    #[structopt(long)]
    pub read_threads: Option<Vec<u32>>,
    #[structopt(long)]
    pub write_threads: Option<Vec<u32>>,
    #[structopt(long)]
    pub use_std_hasher: bool,
    #[structopt(long, default_value = "2000")]
    pub gc_sleep_ms: u64,
    #[structopt(long)]
    pub skip: Option<Vec<String>>, // TODO: use just `Vec<String>`.
    #[structopt(long)]
    pub complete_slow: bool,
    #[structopt(long)]
    pub csv: bool,
    #[structopt(long)]
    pub csv_no_headers: bool,
}

fn gc_cycle(options: &Options) {
    sleep(Duration::from_millis(options.gc_sleep_ms));
    let mut new_guard = crossbeam_epoch::pin();
    new_guard.flush();
    for _ in 0..32 {
        new_guard.repin();
    }
}

type Handler = Box<dyn FnMut(&str, (&u32, &u32), &Measurement)>;

fn case<C>(name: &str, options: &Options, handler: &mut Handler)
where
    C: Collection,
    <C::Handle as CollectionHandle>::Key: Send + Debug,
{
    if options
        .skip
        .as_ref()
        .and_then(|s| s.iter().find(|s| s == &name))
        .is_some()
    {
        println!("-- {} [skipped]", name);
        return;
    } else {
        println!("-- {}", name);
    }

    println!("{:#?}", options.read_threads);

    let read_threads = options
        .read_threads
        .as_ref()
        .cloned()
        .unwrap_or_else(|| (1..(num_cpus::get() * 3 / 2 / 2) as u32).collect());

    let write_threads = options
        .write_threads
        .as_ref()
        .cloned()
        .unwrap_or_else(|| (1..(num_cpus::get() * 3 / 2 / 2) as u32).collect());

    let mut first_throughput = None;

    let scenarios = read_threads.iter().zip(write_threads.iter());
    for scenario in scenarios {
        let m = workloads::create(options, *scenario.0, *scenario.1).run_silently::<C>();
        handler(name, scenario, &m);

        if !options.complete_slow {
            let threshold = *first_throughput.get_or_insert(m.throughput) / 5.;
            if m.throughput <= threshold {
                println!("too long, skipped");
                break;
            }
        }

        gc_cycle(options);
    }
    println!();
}

fn run(options: &Options, h: &mut Handler) {
    case::<CrossbeamSkipMapTable<u64>>("CrossbeamSkipMap", options, h);
    case::<RwLockBTreeMapTable<u64>>("RwLock<BTreeMap>", options, h);

    case::<RwLockStdHashMapTable<u64, BuildMap<massa_hash::hash::Hash>>>(
        "RwLock<MassaPreHashMap>",
        options,
        h,
    );
    case::<DashMapTable<u64, BuildMap<massa_hash::hash::Hash>>>("MassaPreHashDashMap", options, h);
    case::<FlurryTable<u64, BuildMap<massa_hash::hash::Hash>>>("MassaPreHashFlurry", options, h);
    case::<EvmapTable<u64, BuildMap<massa_hash::hash::Hash>>>("MassaPreHashEvmap", options, h);
}

pub fn bench(options: &Options) {
    println!("== {:?}", options.workload);

    let mut handler = if options.csv {
        let mut wr = csv::WriterBuilder::new()
            .has_headers(!options.csv_no_headers)
            .from_writer(io::stderr());

        Box::new(move |name: &str, n: (&u32, &u32), m: &Measurement| {
            wr.serialize(Record {
                name: name.into(),
                total_ops: m.total_ops,
                read_threads: *n.0,
                write_threads: *n.1,
                spent: m.spent,
                throughput: m.throughput,
                latency: m.latency,
            })
            .expect("cannot serialize");
            wr.flush().expect("cannot flush");
        }) as Handler
    } else {
        Box::new(|_: &str, n: (&u32, &u32), m: &Measurement| {
            eprintln!(
                "total_ops={}\tread_threads={}\twrite_threads={}\tspent={:.1?}\tlatency={:?}\tthroughput={:.0}op/s",
                m.total_ops, n.0, n.1, m.spent, m.latency, m.throughput,
            );
        }) as Handler
    };

    run(&options, &mut handler);
}
