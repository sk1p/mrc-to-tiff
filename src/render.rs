use eframe::egui::ColorImage;

fn get_quantile(data: &[f32], q: f32) -> f32 {
    let mut data: Vec<f32> = data.to_vec();
    data.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let idx_for_q: usize = ((data.len() as f32 * q) as usize).min(data.len() - 1).max(0);

    data[idx_for_q]
}

pub fn render_to_rgb(data: &[i16], nx: usize, ny: usize, quantile: f32) -> ColorImage {
    let (vmin, vmax) = &data.iter().fold((i16::MAX, i16::MIN), |a, &b| {
        (a.0.min(b), a.1.max(b))
    });

    let vmin = *vmin as f32;
    let vmax = *vmax as f32;
    
    let data: Vec<f32> = data.iter().map(|v| *v as f32).collect();

    let vmax_quantiled = get_quantile(&data, quantile);

    let normalizer = |(idx, v): (usize, &f32)| (idx, (v - vmin) / (vmax_quantiled - vmin));

    let to_rgba = |(idx, value): (usize, &f32)| {
        let c = 255.0 * *value;
        // let x = (idx % width) as u16;
        // let y = (idx / width) as u16;
        let a = 255;
        [c as u8, c as u8, c as u8, a]
    };

    let iter_flat = (0..).zip(data.iter());

    let mapped: Vec<u8> = if vmax_quantiled == vmin {
        iter_flat.flat_map(to_rgba).collect()
    } else {
        iter_flat
            .map(normalizer)
            .flat_map(|(idx, v)| to_rgba((idx, &v)))
            .collect()
    };

    ColorImage::from_rgba_unmultiplied([ny, nx], &mapped)
}
