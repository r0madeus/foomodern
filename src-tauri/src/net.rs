//! Shared HTTP client setup.

/// A reqwest builder with the platform's TLS set up.
///
/// Android: rustls (ring) with the Mozilla root store. reqwest's default
/// rustls setup uses the platform verifier, which needs JNI setup on
/// Android, and native-tls would mean cross-compiling OpenSSL. Desktop
/// uses the system TLS (native-tls).
pub fn client_builder() -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder();
    #[cfg(target_os = "android")]
    let builder = builder.tls_backend_preconfigured(android_tls());
    builder
}

#[cfg(target_os = "android")]
fn android_tls() -> rustls::ClientConfig {
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("ring supports the default TLS versions")
    .with_root_certificates(roots)
    .with_no_client_auth()
}
