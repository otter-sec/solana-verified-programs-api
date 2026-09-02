//! Static brand assets compiled into the binary.
//!
//! The service has no filesystem asset serving, and the release image copies
//! only the built binary, so the fonts the landing page needs are embedded with
//! `include_bytes!` and handed out from a fixed table. Filenames are stable, so
//! the responses are marked immutable.

use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

/// One year, the maximum `max-age` browsers honour in practice.
const CACHE_CONTROL: &str = "public, max-age=31536000, immutable";

const WOFF2: &str = "font/woff2";

/// Every asset the landing page may request, as `(name, bytes, content type)`.
///
/// The landing page references these by name; `landing_references_every_asset`
/// in `super::index` pins the two lists together.
pub static ASSETS: &[(&str, &[u8], &str)] = &[
    (
        "MDPrimer-Regular.woff2",
        include_bytes!("../../../assets/landing/MDPrimer-Regular.woff2"),
        WOFF2,
    ),
    (
        "MDUIXL-Regular.woff2",
        include_bytes!("../../../assets/landing/MDUIXL-Regular.woff2"),
        WOFF2,
    ),
    (
        "Lilex-Variable.woff2",
        include_bytes!("../../../assets/landing/Lilex-Variable.woff2"),
        WOFF2,
    ),
    // The grain background shader, bundled from @paper-design/shaders — the
    // same mount osec.io uses. See docs/plans for the bundle recipe.
    (
        // Public URL includes the bundle hash so the one-year immutable cache
        // remains safe when a future shader rebuild changes the bytes.
        "grain-6bef640a.js",
        include_bytes!("../../../assets/landing/grain.js"),
        "text/javascript; charset=utf-8",
    ),
];

/// Handler for the static asset endpoint
///
/// # Endpoint: GET /assets/{file}
///
/// # Returns
///
/// The embedded asset with its content type and an immutable cache header, or
/// `404` when the name is not in [`ASSETS`]. Lookup is an exact match against
/// the table, so no path outside the binary is reachable.
pub async fn serve(Path(file): Path<String>) -> Response {
    match ASSETS.iter().find(|(name, _, _)| *name == file) {
        Some((_, bytes, content_type)) => (
            [
                (header::CONTENT_TYPE, *content_type),
                (header::CACHE_CONTROL, CACHE_CONTROL),
            ],
            *bytes,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    /// woff2 files start with the `wOF2` signature.
    const WOFF2_MAGIC: &[u8] = b"wOF2";

    async fn get(file: &str) -> Response {
        serve(Path(file.to_string())).await
    }

    #[tokio::test]
    async fn every_asset_serves_its_bytes() {
        for (name, bytes, _) in ASSETS {
            let response = get(name).await;
            assert_eq!(response.status(), StatusCode::OK, "{name} should be served");

            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            assert!(!body.is_empty(), "{name} served an empty body");
            assert_eq!(body.len(), bytes.len(), "{name} served a truncated body");
        }
    }

    #[tokio::test]
    async fn woff2_assets_are_really_woff2() {
        for (name, bytes, content_type) in ASSETS {
            if *content_type != WOFF2 {
                continue;
            }
            assert!(
                bytes.starts_with(WOFF2_MAGIC),
                "{name} is not a woff2 file (wrong file vendored?)"
            );
        }
    }

    #[tokio::test]
    async fn assets_carry_content_type_and_immutable_cache() {
        for (name, _, content_type) in ASSETS {
            let response = get(name).await;
            let headers = response.headers();

            assert_eq!(
                headers.get(header::CONTENT_TYPE).unwrap(),
                *content_type,
                "{name} has the wrong content type"
            );
            assert!(
                headers
                    .get(header::CACHE_CONTROL)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .contains("immutable"),
                "{name} is not marked immutable"
            );
        }
    }

    #[tokio::test]
    async fn unknown_asset_is_404() {
        assert_eq!(get("nope.woff2").await.status(), StatusCode::NOT_FOUND);
        assert_eq!(get("").await.status(), StatusCode::NOT_FOUND);
    }

    /// The table is an exact-match lookup, so traversal is structurally
    /// impossible. This pins that property against a refactor to disk reads.
    #[tokio::test]
    async fn traversal_attempts_are_404() {
        for attempt in [
            "../Cargo.toml",
            "../../etc/passwd",
            "assets/MDPrimer-Regular.woff2",
            "/MDPrimer-Regular.woff2",
        ] {
            assert_eq!(
                get(attempt).await.status(),
                StatusCode::NOT_FOUND,
                "{attempt} should not resolve"
            );
        }
    }

    #[test]
    fn asset_names_are_unique() {
        let mut names: Vec<&str> = ASSETS.iter().map(|(name, _, _)| *name).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(count, names.len(), "duplicate asset name in the table");
    }
}
