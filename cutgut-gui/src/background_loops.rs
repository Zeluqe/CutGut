macro_rules! preset_loops {
    ($($id:literal),+ $(,)?) => {
        pub fn preset_loop(id: &str) -> Option<&'static [u8]> {
            match id {
                $($id => Some(include_bytes!(concat!("../assets/backgrounds/motion/", $id, ".webm"))),)+
                _ => None,
            }
        }
    };
}

preset_loops!(
    "aurora-blobs",
    "cloud-sky",
    "crimson-dither",
    "dot-swell",
    "ember-dusk",
    "gold-drift-dither",
    "grain-gradient-ember",
    "halftone-sunset",
    "halo-veil",
    "light-leak",
    "orchid-dusk",
    "paper-dithering-swirl",
    "paper-gem-smoke-infrared",
    "paper-grain-gradient-truchet",
    "paper-mesh-gradient-beach",
    "paper-neuro-noise-ghost",
    "paper-smoke-ring-cloud",
    "paper-spiral-jungle",
    "paper-swirl-default",
    "paper-warp-cauldron-pot",
    "paper-warp-live-ink",
    "paper-warp-passion",
    "paper-water-slow-mo",
    "plasma-pink",
    "ribbed-sunset",
    "ripple-cursor",
);
