//! `SSH_ASKPASS` apuntando al propio ejecutable de TEArminal: `ssh.exe` lo lanza para pedir la frase de paso de la
//! llave y aquí se lee del Administrador de credenciales. Así la frase nunca viaja por argumentos ni por variables de
//! entorno (la variable solo lleva el `id` de la conexión, que no es secreto).

use super::credentials::{self, SecretKind};
use std::io::Write;

/// Variable que activa este modo; su valor es el `id` de la conexión
pub(crate) const ENV_VAR: &str = "TEARMINAL_ASKPASS";

/// Gancho para el inicio de `main`: si `ssh` nos lanzó como askpass, responde y termina el proceso
pub(crate) fn run_if_requested() {
    let Some(id) = std::env::var_os(ENV_VAR) else {
        return;
    };
    let prompt = std::env::args().nth(1).unwrap_or_default();
    let code = answer(&id.to_string_lossy(), &prompt, &mut std::io::stdout());
    std::process::exit(code);
}

/// Solo responde a la petición de la frase de paso de una llave: nunca a «password:» ni a una pregunta
/// de huella o de teclado interactivo que pudiera inventar un servidor hostil.
fn answer(connection_id: &str, prompt: &str, out: &mut impl Write) -> i32 {
    let valid_id =
        !connection_id.is_empty() && connection_id.chars().all(|c| c.is_ascii_alphanumeric());
    if !valid_id || !is_key_passphrase_prompt(prompt) {
        return 1;
    }
    match credentials::load(SecretKind::SshPassphrase, connection_id) {
        Ok(Some(secret)) if writeln!(out, "{}", secret.expose()).is_ok() && out.flush().is_ok() => {
            0
        }
        _ => 1,
    }
}

fn is_key_passphrase_prompt(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    lower.starts_with("enter passphrase for") && !lower.contains("password")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::credentials::Secret;

    #[test]
    fn only_key_passphrase_prompts_are_answered() {
        assert!(is_key_passphrase_prompt(
            "Enter passphrase for key 'C:\\k.pem': "
        ));
        for prompt in [
            "deploy@host's password: ",
            "Password: ",
            "Are you sure you want to continue connecting (yes/no)? ",
            "Verification code (enter passphrase for key): ",
            "",
        ] {
            assert!(!is_key_passphrase_prompt(prompt), "{prompt:?}");
        }
    }

    #[test]
    fn refuses_foreign_prompts_and_hostile_ids_without_writing() {
        let mut out = Vec::new();
        assert_eq!(answer("abc", "Password: ", &mut out), 1);
        assert_eq!(
            answer("../x", "Enter passphrase for key 'k': ", &mut out),
            1
        );
        assert_eq!(answer("", "Enter passphrase for key 'k': ", &mut out), 1);
        assert!(out.is_empty());
    }

    #[test]
    fn answers_with_the_stored_passphrase() {
        let id = format!("askpass{}", std::process::id());
        let secret = Secret::new("frase de paso");
        if credentials::store(SecretKind::SshPassphrase, &id, &secret).is_err() {
            return; // sin Administrador de credenciales no hay nada que probar
        }
        let mut out = Vec::new();
        assert_eq!(
            answer(&id, "Enter passphrase for key 'k.pem': ", &mut out),
            0
        );
        assert_eq!(String::from_utf8_lossy(&out), "frase de paso\n");
        let _ = credentials::delete(SecretKind::SshPassphrase, &id);
        let mut out = Vec::new();
        assert_eq!(
            answer(&id, "Enter passphrase for key 'k.pem': ", &mut out),
            1
        );
        assert!(out.is_empty());
    }
}
