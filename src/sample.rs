#[derive(Debug, Clone)]
pub struct Sample {
    pub id: isize,
    pub path: String,
    pub name: String,
    pub format: String,
    pub sample_rate: u32,
    pub size: u64,
}
