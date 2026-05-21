use std::f64::consts::PI;

use num::{Complex, pow::Pow};

fn fft(input: &[f64]) -> Vec<Complex<f64>> {
    let n = input.len();

    if n <= 1 {
        return input.iter().map(Complex::from).collect();
    }

    let evens: Vec<f64> = input.iter().step_by(2).map(|x| *x).collect();
    let odds: Vec<f64> = input.iter().skip(1).step_by(2).map(|x| *x).collect();

    let even = fft(&evens[..]);
    let odd = fft(&odds[..]);

    let w = (-2.0 * PI / n as f64 * Complex::i()).exp();
    let mut out = vec![Complex::new(0.0, 0.0); input.len()];
    for k in 0..n / 2 {
        let t = w.pow(k as f64) * odd[k];
        out[k] = even[k] + t;
        out[k + n / 2] = even[k] - t;
    }

    out
}
