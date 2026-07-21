//! Sidecar path resolution (pure string logic; no filesystem access).

/// Map a Smacker path the engine asked for to its modern sidecar path: if
/// `engine_path` ends in `.smk` (case-insensitive), swap the extension for
/// `container_ext`, keeping the directory and the stem's original casing;
/// otherwise `None`. Existence checks live in the later FFI layer.
pub(crate) fn sidecar_path(engine_path: &str, container_ext: &str) -> Option<String> {
    // ".smk" is four ASCII bytes, so `len - 4` is always a char boundary and
    // slicing there keeps the directory + stem (with original casing) intact.
    // No separator splitting is needed since we only touch the trailing suffix.
    if engine_path.to_ascii_lowercase().ends_with(".smk") {
        let stem = &engine_path[..engine_path.len() - 4];
        Some(format!("{stem}.{container_ext}"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_smk_extension_preserving_dir_and_case() {
        assert_eq!(
            sidecar_path(r"C:\CE\cutscn\intro.smk", "webm"),
            Some(r"C:\CE\cutscn\intro.webm".into())
        );
        assert_eq!(
            sidecar_path(r"cutscn\LOGGA.SMK", "webm"),
            Some(r"cutscn\LOGGA.webm".into())
        );
        assert_eq!(sidecar_path("intro.smk", "webm"), Some("intro.webm".into()));
        assert_eq!(
            sidecar_path("media/clip.SmK", "mp4"),
            Some("media/clip.mp4".into())
        );
    }

    #[test]
    fn returns_none_for_non_smk() {
        assert_eq!(sidecar_path(r"cutscn\intro.txt", "webm"), None);
        assert_eq!(sidecar_path("nodotsmk", "webm"), None);
    }
}
