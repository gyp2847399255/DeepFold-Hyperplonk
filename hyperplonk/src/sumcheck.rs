use arithmetic::field::{batch_inverse, Field};
use util::fiat_shamir::{Proof, Transcript};

pub struct Sumcheck;

impl Sumcheck {
    fn fold_next_domain<F: Field>(poly_evals: &mut Vec<F>, m: usize, challenge: F) {
        for j in 0..m {
            poly_evals[j] =
                poly_evals[j * 2] + (poly_evals[j * 2 + 1] - poly_evals[j * 2]) * challenge;
        }
        poly_evals.truncate(m)
    }

    pub fn prove<F: Field, const N: usize>(
        mut evals: [Vec<F>; N],
        degree: usize,
        transcript: &mut Transcript,
        f: fn([F; N]) -> F,
    ) -> (Vec<F>, [F; N]) {
        let var_num = evals[0].len().ilog2() as usize;
        let mut new_point = vec![];
        for i in 0..var_num {
            let m = 1usize << (var_num - i);
            let sums = (0..m)
                .step_by(2)
                .fold(vec![F::zero(); degree + 1], |mut acc, x| {
                    let mut extrapolations = vec![];
                    for j in 0..N {
                        let v_0 = evals[j][x];
                        let v_1 = evals[j][x + 1];
                        let diff = v_1 - v_0;
                        let mut e = vec![v_0, v_1];
                        for k in 1..degree {
                            e.push(e[k] + diff);
                        }
                        extrapolations.push(e);
                    }
                    for i in 0..degree + 1 {
                        let mut res = vec![extrapolations[0][i]];
                        for j in 1..N {
                            res.push(extrapolations[j][i]);
                        }
                        acc[i] += f(res.try_into().unwrap());
                    }
                    acc
                });
            for j in sums {
                transcript.append_f(j);
            }
            let challenge = transcript.challenge_f();
            new_point.push(challenge);
            for j in evals.iter_mut() {
                Self::fold_next_domain(j, m / 2, challenge)
            }
        }
        (new_point, evals.map(|x| x[0]))
    }

    fn init_base<F: Field>(n: usize) -> Vec<F> {
        let mut res = vec![];
        for i in 0..n + 1 {
            let mut prod = F::one();
            for j in 0..n + 1 {
                if i != j {
                    prod *= F::from(i as u32) - F::from(j as u32);
                }
            }
            res.push(prod);
        }
        batch_inverse(&mut res);
        res
    }

    fn uni_extrapolate<F: Field>(base: &Vec<F>, v: Vec<F>, x: F) -> F {
        let n = base.len() - 1;
        let mut prod = x;
        for i in 1..n + 1 {
            prod *= x - F::from(i as u32);
        }
        let mut numerator = (0..n + 1)
            .map(|y| x - F::from(y as u32))
            .collect::<Vec<_>>();
        batch_inverse(&mut numerator);
        let mut res = F::zero();
        for i in 0..n + 1 {
            res += numerator[i] * base[i] * v[i];
        }
        res * prod
    }

    pub fn verify<F: Field>(
        mut y: F,
        degree: usize,
        var_num: usize,
        transcript: &mut Transcript,
        proof: &mut Proof,
    ) -> (Vec<F>, F) {
        let mut res = vec![];
        let base = Self::init_base(degree);
        for _ in 0..var_num {
            let mut sums = vec![];
            for _ in 0..degree + 1 {
                let x: F = proof.get_next_and_step();
                sums.push(x);
                transcript.append_f(x);
            }
            assert_eq!(sums[0] + sums[1], y);
            let challenge = transcript.challenge_f();
            res.push(challenge);
            y = Self::uni_extrapolate(&base, sums, challenge);
        }
        (res, y)
    }
}

#[cfg(test)]
mod tests {
    use arithmetic::{
        field::{goldilocks64::Goldilocks64Ext, Field},
        poly::MultiLinearPoly,
    };
    use rand::thread_rng;
    use util::fiat_shamir::Transcript;

    use super::Sumcheck;

    #[test]
    fn test_sumcheck() {
        let mut rng = thread_rng();
        let a = (0..4096)
            .map(|_| Goldilocks64Ext::random(&mut rng))
            .collect::<Vec<_>>();
        let b = (0..4096)
            .map(|_| Goldilocks64Ext::random(&mut rng))
            .collect::<Vec<_>>();
        let c = (0..4096)
            .map(|_| Goldilocks64Ext::random(&mut rng))
            .collect::<Vec<_>>();
        let d = (0..4096)
            .map(|_| Goldilocks64Ext::random(&mut rng))
            .collect::<Vec<_>>();
        let mut transcript = Transcript::new();
        Sumcheck::prove(
            [a.clone(), b.clone(), c.clone(), d.clone()],
            3,
            &mut transcript,
            |v: [Goldilocks64Ext; 4]| (v[0] * v[1] + v[2]) * v[3],
        );
        let y = (0..4096).fold(Goldilocks64Ext::zero(), |acc, x| {
            acc + (a[x] * b[x] + c[x]) * d[x]
        });
        let mut proof = transcript.proof;
        let mut transcript = Transcript::new();
        let (point, y) = Sumcheck::verify(y, 3, 12, &mut transcript, &mut proof);
        assert_eq!(
            (MultiLinearPoly::eval_multilinear_ext(&a, &point)
                * MultiLinearPoly::eval_multilinear_ext(&b, &point)
                + MultiLinearPoly::eval_multilinear_ext(&c, &point))
                * MultiLinearPoly::eval_multilinear_ext(&d, &point),
            y
        );
    }
}