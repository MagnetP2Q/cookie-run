use image::{imageops::FilterType, DynamicImage};
use rustdct::DctPlanner;
pub fn matches(label: &str, text: &str) -> bool {
    fn norm(s: &str) -> String {
        s.to_lowercase().replace(['!', ' '], "")
    }
    norm(text).contains(&norm(label))
}
// imagehash.phash(hash_size=16) uses a 64x64 grayscale image and a 2D DCT.
fn hash(img: DynamicImage) -> Vec<bool> {
    let gray = img.resize_exact(64, 64, FilterType::Lanczos3).to_luma8();
    let mut v: Vec<f32> = gray.pixels().map(|p| p[0] as f32).collect();
    let mut planner = DctPlanner::new();
    let dct = planner.plan_dct2(64);
    for row in v.chunks_mut(64) {
        dct.process_dct2(row)
    }
    for x in 0..64 {
        let mut col: Vec<f32> = (0..64).map(|y| v[y * 64 + x]).collect();
        dct.process_dct2(&mut col);
        for y in 0..64 {
            v[y * 64 + x] = col[y]
        }
    }
    let low: Vec<f32> = (0..16)
        .flat_map(|y| (0..16).map(move |x| (y, x)))
        .map(|(y, x)| v[y * 64 + x])
        .collect();
    let mut sorted = low.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let med = (sorted[127] + sorted[128]) / 2.;
    low.iter().map(|n| *n > med).collect()
}
pub fn odd_cards(img: &DynamicImage, r: [u32; 4]) -> Vec<[u32; 2]> {
    let w = (r[2] - r[0]) / 3;
    let h = (r[3] - r[1]) / 2;
    let mut cards = Vec::new();
    for y in 0..2 {
        for x in 0..3 {
            cards.push((
                hash(img.crop_imm(r[0] + x * w, r[1] + y * h, w, h)),
                [r[0] + x * w + w / 2, r[1] + y * h + h / 2],
            ))
        }
    }
    let mut scores: Vec<(usize, usize)> = (0..6)
        .map(|i| {
            (
                i,
                (0..6)
                    .filter(|&j| j != i)
                    .map(|j| {
                        cards[i]
                            .0
                            .iter()
                            .zip(&cards[j].0)
                            .filter(|(a, b)| a != b)
                            .count()
                    })
                    .sum(),
            )
        })
        .collect();
    scores.sort_by(|a, b| b.1.cmp(&a.1));
    scores.iter().take(2).map(|(i, _)| cards[*i].1).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalization() {
        assert!(matches("Play!", " PLAY ! "));
        assert!(!matches("Confirm", "Cancel"));
    }
}
