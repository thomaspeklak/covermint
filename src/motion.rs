pub(crate) fn ease_in_out_cubic(progress: f64) -> f64 {
    let t = progress.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - ((-2.0 * t + 2.0).powi(3) / 2.0)
    }
}

pub(crate) fn ease_out_back_subtle(progress: f64) -> f64 {
    let t = progress.clamp(0.0, 1.0);
    let overshoot = 0.6;
    let c3 = overshoot + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + overshoot * (t - 1.0).powi(2)
}
