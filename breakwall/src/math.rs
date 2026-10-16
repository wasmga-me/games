//! Small float helpers the rules use.

pub fn absf(v: f32) -> f32 {
    v.abs()
}
pub fn clampf(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi)
}
pub fn sqrtf(v: f32) -> f32 {
    v.sqrt()
}
pub fn sinf(x: f32) -> f32 {
    x.sin()
}
pub fn cosf(x: f32) -> f32 {
    x.cos()
}
