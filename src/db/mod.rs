//! Módulo de bases de datos (fase F5: solo backend, sin pantallas).
//!
//! Todo es local: sin nube ni telemetría. Las contraseñas y frases de paso viven en el Administrador de credenciales
//! de Windows y los `.pem` solo se referencian por ruta. Los drivers (feature `db`) corren sobre un runtime de tokio
//! perezoso; la API pública (`Session`) es bloqueante, para llamarla desde un hilo como `TearminalApp::spawn_ssh`.
//!
//! * Sin feature `db`: conexiones, credenciales, reglas de seguridad, permisos de `.pem`, túnel SSH y escapado.
//! * Con feature `db`: además los drivers, TLS con rustls, esquema, sesión y aplicación de cambios.
// ponytail: la API la consume F6 (pestañas de datos); hasta entonces solo la usan los tests.
#![allow(dead_code)]

pub(crate) mod askpass;
pub(crate) mod credentials;
pub(crate) mod perm;
pub(crate) mod safety;
pub(crate) mod sanitize;
pub(crate) mod store;
pub(crate) mod tunnel;

#[cfg(feature = "db")]
mod apply;
#[cfg(feature = "db")]
mod driver;
#[cfg(feature = "db")]
mod mssql;
#[cfg(feature = "db")]
mod mysql;
#[cfg(feature = "db")]
mod pg;
#[cfg(feature = "db")]
mod schema;
#[cfg(feature = "db")]
mod session;
#[cfg(feature = "db")]
mod tls;

#[cfg(all(test, feature = "db"))]
mod integration_tests;
