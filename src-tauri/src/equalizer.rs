// 均衡器
#[derive(Clone)]
pub struct Equalizer {
    bands: [f32; 10],
}

impl Equalizer {
    pub fn new() -> Self {
        Self {
            bands: [0.0; 10],
        }
    }

    pub fn set_bands(&mut self, bands: [f32; 10]) {
        self.bands = bands;
    }

    pub fn get_bands(&self) -> [f32; 10] {
        self.bands
    }
}

impl Default for Equalizer {
    fn default() -> Self {
        Self::new()
    }
}
