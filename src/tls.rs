use anyhow::Context;
use anyhow::anyhow;
use rumqttc::{TlsConfiguration, Transport, tokio_rustls::rustls::pki_types::PrivateKeyDer};
use rustls::DigitallySignedStruct;
use rustls::Error;
use rustls::SignatureScheme;
use rustls::client::danger::HandshakeSignatureValid;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::version::TLS12;
use rustls::{
    ClientConfig, RootCertStore,
    client::danger::{ServerCertVerified, ServerCertVerifier},
};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use toolkit_rs::AppResult;
pub fn load_cert<P: AsRef<Path>>(filename: P) -> AppResult<CertificateDer<'static>> {
    let mut all_certs: Vec<CertificateDer> = load_certs(&filename)?;
    if all_certs.len() != 1 {
        return Err(anyhow!(
            "invalid number of certificates in {:?}, expected 1 got {:?}",
            filename.as_ref(),
            all_certs.len()
        ));
    }

    let Some(p) = all_certs.pop() else {
        return Err(anyhow!("no certs found in {:?}", filename.as_ref()));
    };
    Ok(p)
}

pub fn load_certs<P: AsRef<Path>>(filename: P) -> AppResult<Vec<CertificateDer<'static>>> {
    let certfile = File::open(&filename)?;
    let mut reader = BufReader::new(&certfile);
    let certs: Vec<CertificateDer> = rustls_pemfile::certs(&mut reader).flatten().collect();
    Ok(certs)
}

pub fn load_private_key<P: AsRef<Path>>(filename: P) -> AppResult<PrivateKeyDer<'static>> {
    let keyfile = File::open(&filename)?;
    let mut reader = BufReader::new(keyfile);

    loop {
        match rustls_pemfile::read_one(&mut reader)? {
            Some(rustls_pemfile::Item::Pkcs1Key(key)) => return Ok(key.into()),
            Some(rustls_pemfile::Item::Pkcs8Key(key)) => return Ok(key.into()),
            Some(rustls_pemfile::Item::Sec1Key(key)) => return Ok(key.into()),
            None => break,
            _ => {}
        }
    }

    Err(anyhow!(
        "no keys found in {:?} (encrypted keys not supported)",
        filename.as_ref()
    ))
}

// 自定义证书验证器（用于IP地址连接时跳过域名验证）
#[derive(Debug)]
pub struct NoOpVerifier;

impl ServerCertVerifier for NoOpVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        log::trace!("NoOpVerifier::verify_server_cert");
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        log::trace!("NoOpVerifier::verify_tls12_signature");
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        log::debug!("NoOpVerifier::verify_tls13_signature");
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA1,
            SignatureScheme::ECDSA_SHA1_Legacy,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
            SignatureScheme::ED448,
        ]
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct TlsCert {
    pub device_certs: String,
    pub device_private_key: String,
    pub ca_certs: String,
}

//
pub fn default_tls_client_config(certs: TlsCert) -> AppResult<ClientConfig> {
    // 证书
    let device_certs = vec![load_cert(certs.device_certs)?];
    // 私钥
    let private_key = load_private_key(certs.device_private_key)?;
    //ca
    let ca_certs = load_certs(certs.ca_certs)?;
    let mut root_store = RootCertStore::empty();
    for ca in ca_certs {
        root_store.add(ca)?;
    }
    //
    let config = ClientConfig::builder_with_protocol_versions(&[&TLS12])
        .with_root_certificates(root_store)
        .with_client_auth_cert(device_certs, private_key)
        .with_context(|| "Error setting up mTLS")?;

    // config
    //     .dangerous()
    //     .set_certificate_verifier(Arc::new(NoOpVerifier));

    Ok(config)
}

//
pub fn default_transport_config(certs: TlsCert) -> AppResult<Transport> {
    let config = default_tls_client_config(certs)?;
    let t = Transport::Tls(TlsConfiguration::Rustls(Arc::new(config)));
    Ok(t)
}
