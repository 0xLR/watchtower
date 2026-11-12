use watchtower_core::MediaEngine;

pub fn available() -> Vec<MediaEngine> {
    let mut engines = vec![MediaEngine::Retina];
    if cfg!(feature = "go2rtc") {
        engines.push(MediaEngine::Go2rtc);
    }
    if cfg!(feature = "mediamtx") {
        engines.push(MediaEngine::MediaMtx);
    }
    engines
}

pub fn effective(requested: MediaEngine) -> MediaEngine {
    if available().contains(&requested) {
        requested
    } else {
        MediaEngine::Retina
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_is_always_available_and_optional_engines_follow_features() {
        let engines = available();
        assert!(engines.contains(&MediaEngine::Retina));
        assert_eq!(
            engines.contains(&MediaEngine::Go2rtc),
            cfg!(feature = "go2rtc")
        );
        assert_eq!(
            engines.contains(&MediaEngine::MediaMtx),
            cfg!(feature = "mediamtx")
        );
    }

    #[test]
    fn saved_engines_resolve_without_mutating_preferences() {
        for engine in [
            MediaEngine::Retina,
            MediaEngine::Go2rtc,
            MediaEngine::MediaMtx,
        ] {
            assert_eq!(
                effective(engine),
                if available().contains(&engine) {
                    engine
                } else {
                    MediaEngine::Retina
                }
            );
        }
    }
}
