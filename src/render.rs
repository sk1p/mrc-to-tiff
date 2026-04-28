use eframe::egui::{Color32, ColorImage};
use rand::seq::{IndexedRandom, IteratorRandom};

/// Get the quantile of a given slice by sorting the input.
pub fn get_quantile_by_sort(data: &[f32], q: f32) -> f32 {
    let mut data: Vec<f32> = data.to_vec();
    data.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let idx_for_q: usize = ((data.len() as f32 * q) as usize).min(data.len() - 1);

    data[idx_for_q]
}

/// Get the quantile of a given slice.
///
/// We don't need to sort the full array, we just need to
/// select a single element at a given index! Much faster.
pub fn get_quantile_by_select(data: &[f32], q: f32) -> f32 {
    let idx_for_q: usize = ((data.len() as f32 * q) as usize).min(data.len() - 1);

    let mut data: Vec<f32> = data.to_vec();
    data.select_nth_unstable_by(idx_for_q, |a, b| a.partial_cmp(b).unwrap());

    data[idx_for_q]
}

/// Get the quantile of a given slice using pdqselect
pub fn get_quantile_by_pdqselect(data: &[f32], q: f32) -> f32 {
    let idx_for_q: usize = ((data.len() as f32 * q) as usize).min(data.len() - 1);

    let mut data: Vec<f32> = data.to_vec();
    pdqselect::select_by(&mut data, idx_for_q, |a, b| a.partial_cmp(b).unwrap());

    data[idx_for_q]
}

/// Get the quantile of a given slice by selecting from a random sample
pub fn get_quantile_by_random_sample(data: &[f32], q: f32) -> f32 {
    let mut rng = rand::rng();
    let mut sample: Vec<f32> = data.sample(&mut rng, 15000).copied().collect();
    let idx_for_q: usize = ((sample.len() as f32 * q) as usize).min(sample.len() - 1);

    sample.select_nth_unstable_by(idx_for_q, |a, b| a.partial_cmp(b).unwrap());

    sample[idx_for_q]
}

pub fn render_to_rgb(data: &[f32], nx: usize, ny: usize, quantile: f32) -> ColorImage {
    let (vmin, vmax) = &data
        .iter()
        .fold((f32::MAX, f32::MIN), |a, &b| (a.0.min(b), a.1.max(b)));

    let vmax_quantiled = get_quantile_by_random_sample(data, quantile);

    let normalizer = |v| (v - vmin) / (vmax_quantiled - vmin);
    let to_rgba = |value: &f32| {
        let c = 255.0 * value;
        Color32::from_rgb(c as u8, c as u8, c as u8)
    };

    let iter_flat = data.iter();

    let mapped: Vec<Color32> = if vmax_quantiled == *vmin {
        iter_flat.map(to_rgba).collect()
    } else {
        iter_flat.map(normalizer).map(|v| to_rgba(&v)).collect()
    };

    ColorImage::new([ny, nx], mapped)
}

#[cfg(test)]
mod tests {
    use rand::RngExt;

    use crate::render::{get_quantile_by_select, get_quantile_by_sort};

    #[test]
    fn test_quantile_equal() {
        let mut rng = rand::rng();

        // something large
        let mut random_input = vec![0u32; 400 * 400];
        rng.fill(&mut random_input);

        let random_input: Vec<f32> = random_input.into_iter().map(|i| i as f32).collect();
        assert_eq!(
            get_quantile_by_sort(&random_input[..], 0.9999),
            get_quantile_by_select(&random_input[..], 0.9999),
        );
    }
}
