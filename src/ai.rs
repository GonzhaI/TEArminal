use serde::{Deserialize, Serialize};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AiMessage {
    pub sender: String, // "user" o "axon"
    pub text: String,
    pub timestamp: String,
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

pub struct AiAssistant {
    pub messages: Vec<AiMessage>,
    pub is_generating: bool,
    tx: Sender<(String, String, String)>, // (endpoint, model, prompt)
    rx: Receiver<Result<String, String>>,
}

impl AiAssistant {
    pub fn new() -> Self {
        let (tx, task_rx) = channel::<(String, String, String)>();
        let (res_tx, rx) = channel::<Result<String, String>>();

        // Hilo de fondo con runtime Tokio para peticiones HTTP
        thread::Builder::new()
            .name("axon-ai-worker".into())
            .spawn(move || {
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(r) => r,
                    Err(_) => return,
                };

                let client = reqwest::Client::new();

                while let Ok((endpoint, model, prompt)) = task_rx.recv() {
                    let client_ref = client.clone();
                    let res_tx_ref = res_tx.clone();

                    rt.block_on(async move {
                        let url = format!("{}/api/generate", endpoint.trim_end_matches('/'));
                        let payload = OllamaRequest {
                            model,
                            prompt,
                            stream: false,
                        };

                        match client_ref.post(&url).json(&payload).send().await {
                            Ok(resp) => {
                                if resp.status().is_success() {
                                    if let Ok(ollama_resp) = resp.json::<OllamaResponse>().await {
                                        let _ = res_tx_ref.send(Ok(ollama_resp.response));
                                    } else {
                                        let _ = res_tx_ref.send(Err("Error parseando respuesta del modelo.".into()));
                                    }
                                } else {
                                    let _ = res_tx_ref.send(Err(format!("Error HTTP {}", resp.status())));
                                }
                            }
                            Err(e) => {
                                let _ = res_tx_ref.send(Err(format!(
                                    "No se pudo conectar a la IA en {}: ¿Ollama está encendido? ({})",
                                    url, e
                                )));
                            }
                        }
                    });
                }
            })
            .expect("Failed to spawn AI worker");

        Self {
            messages: vec![AiMessage {
                sender: "Axon".into(),
                text: "Hola! Soy el copiloto de Axon. Puedo analizar errores de la terminal, sugerir comandos o explicar salidas complejas.".into(),
                timestamp: chrono::Local::now().format("%H:%M").to_string(),
            }],
            is_generating: false,
            tx,
            rx,
        }
    }

    pub fn ask(&mut self, endpoint: &str, model: &str, user_query: &str, terminal_context: &str) {
        let prompt = format!(
            "Eres el asistente integrado de la terminal Axon.\n\
            Contexto reciente de la terminal:\n```\n{}\n```\n\
            Pregunta o solicitud del usuario: {}\n\
            Responde de forma clara, técnica, breve y proporciona comandos precisos listos para copiar.",
            terminal_context, user_query
        );

        self.messages.push(AiMessage {
            sender: "Tú".into(),
            text: user_query.to_string(),
            timestamp: chrono::Local::now().format("%H:%M").to_string(),
        });

        self.is_generating = true;
        let _ = self.tx.send((endpoint.to_string(), model.to_string(), prompt));
    }

    pub fn update(&mut self) {
        if let Ok(result) = self.rx.try_recv() {
            self.is_generating = false;
            let text = match result {
                Ok(reply) => reply,
                Err(err) => format!("[Aviso IA] {}", err),
            };
            self.messages.push(AiMessage {
                sender: "Axon".into(),
                text,
                timestamp: chrono::Local::now().format("%H:%M").to_string(),
            });
        }
    }
}
