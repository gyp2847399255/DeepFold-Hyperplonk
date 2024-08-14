use std::time::Instant;

use arithmetic::{
    field::{
        goldilocks64::{Goldilocks64, Goldilocks64Ext},
        Field,
    },
    mul_group::Radix2Group,
};
use poly_commit::deepfold::{DeepFoldParam, DeepFoldProver, DeepFoldVerifier};
use rand::thread_rng;

use hyperplonk::{circuit::Circuit, prover::Prover, verifier::Verifier};

fn main() {
    bench_mock_circuit(26, 1);
}

fn bench_mock_circuit(nv: u32, repetition: usize) {
    let num_gates = 1u32 << nv;
    let mock_circuit = Circuit::<Goldilocks64Ext> {
        permutation: [
            (0..num_gates).map(|x| x.into()).collect(),
            (0..num_gates).map(|x| (x + (1 << 29)).into()).collect(),
            (0..num_gates).map(|x| (x + (1 << 30)).into()).collect(),
        ], // identical permutation
        selector: (0..num_gates).map(|x| (x & 1).into()).collect(),
    };

    let mut mult_subgroups = vec![Radix2Group::<Goldilocks64>::new(nv + 3)];
    for i in 1..nv as usize {
        mult_subgroups.push(mult_subgroups[i - 1].exp(2));
    }
    let pp = DeepFoldParam::<Goldilocks64Ext> {
        mult_subgroups,
        variable_num: 12,
        query_num: 30,
    };
    let (pk, vk) = mock_circuit.setup::<DeepFoldProver<_>, DeepFoldVerifier<_>>(&pp, &pp);
    let prover = Prover { prover_key: pk };
    let verifier = Verifier { verifier_key: vk };
    let a = (0..num_gates)
        .map(|_| Goldilocks64::random(&mut thread_rng()))
        .collect::<Vec<_>>();
    let b = (0..num_gates)
        .map(|_| Goldilocks64::random(&mut thread_rng()))
        .collect::<Vec<_>>();
    let c = (0..num_gates)
        .map(|i| {
            let i = i as usize;
            let s = mock_circuit.selector[i];
            -((Goldilocks64::one() - s) * (a[i] + b[i]) + s * a[i] * b[i])
        })
        .collect::<Vec<_>>();
    let start = Instant::now();
    for _ in 0..repetition {
        let proof = prover.prove(&pp, nv as usize, [a.clone(), b.clone(), c.clone()]);
    }
    println!("proving for 2^{} gates: {} us", nv, start.elapsed().as_micros() / repetition as u128);
    
    // let proof = prover.prove(&pp, nv as usize, [a, b, c]);
    // assert!(verifier.verify(&pp, nv as usize, proof));
}
