use plonkish_backend::util::arithmetic::Field;
use plonkish_backend::{
    backend::{PlonkishBackend, PlonkishCircuit, WitnessEncoding},
    halo2_curves::bn256::Fr,
    pcs::CommitmentChunk,
    util::{
        end_timer, start_timer,
        test::std_rng,
        transcript::{InMemoryTranscript, Keccak256Transcript, TranscriptRead, TranscriptWrite},
    },
};
use std::{
    fmt::Display,
    fs::{create_dir, File, OpenOptions},
    io::{Cursor, Write},
    iter,
    path::Path,
    time::{Duration, Instant},
};

const OUTPUT_DIR: &str = "../chiquito/target";

pub fn bench_plonkish_backend<B, F: Field>(
    system: System,
    k: usize,
    circuit: &impl PlonkishCircuit<Fr>,
) where
    B: PlonkishBackend<Fr> + WitnessEncoding,
    Keccak256Transcript<Cursor<Vec<u8>>>: TranscriptRead<CommitmentChunk<Fr, B::Pcs>, Fr>
        + TranscriptWrite<CommitmentChunk<Fr, B::Pcs>, Fr>
        + InMemoryTranscript,
{
    create_output(&[system]);
    let circuit_info = circuit.circuit_info().unwrap();
    let instances = circuit.instances();

    let timer = start_timer(|| format!("{system}_setup-{k}"));
    let param = B::setup(&circuit_info, std_rng()).unwrap();
    end_timer(timer);

    let timer = start_timer(|| format!("{system}_preprocess-{k}"));
    let (pp, vp) = B::preprocess(&param, &circuit_info).unwrap();
    end_timer(timer);

    let proof = sample(system, k, || {
        let _timer = start_timer(|| format!("{system}_prove-{k}"));
        let mut transcript = Keccak256Transcript::default();
        B::prove(&pp, circuit, &mut transcript, std_rng()).unwrap();
        transcript.into_proof()
    });

    let _timer = start_timer(|| format!("{system}_verify-{k}"));
    let accept = {
        let mut transcript = Keccak256Transcript::from_proof((), proof.as_slice());
        B::verify(&vp, instances, &mut transcript, std_rng()).is_ok()
    };
    assert!(accept);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum System {
    HyperPlonk,
    UniHyperPlonk,
    Halo2,
    EspressoHyperPlonk,
}

impl System {
    fn output_path(&self) -> String {
        format!("{OUTPUT_DIR}/{self}")
    }

    fn output(&self) -> File {
        OpenOptions::new()
            .append(true)
            .open(self.output_path())
            .unwrap()
    }
}

impl Display for System {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            System::HyperPlonk => write!(f, "hyperplonk"),
            System::UniHyperPlonk => write!(f, "unihyperplonk"),
            System::Halo2 => write!(f, "halo2"),
            System::EspressoHyperPlonk => write!(f, "espresso_hyperplonk"),
        }
    }
}

fn create_output(systems: &[System]) {
    if !Path::new(OUTPUT_DIR).exists() {
        create_dir(OUTPUT_DIR).unwrap();
    }
    for system in systems {
        File::create(system.output_path()).unwrap();
    }
}

fn sample<T>(system: System, k: usize, prove: impl Fn() -> T) -> T {
    let mut proof = None;
    let sample_size = sample_size(k);
    let sum = iter::repeat_with(|| {
        let start = Instant::now();
        proof = Some(prove());
        start.elapsed()
    })
    .take(sample_size)
    .sum::<Duration>();
    let avg = sum / sample_size as u32;
    writeln!(&mut system.output(), "{k}, {}", avg.as_millis()).unwrap();
    proof.unwrap()
}

fn sample_size(k: usize) -> usize {
    if k < 16 {
        20
    } else if k < 20 {
        5
    } else {
        1
    }
}
